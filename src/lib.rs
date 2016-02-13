//! D-Bus bindings for Rust
//!
//! [D-Bus](http://dbus.freedesktop.org/) is a message bus, and is mainly used in Linux
//! for communication between processes. It is present by default on almost every
//! Linux distribution out there, and runs in two instances - one per session, and one
//! system-wide.
//!
//! See the examples directory for some examples to get you started.

// #![warn(missing_docs)]

extern crate libc;

pub use ffi::DBusBusType as BusType;
pub use ffi::DBusNameFlag as NameFlag;
pub use ffi::DBusRequestNameReply as RequestNameReply;
pub use ffi::DBusReleaseNameReply as ReleaseNameReply;
pub use ffi::DBusMessageType as MessageType;
pub use ffi::DBusWatchEvent as WatchEvent;

pub use message::{Message, MessageItem, FromMessageItem, OwnedFd, ArrayError};
pub use prop::PropHandler;
pub use prop::Props;
pub use watch::Watch;

/// A TypeSig describes the type of a MessageItem.
pub type TypeSig<'a> = std::borrow::Cow<'a, str>;

use std::ffi::{CString, CStr};
use std::ptr::{self};
use std::collections::LinkedList;
use std::cell::{Cell, RefCell};
use std::mem;
use std::os::unix::io::RawFd;

#[allow(missing_docs)]
mod ffi;
mod message;
mod prop;
mod objpath;
mod watch;
mod msgarg;

mod strings;
pub use strings::{Signature, Path, Interface, Member, ErrorName, BusName};

/// Types and traits for easily appending a message with arguments.
///
/// Using this module to append argument to a message should be faster than
/// using MessageItem, especially when large arrays need to be appended.
/// It also encodes more of D-Bus restrictions into Rust's type system, so
/// trying to append anything that D-Bus would not allow should result in a
/// compile-time error.
///
/// **Append a**:
///
/// `bool, u8, u16, u32, u64, i16, i32, i64, f64` - the corresponding D-Bus basic type
///
/// `&str` - a D-Bus string. TODO: If the string contains null characters, it will be cropped
/// to only include the data before the null character. (This allows for skipping an
/// allocation by writing a string literal which ends with a null character.)
///
/// `&[T] where T: Append` - a D-Bus array. Note: can use an efficient fast-path in case of 
/// T being an FixedArray type.
///
/// `Array<T, I> where I: Iterator<Item=T>, T: Append` - a D-Bus array, maximum flexibility.
///
/// `Variant<T> where T: Append` - a D-Bus variant.
///
/// `(T1, T2) where T1: Append, T2: Append` - tuples are D-Bus structs. Implemented up to 12.
///
/// `Dict<T1, T2, I> where I: Iterator<Item=(T1, T2)>, T1: Append, T2: Append` - A D-Bus dict (array of dict entries).
///
/// Not implemented yet: Fd, ObjectPath and Signature types. And to get arguments just as ergonomic
/// as this module appends them.
pub mod arg {
    pub use msgarg::{Arg, FixedArray, Get, DictKey, Append};
    pub use msgarg::{Iter, IterAppend, Array, Variant, Dict};
}


/// Contains functionality for the "server" of a D-Bus object. A remote application can
/// introspect this object and call methods on it.
/// Deprecated - use the `tree` module instead.
pub mod obj {
    pub use objpath::{ObjectPath, Interface, Property, Signal, Argument};
    pub use objpath::{Method, MethodHandler, MethodResult};
    pub use objpath::{PropertyROHandler, PropertyRWHandler, PropertyWOHandler, PropertyGetResult, PropertySetResult};
}

mod methoddisp;

/// Contains functionality for dispatching methods on a D-Bus "server".
/// Supersedes the `obj` module. Properties are somewhat still WIP,
/// but should in any case be better than `obj` already.
///
/// # Example
/// ```
/// use dbus::{tree, Connection, BusType};
/// let f = tree::Factory::new_fn();
/// /* Add a method returning "Thanks!" on interface "com.example.dbus.rs"
///    on object path "/example". */
/// let t = f.tree().add(f.object_path("/example").introspectable()
///     .add(f.interface("com.example.dbus.rs")
///         .add_m(f.method("CallMe", |m,_,_| {
///             Ok(vec!(m.method_return().append("Thanks!"))) }
///         ).out_arg("s"))
/// ));
///
/// let c = Connection::get_private(BusType::Session).unwrap();
/// t.set_registered(&c, true).unwrap();
/// /* Run forever */
/// // for _ in t.run(&c, c.iter(1000)) {}
/// ```

