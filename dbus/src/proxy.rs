//! A D-Bus "Proxy" is a client-side object that corresponds to a remote object on the server side. 
//! Calling methods on the proxy object calls methods on the remote object.
//! Read more in the [D-Bus tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html#proxies)

use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::{channel, Error, Message};

/// A struct that wraps a connection, destination and path.
///
/// Useful if you want to make many method calls to the same remote object. Supersedes ConnPath.
#[derive(Clone, Debug)]
pub struct Proxy<'a, S> {
    /// Destination, i e what D-Bus service you're communicating with
    pub dest: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Timeout in milliseconds for blocking method calls
    pub timeout_ms: i32,
    /// Some way to send a message, either blocking or non-blocking.
    pub sender: S,
}

impl<'a, S> Proxy<'a, S> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, timeout_ms: i32, sender: S) -> Self {
        Proxy { dest: dest.into(), path: path.into(), timeout_ms, sender } 
    }
}

impl<'a, T: channel::BlockingSender, S: std::ops::Deref<Target=T>> Proxy<'a, S> {
// impl<'a, S: std::convert::AsRef<channel::Sender>> Proxy<'a, S> {
    /// Make a method call using typed input and output arguments, then block waiting for a reply.
    ///
    /// # Example
    ///
    /// ```
    /// use dbus::channel::Connection;
    /// use dbus::proxy::Proxy;
    ///
    /// let conn = Connection::new_session()?;
    /// let proxy = Proxy::new("org.freedesktop.DBus", "/", 5000, &conn);
    /// let (has_owner,): (bool,) = proxy.blocking_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",))?;
    /// assert_eq!(has_owner, false);
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn blocking_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A) -> Result<R, Error> {
        let mut msg = Message::method_call(&self.dest, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let r = self.sender.send_with_reply_and_block(msg, self.timeout_ms)?;
        Ok(R::read(&mut r.iter_init())?)
    }
}


