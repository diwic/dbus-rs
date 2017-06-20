use super::{Error, ffi, libc, to_c_str, c_str_to_slice, Watch, Message, MessageType, BusName, Path, ConnPath};
use super::{RequestNameReply, ReleaseNameReply, BusType, WatchEvent};
use super::watch::WatchList;
use std::{fmt, mem, ptr, thread, panic};
use std::collections::LinkedList;
use std::cell::{Cell, RefCell};
use std::os::unix::io::RawFd;
use std::os::raw::{c_void, c_char, c_int, c_uint};


impl WatchEvent {
    /// After running poll, this transforms the revents into a parameter you can send into `Connection::watch_handle`
    pub fn from_revents(revents: libc::c_short) -> c_uint {
        0 +
        if (revents & libc::POLLIN) != 0 { WatchEvent::Readable as c_uint } else { 0 } +
        if (revents & libc::POLLOUT) != 0 { WatchEvent::Writable as c_uint } else { 0 } +
        if (revents & libc::POLLERR) != 0 { WatchEvent::Error as c_uint } else { 0 } +
        if (revents & libc::POLLHUP) != 0 { WatchEvent::Hangup as c_uint } else { 0 } 
    }
}

/// When listening for incoming events on the D-Bus, this enum will tell you what type
/// of incoming event has happened.
#[derive(Debug)]
pub enum ConnectionItem {
    /// No event between now and timeout
    Nothing,
    /// Incoming method call
    MethodCall(Message),
    /// Incoming signal
    Signal(Message),
    /// Incoming method return (mostly used for Async I/O)
    MethodReturn(Message),
    /// Indicates whether a file descriptor should be monitored or not.
    /// Unless you're doing Async I/O, you can simply ignore this variant.
    WatchFd(Watch),
}

/// ConnectionItem iterator
pub struct ConnectionItems<'a> {
    c: &'a Connection,
    timeout_ms: Option<i32>,
    handlers: Vec<Box<MsgHandler>>,
}

impl<'a> ConnectionItems<'a> {
    /// Builder method that adds a new msg handler.
    pub fn with<H: 'static + MsgHandler>(mut self, h: H) -> Self {
        self.handlers.push(Box::new(h)); self
    }

    // Returns true if processed, false if not
    fn process_handlers(&mut self, ci: &ConnectionItem) -> bool {
        let mut i = 0;
        while i < self.handlers.len() {
            if let Some(r) = self.handlers[i].handle_ci(ci) {
                for msg in r.reply.into_iter() { self.c.send(msg).unwrap(); }
                if r.done { self.handlers.remove(i); } else { i += 1; }
                if r.handled { return true; }
            }
            else { i += 1; }
        }
        false
    }

    /// Access and modify message handlers 
    pub fn msg_handlers(&mut self) -> &mut Vec<Box<MsgHandler>> { &mut self.handlers }
}

impl<'a> Iterator for ConnectionItems<'a> {
    type Item = ConnectionItem;
    fn next(&mut self) -> Option<ConnectionItem> {
        loop {
            if self.c.i.filter_cb.borrow().is_none() { panic!("Cannot call ConnectionItems iterator recursively"); }
            let i = self.c.i.pending_items.borrow_mut().pop_front();
            if let Some(ci) = i {
                if !self.process_handlers(&ci) { return Some(ci); }
            }

            match self.timeout_ms {
                Some(t) => {
                    let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.c.conn(), t as c_int) };

                    let p = mem::replace(&mut *self.c.i.filter_cb_panic.borrow_mut(), Ok(()));
                    if let Err(perr) = p { panic::resume_unwind(perr); }

                    if !self.c.i.pending_items.borrow().is_empty() { continue };
                    if r == 0 { return None; }
                    return Some(ConnectionItem::Nothing);
                }
                None => {
                    let r = unsafe { ffi::dbus_connection_dispatch(self.c.conn()) };

                    let p = mem::replace(&mut *self.c.i.filter_cb_panic.borrow_mut(), Ok(()));
                    if let Err(perr) = p { panic::resume_unwind(perr); }

                    if !self.c.i.pending_items.borrow().is_empty() { continue };
                    if r == ffi::DBusDispatchStatus::DataRemains { continue };
                    if r == ffi::DBusDispatchStatus::Complete { return None };
                    panic!("dbus_connection_dispatch failed");
                }
            }
        }
    }
}

/* Since we register callbacks with userdata pointers,
   we need to make sure the connection pointer does not move around.
   Hence this extra indirection. */
struct IConnection {
    conn: Cell<*mut ffi::DBusConnection>,
    pending_items: RefCell<LinkedList<ConnectionItem>>,
    watches: Option<Box<WatchList>>,