pub mod tree {
    pub use methoddisp::{Factory, Tree, TreeServer, ObjectPath, Interface, Signal};
    pub use methoddisp::{Property, EmitsChangedSignal, Access};
    pub use methoddisp::{Method, MethodErr, MethodResult, Argument};
    pub use methoddisp::{MethodFn, MethodFnMut, MethodSync};
}

static INITDBUS: std::sync::Once = std::sync::ONCE_INIT;

fn init_dbus() {
    INITDBUS.call_once(|| {
        if unsafe { ffi::dbus_threads_init_default() } == 0 {
            panic!("Out of memory when trying to initialize D-Bus library!");
        }
    });
}

/// D-Bus Error wrapper
pub struct Error {
    e: ffi::DBusError,
}

unsafe impl Send for Error {}

fn c_str_to_slice(c: & *const libc::c_char) -> Option<&str> {
    if *c == ptr::null() { None }
    else { std::str::from_utf8( unsafe { CStr::from_ptr(*c).to_bytes() }).ok() }
}

fn to_c_str(n: &str) -> CString { CString::new(n.as_bytes()).unwrap() }

impl Error {

    /// Create a new custom D-Bus Error.
    pub fn new_custom(name: &str, message: &str) -> Error {
        let n = to_c_str(name);
        let m = to_c_str(&message.replace("%","%%"));
        let mut e = Error::empty();

        unsafe { ffi::dbus_set_error(e.get_mut(), n.as_ptr(), m.as_ptr()) };
        e
    }

    fn empty() -> Error {
        init_dbus();
        let mut e = ffi::DBusError {
            name: ptr::null(),
            message: ptr::null(),
            dummy: 0,
            padding1: ptr::null()
        };
        unsafe { ffi::dbus_error_init(&mut e); }
        Error{ e: e }
    }

    /// Error name/type, e g 'org.freedesktop.DBus.Error.Failed'
    pub fn name(&self) -> Option<&str> {
        c_str_to_slice(&self.e.name)
    }

    /// Custom message, e g 'Could not find a matching object path'
    pub fn message(&self) -> Option<&str> {
        c_str_to_slice(&self.e.message)
    }

    fn get_mut(&mut self) -> &mut ffi::DBusError { &mut self.e }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { ffi::dbus_error_free(&mut self.e); }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "D-Bus error: {} ({})", self.message().unwrap_or(""),
            self.name().unwrap_or(""))
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str { "D-Bus error" }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(),std::fmt::Error> {
        if let Some(x) = self.message() {
             write!(f, "{:?}", x.to_string())
        } else { Ok(()) }
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
                    let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.c.conn(), t as libc::c_int) };
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
    watches: Option<Box<watch::WatchList>>,
}

/// A D-Bus connection. Start here if you want to get on the D-Bus!
pub struct Connection {
    i: Box<IConnection>,
}

