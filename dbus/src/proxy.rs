//! A D-Bus "Proxy" is a client-side object that corresponds to a remote object on the server side. 
//! Calling methods on the proxy object calls methods on the remote object.
//! Read more in the [D-Bus tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html#proxies)

use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::{channel, Error, Message};
use crate::message::{MatchRule, SignalArgs};

/// A struct that wraps a connection, destination and path.
///
/// Useful if you want to make many method calls to the same remote object. Supersedes ConnPath.
#[derive(Clone, Debug)]
pub struct Proxy<'a, C> {
    /// Destination, i e what D-Bus service you're communicating with
    pub destination: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Timeout in milliseconds for blocking method calls
    pub timeout_ms: i32,
    /// Some way to send and/or receive messages, either blocking or non-blocking.
    pub connection: C,
}

impl<'a, C> Proxy<'a, C> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, timeout_ms: i32, connection: C) -> Self {
        Proxy { destination: dest.into(), path: path.into(), timeout_ms, connection } 
    }
}

impl<'a, T: channel::BlockingSender, C: std::ops::Deref<Target=T>> Proxy<'a, C> {
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
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let r = self.connection.send_with_reply_and_block(msg, self.timeout_ms)?;
        Ok(R::read(&mut r.iter_init())?)
    }
}

impl<'a, T, C> Proxy<'a, C> 
where
    T: channel::BlockingSender + channel::MatchingReceiver<F=Box<FnMut(Message) -> bool>>,
    C: 'static + std::ops::Deref<Target=T> + Clone
{

    /// TODO: Docs
    pub fn blocking_add_match<R: ReadAll, F: 'static + FnMut(R) -> bool>(&self, mr: &MatchRule, mut f: F) -> Result<u32, Error> {
        let mut mr = mr.static_clone();
        mr.path = Some(self.path.clone().into_static());
        mr.sender = Some(self.destination.clone().into_static());

        let mstr = mr.match_str();
        let msg = Message::call_with_args("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "AddMatch", (&mstr,));
        let r = self.connection.send_with_reply_and_block(msg, self.timeout_ms)?;
        let id = r.get_serial();
        let conn = self.connection.clone();
        let timeout = self.timeout_ms;
        self.connection.start_receive(id, mr, Box::new(move |msg: Message| {
            // We silently drop type mismatch errors here. 
            if let Ok(r) = R::read(&mut msg.iter_init()) { 
                if f(r) { return true };
                let msg = Message::call_with_args("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "RemoveMatch", (&mstr,));
                let _ = conn.send_with_reply_and_block(msg, timeout);
                false
            } else { true }
        }));
        Ok(id)
    }

    /// TODO: Docs
    pub fn blocking_match_signal<S: SignalArgs + ReadAll, F: 'static + FnMut(S) -> bool>(&self, f: F) -> Result<u32, Error> {
        self.blocking_add_match(&S::match_rule(Some(&self.destination), Some(&self.path)), f)
    }

}