    filter_cb: RefCell<Option<Box<FnMut(&Connection, Message) -> bool>>>,
    filter_cb_panic: RefCell<thread::Result<()>>,
}

/// A D-Bus connection. Start here if you want to get on the D-Bus!
pub struct Connection {
    i: Box<IConnection>,
}

pub fn conn_handle(c: &Connection) -> *mut ffi::DBusConnection {
    c.i.conn.get()
}

extern "C" fn filter_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut c_void) -> ffi::DBusHandlerResult {

    let i: &IConnection = unsafe { mem::transmute(user_data) };
    let connref: panic::AssertUnwindSafe<&Connection> = unsafe { mem::transmute(&i) };
    if i.conn.get() != conn || i.filter_cb_panic.try_borrow().is_err() {
        // This should never happen, but let's be extra sure
        // process::abort(); ??
        return ffi::DBusHandlerResult::Handled;
    }
    if i.filter_cb_panic.borrow().is_err() {
        // We're in panic mode. Let's quit this ASAP
        return ffi::DBusHandlerResult::Handled;
    }

    let fcb = panic::AssertUnwindSafe(&i.filter_cb);
    let r = panic::catch_unwind(|| {
        let m = super::message::message_from_ptr(msg, true);
        let mut cb = fcb.borrow_mut().take().unwrap(); // Take the callback out while we call it.
        let r = cb(connref.0, m);
        let mut cb2 = fcb.borrow_mut(); // If the filter callback has not been replaced, put it back in.
        if cb2.is_none() { *cb2 = Some(cb) };
        r
    });

    match r {
        Ok(false) => ffi::DBusHandlerResult::NotYetHandled, 
        Ok(true) => ffi::DBusHandlerResult::Handled, 
        Err(e) => {
            *i.filter_cb_panic.borrow_mut() = Err(e);
            ffi::DBusHandlerResult::Handled
        }
    }
}

fn default_filter_callback(c: &Connection, m: Message) -> bool {
    let mtype = m.msg_type();
    let r = match mtype {
        MessageType::Signal => ConnectionItem::Signal(m),
        MessageType::MethodReturn => ConnectionItem::MethodReturn(m),
        MessageType::Error => ConnectionItem::MethodReturn(m),
        MessageType::MethodCall => ConnectionItem::MethodCall(m),
        _ => return false,
    };

    c.i.pending_items.borrow_mut().push_back(r);
    mtype == MessageType::Signal
}

extern "C" fn object_path_message_cb(_conn: *mut ffi::DBusConnection, _msg: *mut ffi::DBusMessage,
    _user_data: *mut c_void) -> ffi::DBusHandlerResult {
    /* Already pushed in filter_message_cb, so we just set the handled flag here to disable the 
       "default" handler. */
    ffi::DBusHandlerResult::Handled
}

extern "C" fn pending_call_cb<F: FnOnce(Message)>(pending: *mut ffi::DBusPendingCall, user_data: *mut c_void) {
    let message = unsafe { ffi::dbus_pending_call_steal_reply(pending) };
    assert!(!message.is_null());
    let message = super::message::message_from_ptr(message, false);

    let user_closure: *mut Option<Box<F>> = user_data as *mut Option<Box<F>>;
    let user_closure = unsafe { (*user_closure).take().unwrap() };
    (*user_closure)(message);
}

extern "C" fn pending_call_data_free_cb<F: FnOnce(Message)>(user_data: *mut c_void) {
    let user_closure: *mut Option<Box<F>> = user_data as *mut Option<Box<F>>;
    let user_closure = unsafe { Box::from_raw(user_closure) };
    drop(user_closure)
}

impl Connection {
    #[inline(always)]
    fn conn(&self) -> *mut ffi::DBusConnection {
        self.i.conn.get()
    }

    /// Creates a new D-Bus connection.
    pub fn get_private(bus: BusType) -> Result<Connection, Error> {
        let mut e = Error::empty();
        let conn = unsafe { ffi::dbus_bus_get_private(bus, e.get_mut()) };
        if conn == ptr::null_mut() {
            return Err(e)
        }
        let mut c = Connection { i: Box::new(IConnection {
            conn: Cell::new(conn),
            pending_items: RefCell::new(LinkedList::new()),
            watches: None,
            filter_cb: RefCell::new(Some(Box::new(default_filter_callback))),
            filter_cb_panic: RefCell::new(Ok(())),
        })};

        /* No, we don't want our app to suddenly quit if dbus goes down */
        unsafe { ffi::dbus_connection_set_exit_on_disconnect(conn, 0) };
        assert!(unsafe {
            ffi::dbus_connection_add_filter(c.conn(), Some(filter_message_cb as ffi::DBusCallback), mem::transmute(&*c.i), None)
        } != 0);

        let iconn: *const IConnection = &*c.i;
        c.i.watches = Some(WatchList::new(&c, Box::new(move |w| {
            let i: &IConnection = unsafe { mem::transmute(iconn) };
            i.pending_items.borrow_mut().push_back(ConnectionItem::WatchFd(w));
        })));

        Ok(c)
    }

