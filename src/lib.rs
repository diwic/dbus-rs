#![feature(unsafe_destructor, alloc, core, libc, std_misc)]

extern crate libc;

pub use ffi::DBusBusType as BusType;
pub use ffi::DBusNameFlag as NameFlag;
pub use ffi::DBusRequestNameReply as RequestNameReply;
pub use ffi::DBusReleaseNameReply as ReleaseNameReply;
pub use ffi::DBusMessageType as MessageType;

pub use message::{Message, MessageItem};
pub use prop::PropHandler;
pub use prop::Props;
pub use objpath::{ObjectPath, Interface, Property, Method, Signal, MethodHandler, MethodResult, Argument};

pub type TypeSig<'a> = std::string::CowString<'a>;

use std::ffi as cstr;
use std::ffi::CString;
use std::ptr::{self, PtrExt};
use std::collections::LinkedList;
use std::cell::{Cell, RefCell};

mod ffi;
mod message;
mod prop;
mod objpath;

static INITDBUS: std::sync::Once = std::sync::ONCE_INIT;

fn init_dbus() {
    INITDBUS.call_once(|| {
        if unsafe { ffi::dbus_threads_init_default() } == 0 {
            panic!("Out of memory when trying to initialize D-Bus library!");
        }
    });
}


pub struct Error {
    e: ffi::DBusError,
}

unsafe impl Send for Error {}

fn c_str_to_slice(c: & *const libc::c_char) -> Option<&str> {
    if *c == ptr::null() { None }
    else { std::str::from_utf8( unsafe { cstr::c_str_to_bytes(c) }).ok() }
}

fn to_c_str<S: Str>(n: S) -> CString { CString::from_slice(n.as_slice().as_bytes()) }

impl Error {

    pub fn new(e: ffi::DBusError) -> Error {
        Error { e: e }
    }

    pub fn new_custom(name: &str, message: &str) -> Error {
        let n = to_c_str(name);
        let m = to_c_str(message.replace("%","%%"));
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

    pub fn get(&self) -> &ffi::DBusError { &self.e }

    pub fn name(&self) -> Option<&str> {
        c_str_to_slice(&self.e.name)
    }

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
        self.message().map(|x| write!(f, "{:?}", x.to_string()));
        Ok(())
    }
}

#[derive(Debug)]
pub enum ConnectionItem {
    Nothing,
    MethodCall(Message),
    Signal(Message),
}

pub struct ConnectionItems<'a> {
    c: &'a Connection,
    timeout_ms: i32,
}

impl<'a> Iterator for ConnectionItems<'a> {
    type Item = ConnectionItem;
    fn next(&mut self) -> Option<ConnectionItem> {
        loop {
            let i = self.c.i.pending_items.borrow_mut().pop_front();
            if i.is_some() { return i; }

            let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.c.conn(), self.timeout_ms as libc::c_int) };
            if !self.c.i.pending_items.borrow().is_empty() { continue };

            if r == 0 { return None; }
            return Some(ConnectionItem::Nothing);
        }
    }
}

/* Since we register callbacks with userdata pointers,
   we need to make sure the connection pointer does not move around.
   Hence this extra indirection. */
struct IConnection {
    conn: Cell<*mut ffi::DBusConnection>,
    pending_items: RefCell<LinkedList<ConnectionItem>>,
}

pub struct Connection {
    i: Box<IConnection>,
}

extern "C" fn filter_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut libc::c_void) -> ffi::DBusHandlerResult {

    let m = message::message_from_ptr(msg, true);
    let c = Connection { i: unsafe { std::mem::transmute(user_data) } };
    assert_eq!(c.conn(), conn);

    let mtype: ffi::DBusMessageType = unsafe { std::mem::transmute(ffi::dbus_message_get_type(msg)) };
    let r = match mtype {
        ffi::DBusMessageType::Signal => {
            c.i.pending_items.borrow_mut().push_back(ConnectionItem::Signal(m));
            ffi::DBusHandlerResult::Handled
        }
        _ => ffi::DBusHandlerResult::NotYetHandled,
    };

    unsafe { std::mem::forget(c) };
    r
}

extern "C" fn object_path_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut libc::c_void) -> ffi::DBusHandlerResult {

    let m = message::message_from_ptr(msg, true);
    let c = Connection { i: unsafe { std::mem::transmute(user_data) } };
    assert!(c.conn() == conn);
    c.i.pending_items.borrow_mut().push_back(ConnectionItem::MethodCall(m));
    unsafe { std::mem::forget(c) };
    ffi::DBusHandlerResult::Handled
}

impl Connection {

    #[inline(always)]
    fn conn(&self) -> *mut ffi::DBusConnection {
        self.i.conn.get()
    }

