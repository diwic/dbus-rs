use super::{Error, ffi, libc, to_c_str, c_str_to_slice, Watch, Message, BusName, Path, ConnPath};
use super::{RequestNameReply, ReleaseNameReply, BusType, WatchEvent};
use super::watch::WatchList;
use std::{fmt, mem, ptr};
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
}

impl<'a> Iterator for ConnectionItems<'a> {
    type Item = ConnectionItem;
    fn next(&mut self) -> Option<ConnectionItem> {
        loop {
            let i = self.c.i.pending_items.borrow_mut().pop_front();
            if i.is_some() { return i; }

            match self.timeout_ms {
                Some(t) => {
                    let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.c.conn(), t as c_int) };
                    if !self.c.i.pending_items.borrow().is_empty() { continue };
                    if r == 0 { return None; }
                    return Some(ConnectionItem::Nothing);
                }
                None => {
                    let r = unsafe { ffi::dbus_connection_dispatch(self.c.conn()) };
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

    let m = super::message::message_from_ptr(msg, true);
    let i: &IConnection = unsafe { mem::transmute(user_data) };
    assert!(i.conn.get() == conn);

    let mtype: ffi::DBusMessageType = unsafe { mem::transmute(ffi::dbus_message_get_type(msg)) };
    let r = match mtype {
        ffi::DBusMessageType::Signal => {
            i.pending_items.borrow_mut().push_back(ConnectionItem::Signal(m));
            ffi::DBusHandlerResult::Handled
        }
        ffi::DBusMessageType::MethodReturn => {
            i.pending_items.borrow_mut().push_back(ConnectionItem::MethodReturn(m));
            ffi::DBusHandlerResult::NotYetHandled
        }
        _ => ffi::DBusHandlerResult::NotYetHandled,
    };

    r
}

extern "C" fn object_path_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut c_void) -> ffi::DBusHandlerResult {

    let m = super::message::message_from_ptr(msg, true);
    let i: &IConnection = unsafe { mem::transmute(user_data) };
    assert!(i.conn.get() == conn);
    i.pending_items.borrow_mut().push_back(ConnectionItem::MethodCall(m));
    ffi::DBusHandlerResult::Handled
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

    /// Sends a message over the D-Bus and call the callback with the result when it is received.
    /// This is usually used for asynchronous method calls.
    pub fn send_with_reply<F: Fn(Result<Message, Error>)>(&self, msg: Message, pending_return: F, timeout_ms: i32) -> Result<(), ()> {
        let mut pending_return_ptr = ptr::null_mut() as *mut ffi::DBusPendingCall;
        let response = unsafe {
            ffi::dbus_connection_send_with_reply(self.conn(), super::message::get_message_ptr(&msg),
                &mut pending_return_ptr, timeout_ms as c_int)
        };
        if response == 0 {
            return Err(());
        }
        let function: Box<Box<Fn(Result<Message, Error>)>> = Box::new(Box::new(pending_return));
        let user_data: *mut c_void = Box::into_raw(function) as *mut _;
        let trampoline: ffi::DBusPendingCallNotifyFunction = unsafe {
            mem::transmute(dbus_pending_call_set_notify_trampoline as usize)
        };
        let response = unsafe {
            ffi::dbus_pending_call_set_notify(pending_return_ptr, trampoline, user_data, None)
        };
        unsafe { ffi::dbus_pending_call_unref(pending_return_ptr) };
        if response == 0 {
            return Err(());
        }
        Ok(())
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
        ConnectionItems { c: self, timeout_ms: None }
    }


    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_path<'a, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'a self, dest: D, path: P, timeout_ms: i32) ->
        ConnPath<'a, &'a Connection> {
        ConnPath { conn: self, dest: dest.into(), path: path.into(), timeout: timeout_ms }
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


unsafe extern "C" fn dbus_pending_call_set_notify_trampoline(pending: *mut ffi::DBusPendingCall, user_data: *mut c_void) {
    let response = ffi::dbus_pending_call_steal_reply(pending);
    debug_assert_ne!(response, ptr::null_mut());
    let message = super::message::message_from_ptr(response, false);
    let f: &Box<Fn(Result<Message, Error>) + 'static> = &*(user_data as *const _);
    f(Ok(message));
}
