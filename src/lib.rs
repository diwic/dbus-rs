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
pub use connection::{Connection, ConnectionItems, ConnectionItem};
pub use prop::PropHandler;
pub use prop::Props;
pub use watch::Watch;

/// A TypeSig describes the type of a MessageItem.
pub type TypeSig<'a> = std::borrow::Cow<'a, str>;

use std::ffi::{CString, CStr};
use std::ptr;
use std::os::raw::c_char;

#[allow(missing_docs)]
mod ffi;
mod message;
mod prop;
mod objpath;
mod watch;
mod connection;

mod strings;
pub use strings::{Signature, Path, Interface, Member, ErrorName, BusName};

/// Types and traits for easily getting a message's arguments, or appening a message with arguments.
///
/// Using this module should be faster than
/// using MessageItem, especially when large arrays need to be appended.
/// It also encodes more of D-Bus restrictions into Rust's type system, so
/// trying to append anything that D-Bus would not allow should result in a
/// compile-time error.
///
/// A message has `get1`, `get2` etc, and `append1`, `append2` etc, which is your
/// starting point into this module's types. 
///
/// **Append a**:
///
/// `bool, u8, u16, u32, u64, i16, i32, i64, f64` - the corresponding D-Bus basic type
///
/// `&str` - a D-Bus string. D-Bus strings do not allow null characters, so 
/// if the string contains null characters, it will be cropped
/// to only include the data before the null character. (Tip: This allows for skipping an
/// allocation by writing a string literal which ends with a null character.)
///
/// `&[T] where T: Append` - a D-Bus array. Note: can use an efficient fast-path in case of 
/// T being an FixedArray type.
///
/// `Array<T, I> where T: Append, I: Iterator<Item=T>` - a D-Bus array, maximum flexibility.
///
/// `Variant<T> where T: Append` - a D-Bus variant.
///
/// `(T1, T2) where T1: Append, T2: Append` - tuples are D-Bus structs. Implemented up to 12.
///
/// `Dict<K, V, I> where K: Append + DictKey, V: Append, I: Iterator<Item=(&K, &V)>` - A D-Bus dict (array of dict entries).
///
/// `ObjectPath` - a D-Bus object path.
///
/// `Signature` - a D-Bus signature.
///
/// `OwnedFd` - shares the file descriptor with the remote side.
///
/// **Get a**:
///
/// `bool, u8, u16, u32, u64, i16, i32, i64, f64` - the corresponding D-Bus basic type
///
/// `&str`, `&CStr` - a D-Bus string. D-Bus strings are always UTF-8 and do not contain null characters.
///
/// `&[T] where T: FixedArray` - a D-Bus array of integers or f64.
///
/// `Array<T, Iter> where T: Get` - a D-Bus array, maximum flexibility. Implements Iterator so you can easily
/// collect it into, e g, a `Vec`.
///
/// `Variant<T> where T: Get` - a D-Bus variant. Use this type of Variant if you know the inner type.
///
/// `Variant<Iter>` - a D-Bus variant. This type of Variant allows you to examine the inner type.
///
/// `(T1, T2) where T1: Get, T2: Get` - tuples are D-Bus structs. Implemented up to 12.
///
/// `Dict<K, V, Iter> where K: Get + DictKey, V: Get` - A D-Bus dict (array of dict entries). Implements Iterator so you can easily
/// collect it into, e g, a `HashMap`.
///
/// `ObjectPath` - a D-Bus object path.
///
/// `Signature` - a D-Bus signature.
///
/// `OwnedFd` - a file descriptor sent from the remote side.
///

pub mod arg;


/// Contains functionality for the "server" of a D-Bus object. A remote application can
/// introspect this object and call methods on it.
/// Deprecated - use the `tree` module instead.
pub mod obj {
    pub use objpath::{ObjectPath, Interface, Property, Signal, Argument};
    pub use objpath::{Method, MethodHandler, MethodResult};
    pub use objpath::{PropertyROHandler, PropertyRWHandler, PropertyWOHandler, PropertyGetResult, PropertySetResult};
}

pub mod tree;

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

fn c_str_to_slice(c: & *const c_char) -> Option<&str> {
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