    /// Sends a message over the D-Bus and waits for a reply.
    /// This is usually used for method calls.
    pub fn send_with_reply_and_block(&self, msg: Message, timeout_ms: i32) -> Result<Message, Error> {
        let mut e = Error::empty();
        let response = unsafe {
            ffi::dbus_connection_send_with_reply_and_block(self.conn(), super::message::get_message_ptr(&msg),
                timeout_ms as c_int, e.get_mut())
        };
        if response == ptr::null_mut() {
            return Err(e);
        }
        Ok(super::message::message_from_ptr(response, false))
    }

    /// Sends a message over the D-Bus without waiting. Useful for sending signals and method call replies.
    pub fn send(&self, msg: Message) -> Result<u32,()> {
        let mut serial = 0u32;
        let r = unsafe { ffi::dbus_connection_send(self.conn(), super::message::get_message_ptr(&msg), &mut serial) };
        if r == 0 { return Err(()); }
        unsafe { ffi::dbus_connection_flush(self.conn()) };
        Ok(serial)
    }

    /// Sends a message over the D-Bus without waiting, but calls the given closure when the reply is received.
    pub fn send_with_reply<'a, F: FnOnce(Message) + 'a>(&self, mut msg: Message, f: F) -> Result<(),()> {
        // Ensure allocation of a fresh serial, so that callbacks work as expected.
        super::message::message_set_serial(&mut msg, 0);

        let mut pc: *mut ffi::DBusPendingCall = ::std::ptr::null_mut();
        let r = unsafe {
            ffi::dbus_connection_send_with_reply(
                self.conn(),
                super::message::get_message_ptr(&msg),
                &mut pc,
                ffi::DBUS_TIMEOUT_INFINITE
            )
        };
        if pc.is_null() { return Err(()); }
        let pc = super::pending::pending_call_from_ptr(pc, false);
        if r == 0 { return Err(()); }

        let callback: Box<Option<Box<F>>> = Box::new(Some(Box::new(f)));
        let callback = Box::into_raw(callback) as *mut c_void;
        let r = unsafe {
            ffi::dbus_pending_call_set_notify(
                super::pending::get_pending_call_ptr(&pc),
                Some(pending_call_cb::<F>),
                callback,
                Some(pending_call_data_free_cb::<F>)
            )
        };
        if r == 0 {
            drop(unsafe { Box::from_raw(callback) });
            return Err(());
        }
        Ok(())
    }

    /// Get the connection's unique name.
    pub fn unique_name(&self) -> String {
        let c = unsafe { ffi::dbus_bus_get_unique_name(self.conn()) };
        c_str_to_slice(&c).unwrap_or("").to_string()
    }

    /// Check if there are new incoming events
    pub fn iter(&self, timeout_ms: i32) -> ConnectionItems {
        ConnectionItems {
            c: self,
            timeout_ms: Some(timeout_ms),
            handlers: Vec::new(),
        }
    }

    /// Register an object path.
    pub fn register_object_path(&self, path: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let p = to_c_str(path);
        let vtable = ffi::DBusObjectPathVTable {
            unregister_function: None,
            message_function: Some(object_path_message_cb as ffi::DBusCallback),
            dbus_internal_pad1: None,
            dbus_internal_pad2: None,
            dbus_internal_pad3: None,
            dbus_internal_pad4: None,
        };
        let r = unsafe {
            let user_data: *mut c_void = mem::transmute(&*self.i);
            ffi::dbus_connection_try_register_object_path(self.conn(), p.as_ptr(), &vtable, user_data, e.get_mut())
        };
        if r == 0 { Err(e) } else { Ok(()) }
    }

    /// Unregister an object path.
    pub fn unregister_object_path(&self, path: &str) {
        let p = to_c_str(path);
        let r = unsafe { ffi::dbus_connection_unregister_object_path(self.conn(), p.as_ptr()) };
        if r == 0 { panic!("Out of memory"); }
    }

    /// List registered object paths.
    pub fn list_registered_object_paths(&self, path: &str) -> Vec<String> {
        let p = to_c_str(path);
        let mut clist: *mut *mut c_char = ptr::null_mut();
        let r = unsafe { ffi::dbus_connection_list_registered(self.conn(), p.as_ptr(), &mut clist) };
        if r == 0 { panic!("Out of memory"); }
        let mut v = Vec::new();
        let mut i = 0;
        loop {
            let s = unsafe {
                let citer = clist.offset(i);
                if *citer == ptr::null_mut() { break };
                mem::transmute(citer)
            };
            v.push(format!("{}", c_str_to_slice(s).unwrap()));
            i += 1;
        }
        unsafe { ffi::dbus_free_string_array(clist) };
        v
    }

