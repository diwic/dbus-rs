//! D-Bus bindings for Rust
//!
//! [D-Bus](http://dbus.freedesktop.org/) is a message bus, and is mainly used in Linux
//! for communication between processes. It is present by default on almost every
//! Linux distribution out there, and runs in two instances - one per session, and one
//! system-wide.
//!
//! In addition to the API documentation, which you're currently reading, you might want to
//! look in the examples directory, which contains many examples and an argument guide.
//! README.md also contain a few quick "getting started" examples.
//!
//! In addition to this crate, there are two companion crates, dbus-codegen for generating Rust
//! code from D-Bus introspection data, and dbus-tokio for integrating D-Bus with [Tokio](http://tokio.rs).
//! However, at the time of this writing, these are far less mature than this crate. 

#![warn(missing_docs)]

extern crate libc;

#[allow(missing_docs)]
extern crate libdbus_sys as ffi;


pub use crate::ffi::DBusBusType as BusType;
pub use crate::connection::DBusNameFlag as NameFlag;
pub use crate::ffi::DBusRequestNameReply as RequestNameReply;
pub use crate::ffi::DBusReleaseNameReply as ReleaseNameReply;
pub use crate::ffi::DBusMessageType as MessageType;

pub use crate::message::{Message, MessageItem, MessageItemArray, FromMessageItem, OwnedFd, ArrayError, ConnPath};
pub use crate::connection::{Connection, ConnectionItems, ConnectionItem, ConnMsgs, MsgHandler, MsgHandlerResult, MsgHandlerType, MessageCallback};
pub use crate::prop::PropHandler;
pub use crate::prop::Props;
pub use crate::watch::{Watch, WatchEvent};
pub use crate::signalargs::SignalArgs;

/// A TypeSig describes the type of a MessageItem.
#[deprecated(note="Use Signature instead")]
pub type TypeSig<'a> = std::borrow::Cow<'a, str>;

mod message;
mod prop;
mod watch;
mod connection;
mod signalargs;

mod error;
pub use error::Error;

mod connection2;
mod dispatcher;
pub use crate::connection2::TxRx;
pub use crate::dispatcher::{MessageDispatcher, MessageDispatcherConfig};
// pub mod crossroads;

mod matchrule;
pub use crate::matchrule::MatchRule;

mod strings;
pub use crate::strings::{Signature, Path, Interface, Member, ErrorName, BusName};

pub mod arg;

pub mod stdintf;

pub mod tree;

static INITDBUS: std::sync::Once = std::sync::ONCE_INIT;

use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;

fn init_dbus() {
    INITDBUS.call_once(|| {
        if unsafe { ffi::dbus_threads_init_default() } == 0 {
            panic!("Out of memory when trying to initialize D-Bus library!");
        }
    });
}

fn c_str_to_slice(c: & *const c_char) -> Option<&str> {
    if *c == ptr::null() { None }
    else { std::str::from_utf8( unsafe { CStr::from_ptr(*c).to_bytes() }).ok() }
}

fn to_c_str(n: &str) -> CString { CString::new(n.as_bytes()).unwrap() }


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
        let d = c.watch_fds();
        assert!(d.len() > 0);
        println!("Fds to watch: {:?}", d);
    }
}
