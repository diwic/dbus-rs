//! Connection base / building block.
//!
//! Contains some helper structs and traits common to all Connection types.-

use crate::{Message, MessageType, to_c_str};
use crate::message::MatchRule;
use std::os::unix::io::RawFd;

#[cfg(not(feature = "native"))]
mod not_native;
#[cfg(not(feature = "native"))]
use not_native::ConnHandle;
#[cfg(not(feature = "native"))]
pub use not_native::BusType;


#[cfg(feature = "native")]
mod native_channel;
#[cfg(feature = "native")]
use native_channel::ConnHandle;
#[cfg(feature = "native")]
pub use native_channel::BusType;


#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A file descriptor, and an indication whether it should be read from, written to, or both.
pub struct Watch {
    /// File descriptor
    pub fd: RawFd,
    /// True if wakeup should happen when the file descriptor is ready for reading
    pub read: bool,
    /// True if wakeup should happen when the file descriptor is ready for writing
    pub write: bool,
}

/// Low-level connection - handles read/write to the socket
///
/// You probably do not need to worry about this as you would typically
/// use the various blocking and non-blocking "Connection" structs instead.
///
/// This version avoids dbus_connection_dispatch, and thus avoids
/// callbacks from that function. Instead the same functionality
/// is implemented in the various blocking and non-blocking "Connection" components.
///
/// Blocking operations are clearly marked as such, although if you
/// try to access the connection from several threads at the same time,
/// blocking might occur due to an internal mutex inside the dbus library.
#[derive(Debug)]
pub struct Channel {
    handle: ConnHandle,
    #[cfg(not(feature = "native"))]
    watchmap: Option<Box<not_native::WatchMap>>,
}

/// Abstraction over different connections that send data
pub trait Sender {
    /// Schedules a message for sending.
    ///
    /// Returns a serial number than can be used to match against a reply.
    fn send(&self, msg: Message) -> Result<u32, ()>;
}

/// Use in case you don't want the send the message, but just collect it instead.
impl Sender for std::cell::RefCell<Vec<Message>> {
    fn send(&self, msg: Message) -> Result<u32, ()> {
        self.borrow_mut().push(msg);
        Ok(0)
    }
}

/// Token used to identify a callback in the MatchingReceiver trait
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Token(pub usize);

/// Abstraction over different connections that receive data
pub trait MatchingReceiver {
    /// Type of callback
    type F;
    /// Add a callback to be called in case a message matches.
    ///
    /// Returns an id that can be used to remove the callback.
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> Token;
    /// Remove a previously added callback.
    fn stop_receive(&self, id: Token) -> Option<(MatchRule<'static>, Self::F)>;
}

impl Sender for Channel {
    fn send(&self, msg: Message) -> Result<u32, ()> { Channel::send(self, msg) }
}

/// Handles what we need to be a good D-Bus citizen.
///
/// Call this if you have not handled the message yourself:
/// * It handles calls to org.freedesktop.DBus.Peer.
/// * For other method calls, it sends an error reply back that the method was unknown.
pub fn default_reply(m: &Message) -> Option<Message> {
    peer(&m).or_else(|| unknown_method(&m))
}

/// Replies if this is a call to org.freedesktop.DBus.Peer, otherwise returns None.
fn peer(m: &Message) -> Option<Message> {
    if let Some(intf) = m.interface() {
        if &*intf != "org.freedesktop.DBus.Peer" { return None; }
        if let Some(method) = m.member() {
            if &*method == "Ping" { return Some(m.method_return()) }
            if &*method == "GetMachineId" {
                #[cfg(feature = "native")]
                return Some(native_channel::machine_id_reply(m));
                #[cfg(not(feature = "native"))]
                return Some(not_native::machine_id_reply(m));
            }
        }
        Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Method does not exist")))
    } else { None }
}

/// For method calls, it replies that the method was unknown, otherwise returns None.
fn unknown_method(m: &Message) -> Option<Message> {
    if m.msg_type() != MessageType::MethodCall { return None; }
    // if m.get_no_reply() { return None; } // The reference implementation does not do this?
    Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Path, Interface, or Method does not exist")))
}

#[test]
fn test_channel_send_sync() {
    fn is_send<T: Send>(_: &T) {}
    fn is_sync<T: Sync>(_: &T) {}
    let c = Channel::get_private(BusType::Session).unwrap();
    is_send(&c);
    is_sync(&c);
}

#[test]
fn channel_simple_test() {
    let mut c = Channel::get_private(BusType::Session).unwrap();
    assert!(c.is_connected());
    let fds = c.watch_fds().unwrap();
    println!("{:?}", fds);
    assert!(fds.len() == 1);
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let reply = c.send(m).unwrap();
    let my_name = c.unique_name().unwrap();
    loop {
        while let Some(mut msg) = c.pop_message() {
            println!("{:?}", msg);
            if msg.get_reply_serial() == Some(reply) {
                let r = msg.as_result().unwrap();
                let z: crate::arg::Array<&str, _>  = r.get1().unwrap();
                for n in z {
                    println!("{}", n);
                    if n == my_name { return; } // Hooray, we found ourselves!
                }
                assert!(false);
            } else if let Some(r) = default_reply(&msg) {
                c.send(r).unwrap();
            }
        }
        c.read_write(Some(std::time::Duration::from_millis(100))).unwrap();
    }
}

#[test]
fn test_bus_type_is_compatible_with_set() {
    use std::collections::HashSet;

    let mut set: HashSet<BusType> = HashSet::new();
    set.insert(BusType::Starter);
    set.insert(BusType::Starter);

    assert_eq!(set.len(), 1);
    assert!(!set.contains(&BusType::Session));
    assert!(!set.contains(&BusType::System));
    assert!(set.contains(&BusType::Starter));
}