    /// Register a name.
    pub fn register_name(&self, name: &str, flags: u32) -> Result<RequestNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_request_name(self.conn(), n.as_ptr(), flags, e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { mem::transmute(r) }) }
    }

    /// Release a name.
    pub fn release_name(&self, name: &str) -> Result<ReleaseNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_release_name(self.conn(), n.as_ptr(), e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { mem::transmute(r) }) }
    }

    /// Add a match rule to match messages on the message bus.
    ///
    /// See the `unity_focused_window` example for how to use this to catch signals.
    /// (The syntax of the "rule" string is specified in the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#message-bus-routing-match-rules).)
    pub fn add_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_add_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
    }

    /// Remove a match rule to match messages on the message bus.
    pub fn remove_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_remove_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
    }

    /// Async I/O: Get an up-to-date list of file descriptors to watch.
    ///
    /// See the `Watch` struct for an example.
    pub fn watch_fds(&self) -> Vec<Watch> {
        self.i.watches.as_ref().unwrap().get_enabled_fds()
    }

    /// Async I/O: Call this function whenever you detected an event on the Fd,
    /// Flags are a set of WatchEvent bits.
    /// The returned iterator will return pending items only, never block for new events.
    ///
    /// See the `Watch` struct for an example.
    pub fn watch_handle(&self, fd: RawFd, flags: c_uint) -> ConnectionItems {
        self.i.watches.as_ref().unwrap().watch_handle(fd, flags);
        ConnectionItems { c: self, timeout_ms: None, handlers: Vec::new() }
    }


    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_path<'a, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'a self, dest: D, path: P, timeout_ms: i32) ->
        ConnPath<'a, &'a Connection> {
        ConnPath { conn: self, dest: dest.into(), path: path.into(), timeout: timeout_ms }
    }

    /// Replace the default message callback.
    ///
    /// By default, when you call ConnectionItems::next, all relevant incoming messages
    /// are returned through the ConnectionItems iterator, and 
    /// irrelevant messages are passed on to libdbus's default handler.
    /// If you need to customize this behaviour (i e, to handle all incoming messages yourself),
    /// you can set this message callback yourself. A few caveats apply:
    ///
    /// Return true from the callback to disable libdbus's internal handling of the message, or
    /// false to allow it.
    ///
    /// Don't call ConnectionItems::next from inside the message callback (you'll likely get a panic).
    ///
    /// If your message callback panics, ConnectionItems::next will panic, too.  
    pub fn set_message_callback<F: 'static + FnMut(&Connection, Message) -> bool>(&self, f: F) {
        *self.i.filter_cb.borrow_mut() = Some(Box::new(f));
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            ffi::dbus_connection_close(self.conn());
            ffi::dbus_connection_unref(self.conn());
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "D-Bus Connection({})", self.unique_name())
    }
}

/// A trait for handling incoming messages. To use in combination with `ConnectionItems`.
pub trait MsgHandler {
    /// The default handle_ci function calls handle_msg, so implement the one that fits you better.
    fn handle_ci(&mut self, ci: &ConnectionItem) -> Option<MsgHandlerResult> {
        match *ci {
            ConnectionItem::MethodReturn(ref msg) => self.handle_msg(msg),
            ConnectionItem::Signal(ref msg) => self.handle_msg(msg),
            ConnectionItem::MethodCall(ref msg) => self.handle_msg(msg),
            ConnectionItem::WatchFd(_) => None,
            ConnectionItem::Nothing => None,
        }
    }

    /// Called for all incoming method calls, method returns, and signals.
    fn handle_msg(&mut self, _msg: &Message) -> Option<MsgHandlerResult> { None }
}

/// The result from MsgHandler::handle.
///
/// WIP: field names are still unstable
#[derive(Debug, Default)]
pub struct MsgHandlerResult {
    /// Indicates that the message has been dealt with and should not be processed further.
    pub handled: bool,
    /// Indicates that this MsgHandler no longer wants to receive messages and can be removed.
    pub done: bool,
    /// Messages to send (e g, a reply to a method call)
    pub reply: Vec<Message>,
}

#[test]
fn message_reply() {
    use std::{cell, rc};
    let c = Connection::get_private(BusType::Session).unwrap();
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let quit = rc::Rc::new(cell::Cell::new(false));
    let quit2 = quit.clone();
    c.send_with_reply(m, move |result| {
        let r = result;
        let _: ::arg::Array<&str, _>  = r.get1().unwrap();
        quit2.set(true);
    }).unwrap();
    for _ in c.iter(1000) { if quit.get() { return; } }
    assert!(false);
}

