//! Connections and proxies that make blocking method calls.


use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::{channel, Error, Message};
use crate::message::{MatchRule, SignalArgs};
use crate::channel::{Channel, BusType};
use std::cell::RefCell;

pub mod stdintf;

struct Filter<F> {
   id: u32,
   rule: MatchRule<'static>,
   callback: F, // ,
}

fn run_filters<F, G: FnOnce(&mut F, Message) -> bool>(filters: &mut Vec<Filter<F>>, msg: Message, g: G) -> Option<Message> {
    if let Some(idx) = filters.iter().position(|f| f.rule.matches(&msg)) {
        if !g(&mut filters[idx].callback, msg) { filters.remove(idx); }
        None
    } else { Some(msg) }
}

/// Experimental rewrite of Connection, thread local + non-async version
pub struct Connection {
    channel: Channel,
    filters: RefCell<Vec<Filter<Box<FnMut(Message) -> bool>>>>
}

impl Connection {
    /// Create a new connection to the session bus.
    pub fn new_session() -> Result<Self, Error> { Ok(Connection {
        channel: Channel::get_private(BusType::Session)?,
        filters: Default::default(),
    })}

    /// Create a new connection to the system-wide bus.
    pub fn new_system() -> Result<Self, Error> { Ok(Connection { 
        channel: Channel::get_private(BusType::System)?, 
        filters: Default::default(),
    })}

    fn dispatch(&self, msg: Message) {
        if let Some(msg) = run_filters(&mut self.filters.borrow_mut(), msg, |cb, msg| { cb(msg) }) {
            unimplemented!("{:?}", msg)
        }
    }

    /// Tries to handle an incoming message if there is one. If there isn't one,
    /// it will wait up to timeout_ms milliseconds.
    ///
    /// Note: Might panic if called recursively.
    pub fn process(&self, timeout_ms: i32) -> Result<bool, Error> {
        if let Some(msg) = self.channel.pop_message() {
            self.dispatch(msg);
            return Ok(true);
        }
        self.channel.read_write(Some(timeout_ms)).map_err(|_| 
            Error::new_custom("org.freedesktop.dbus.error.failed", "Failed to read/write data, disconnected from D-Bus?")
        )?;
        if let Some(msg) = self.channel.pop_message() {
            self.dispatch(msg);
            Ok(true)
        } else { Ok(false) }
    }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_proxy<'a, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'a self, dest: D, path: P, timeout_ms: i32) ->
    Proxy<'a, &'a Connection> {
        Proxy { connection: self, destination: dest.into(), path: path.into(), timeout_ms }
    }
}

/// Abstraction over different connections
pub trait BlockingSender {
    /// Sends a message over the D-Bus and blocks, waiting for a reply or a timeout. This is used for method calls.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    fn send_with_reply_and_block(&self, msg: Message, timeout_ms: i32) -> Result<Message, Error>;
}

impl BlockingSender for Channel {
    fn send_with_reply_and_block(&self, msg: Message, timeout_ms: i32) -> Result<Message, Error> {
        Channel::send_with_reply_and_block(self, msg, timeout_ms)
    }
}

impl BlockingSender for Connection {
    fn send_with_reply_and_block(&self, msg: Message, timeout_ms: i32) -> Result<Message, Error> {
        self.channel.send_with_reply_and_block(msg, timeout_ms)
    }
}

impl channel::Sender for Connection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

impl channel::MatchingReceiver for Connection {
    type F = Box<FnMut(Message) -> bool>;
    fn start_receive(&self, id: u32, m: MatchRule<'static>, f: Self::F) {
        self.filters.borrow_mut().push(Filter { id, rule: m, callback: f } )
    }
    fn stop_receive(&self, id: u32) -> Option<Self::F> {
        let mut filters = self.filters.borrow_mut(); 
        if let Some(idx) = filters.iter().position(|f| f.id == id) { Some(filters.remove(idx).callback) }
        else { None }
    }
}


/// A struct that wraps a connection, destination and path.
///
/// A D-Bus "Proxy" is a client-side object that corresponds to a remote object on the server side. 
/// Calling methods on the proxy object calls methods on the remote object.
/// Read more in the [D-Bus tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html#proxies)
#[derive(Clone, Debug)]
pub struct Proxy<'a, C> {
    /// Destination, i e what D-Bus service you're communicating with
    pub destination: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Timeout in milliseconds for method calls
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

impl<'a, T: BlockingSender, C: std::ops::Deref<Target=T>> Proxy<'a, C> {
// impl<'a, S: std::convert::AsRef<channel::Sender>> Proxy<'a, S> {
    /// Make a method call using typed input and output arguments, then block waiting for a reply.
    ///
    /// # Example
    ///
    /// ```
    /// use dbus::blocking::{Connection, Proxy};
    ///
    /// let conn = Connection::new_session()?;
    /// let proxy = Proxy::new("org.freedesktop.DBus", "/", 5000, &conn);
    /// let (has_owner,): (bool,) = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",))?;
    /// assert_eq!(has_owner, false);
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn method_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A) -> Result<R, Error> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let r = self.connection.send_with_reply_and_block(msg, self.timeout_ms)?;
        Ok(R::read(&mut r.iter_init())?)
    }
}

    /// Sets up a match, including calls to the D-Bus server to add and remove this match.
    fn add_match<C, T, F>(conn: &C, mr: MatchRule<'static>, timeout_ms: i32, mut f: F) -> Result<u32, Error>
    where 
        T: BlockingSender + channel::MatchingReceiver<F=Box<FnMut(Message) -> bool>>,
        C: 'static + std::ops::Deref<Target=T> + Clone,
        F: 'static + FnMut(Message) -> bool {

        let mstr = mr.match_str();
        let msg = Message::call_with_args("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "AddMatch", (&mstr,));
        let r = conn.send_with_reply_and_block(msg, timeout_ms)?;
        let id = r.get_serial();
        let conn2 = conn.clone();
        conn.start_receive(id, mr, Box::new(move |msg: Message| {
            if f(msg) { return true };
            let msg = Message::call_with_args("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "RemoveMatch", (&mstr,));
            let _ = conn2.send_with_reply_and_block(msg, timeout_ms);
            false
        }));
        Ok(id)
    }

impl<'a, T, C> Proxy<'a, C> 
where
    T: BlockingSender + channel::MatchingReceiver<F=Box<FnMut(Message) -> bool>>,
    C: 'static + std::ops::Deref<Target=T> + Clone
{

    /// Sets up an incoming signal match, that calls the supplied callback every time the signal is received.
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn match_signal<S: SignalArgs + ReadAll, F: 'static + FnMut(S) -> bool>(&self, mut f: F) -> Result<u32, Error> {
        let mr = S::match_rule(Some(&self.destination), Some(&self.path)).static_clone();
        add_match(&self.connection, mr, self.timeout_ms, move |msg| {
            // We silently drop type mismatch errors here. Hopefully that's the right thing to do.
            if let Ok(r) = S::read(&mut msg.iter_init()) { f(r) } else { true }
        })
    }

}