    pub fn get_private(bus: BusType) -> Result<Connection, Error> {
        let mut e = Error::empty();
        let conn = unsafe { ffi::dbus_bus_get_private(bus, e.get_mut()) };
        if conn == ptr::null_mut() {
            return Err(e)
        }
        let c = Connection { i: Box::new(IConnection { conn: Cell::new(conn), pending_items: RefCell::new(LinkedList::new()) })};

        /* No, we don't want our app to suddenly quit if dbus goes down */
        unsafe { ffi::dbus_connection_set_exit_on_disconnect(conn, 0) };
        assert!(unsafe {
            ffi::dbus_connection_add_filter(c.conn(), Some(filter_message_cb as ffi::DBusCallback), std::mem::transmute(&*c.i), None)
        } != 0);
        Ok(c)
    }

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

    pub fn send(&self, msg: Message) -> Result<(),()> {
        let r = unsafe { ffi::dbus_connection_send(self.conn(), message::get_message_ptr(&msg), ptr::null_mut()) };
        if r == 0 { return Err(()); }
        unsafe { ffi::dbus_connection_flush(self.conn()) };
        Ok(())
    }

    pub fn unique_name(&self) -> String {
        let c = unsafe { ffi::dbus_bus_get_unique_name(self.conn()) };
        c_str_to_slice(&c).unwrap_or("").to_string()
    }

    pub fn iter(&self, timeout_ms: i32) -> ConnectionItems {
        ConnectionItems {
            c: self,
            timeout_ms: timeout_ms,
        }
    }

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
            let user_data: *mut libc::c_void = std::mem::transmute(&*self.i);
            ffi::dbus_connection_try_register_object_path(self.conn(), p.as_ptr(), &vtable, user_data, e.get_mut())
        };
        if r == 0 { Err(e) } else { Ok(()) }
    }

    pub fn unregister_object_path(&self, path: &str) {
        let p = to_c_str(path);
        let r = unsafe { ffi::dbus_connection_unregister_object_path(self.conn(), p.as_ptr()) };
        if r == 0 { panic!("Out of memory"); }
    }

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
                std::mem::transmute(citer)
            };
            v.push(format!("{}", c_str_to_slice(s).unwrap()));
            i += 1;
        }
        unsafe { ffi::dbus_free_string_array(clist) };
        v
    }

    pub fn register_name(&self, name: &str, flags: u32) -> Result<RequestNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_request_name(self.conn(), n.as_ptr(), flags, e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { std::mem::transmute(r) }) }
    }

    pub fn release_name(&self, name: &str) -> Result<ReleaseNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_release_name(self.conn(), n.as_ptr(), e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { std::mem::transmute(r) }) }
    }

    pub fn add_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_add_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
    }

    pub fn remove_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_remove_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
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
        let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
        let mut r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
    }

    #[test]
    fn message_namehasowner() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "NameHasOwner").unwrap();
        m.append_items(&[MessageItem::Str("org.freedesktop.DBus".to_string())]);
        let mut r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        assert_eq!(reply, vec!(MessageItem::Bool(true)));
    }

    #[test]
    fn object_path() {
        use  std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let thread = ::std::thread::Thread::scoped(move || {
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
        let m = Message::new_method_call(&*n, "/hello", "com.example.hello", "Hello").unwrap();
        println!("Sending...");
        let mut r = c.send_with_reply_and_block(m, 8000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        thread.join();

    }

    #[test]
    fn register_name() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let n = format!("com.example.hello.test.register_name");
        assert_eq!(c.register_name(&*n, NameFlag::ReplaceExisting as u32).unwrap(), RequestNameReply::PrimaryOwner);
        assert_eq!(c.release_name(&*n).unwrap(), ReleaseNameReply::Released);
    }

    #[test]
    fn signal() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let iface = "com.example.signaltest";
        let mstr = format!("interface='{}',member='ThisIsASignal'", iface);
        c.add_match(&*mstr).unwrap();
        let m = Message::new_signal("/mysignal", iface, "ThisIsASignal").unwrap();
        let uname = c.unique_name();
        c.send(m).unwrap();
        for n in c.iter(1000) {
            match n {
                ConnectionItem::Signal(s) => {
                    let (_, p, i, m) = s.headers();
                    match (&*p.unwrap(), &*i.unwrap(), &*m.unwrap()) {
                        ("/mysignal", "com.example.signaltest", "ThisIsASignal") => {
                            assert_eq!(s.sender().unwrap(), uname);
                            break;
                        },
                        (_, _, _) => println!("Other signal: {:?}", s.headers()),
                    }
                }
                _ => {},
            }
        }
        c.remove_match(&*mstr).unwrap();
    }

}