extern "C" fn filter_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut libc::c_void) -> ffi::DBusHandlerResult {

    let m = message::message_from_ptr(msg, true);
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
    user_data: *mut libc::c_void) -> ffi::DBusHandlerResult {

    let m = message::message_from_ptr(msg, true);
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
        c.i.watches = Some(watch::WatchList::new(&c, Box::new(move |w| {
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
            ffi::dbus_connection_send_with_reply_and_block(self.conn(), message::get_message_ptr(&msg),
                timeout_ms as libc::c_int, e.get_mut())
        };
        if response == ptr::null_mut() {
            return Err(e);
        }
        Ok(message::message_from_ptr(response, false))
    }

    /// Sends a message over the D-Bus without waiting. Useful for sending signals and method call replies.
    pub fn send(&self, msg: Message) -> Result<u32,()> {
        let mut serial = 0u32;
        let r = unsafe { ffi::dbus_connection_send(self.conn(), message::get_message_ptr(&msg), &mut serial) };
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
            let user_data: *mut libc::c_void = mem::transmute(&*self.i);
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
        let mut clist: *mut *mut libc::c_char = ptr::null_mut();
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
    pub fn watch_fds(&self) -> Vec<Watch> {
        self.i.watches.as_ref().unwrap().get_enabled_fds()
    }

    /// Async I/O: Call this function whenever you detected an event on the Fd,
    /// Flags are a set of WatchEvent bits.
    /// The returned iterator will return pending items only, never block for new events.
    pub fn watch_handle(&self, fd: RawFd, flags: libc::c_uint) -> ConnectionItems {
        self.i.watches.as_ref().unwrap().watch_handle(fd, flags);
        ConnectionItems { c: self, timeout_ms: None }
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

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "D-Bus Connection({})", self.unique_name())
    }
}

#[cfg(test)]
mod test {
    use super::{Connection, Message, BusType, MessageItem, ConnectionItem, NameFlag,
        RequestNameReply, ReleaseNameReply};

    #[test]
    fn connection() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let n = c.unique_name();
        assert!(n.starts_with(":1."));
        println!("Connected to DBus, unique name: {}", n);
    }

    #[test]
    fn invalid_message() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let m = Message::new_method_call("foo.bar", "/", "foo.bar", "FooBar").unwrap();
        let e = c.send_with_reply_and_block(m, 2000).err().unwrap();
        assert!(e.name().unwrap() == "org.freedesktop.DBus.Error.ServiceUnknown");
    }

    #[test]
    fn message_listnames() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let m = Message::method_call(&"org.freedesktop.DBus".into(), &"/".into(),
            &"org.freedesktop.DBus".into(), &"ListNames".into());
        let r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
    }

    #[test]
    fn message_namehasowner() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "NameHasOwner").unwrap();
        m.append_items(&[MessageItem::Str("org.freedesktop.DBus".to_string())]);
        let r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        assert_eq!(reply, vec!(MessageItem::Bool(true)));
    }

    #[test]
    fn object_path() {
        use  std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let thread = ::std::thread::spawn(move || {
            let c = Connection::get_private(BusType::Session).unwrap();
            c.register_object_path("/hello").unwrap();
            // println!("Waiting...");
            tx.send(c.unique_name()).unwrap();
            for n in c.iter(1000) {
                // println!("Found message... ({})", n);
                match n {
                    ConnectionItem::MethodCall(ref m) => {
                        let reply = Message::new_method_return(m).unwrap();
                        c.send(reply).unwrap();
                        break;
                    }
                    _ => {}
                }
            }
            c.unregister_object_path("/hello");
        });

        let c = Connection::get_private(BusType::Session).unwrap();
        let n = rx.recv().unwrap();
        let m = Message::new_method_call(&n, "/hello", "com.example.hello", "Hello").unwrap();
        println!("Sending...");
        let r = c.send_with_reply_and_block(m, 8000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        thread.join().unwrap();

    }

    #[test]
    fn register_name() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let n = format!("com.example.hello.test.register_name");
        assert_eq!(c.register_name(&n, NameFlag::ReplaceExisting as u32).unwrap(), RequestNameReply::PrimaryOwner);
        assert_eq!(c.release_name(&n).unwrap(), ReleaseNameReply::Released);
    }

    #[test]
    fn signal() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let iface = "com.example.signaltest";
        let mstr = format!("interface='{}',member='ThisIsASignal'", iface);
        c.add_match(&mstr).unwrap();
        let m = Message::new_signal("/mysignal", iface, "ThisIsASignal").unwrap();
        let uname = c.unique_name();
        c.send(m).unwrap();
        for n in c.iter(1000) {
            match n {
                ConnectionItem::Signal(s) => {
                    let (_, p, i, m) = s.headers();
                    match (&*p.unwrap(), &*i.unwrap(), &*m.unwrap()) {
                        ("/mysignal", "com.example.signaltest", "ThisIsASignal") => {
                            assert_eq!(&*s.sender().unwrap(), &*uname);
                            break;
                        },
                        (_, _, _) => println!("Other signal: {:?}", s.headers()),
                    }
                }
                _ => {},
            }
        }
        c.remove_match(&mstr).unwrap();
    }

    #[test]
    fn watch() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut d = c.watch_fds();
        assert!(d.len() > 0);
        println!("Fds to watch: {:?}", d);
        for n in c.iter(1000) {
            match n {
                ConnectionItem::WatchFd(w) => {
                    assert!(w.readable() || w.writable());
                    assert!(d.contains(&w));
                    d.retain(|x| *x != w);
                    if d.len() == 0 { break };
                }
                _ => {},
            }
        }
    }
}
