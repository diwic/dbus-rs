//! Connections and proxies that make blocking method calls.


use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::{channel, Error, Message};
use crate::message::{MatchRule, SignalArgs};
use crate::channel::{Channel, BusType};
use std::{cell::{RefCell, Cell}, time::Duration, sync::Mutex};

pub mod stdintf;

struct Filter<F> {
   id: u32,
   rule: MatchRule<'static>,
   callback: F, // ,
}

fn dispatch<F, G: FnOnce(&mut F, Message) -> bool>(filters: &mut Vec<Filter<F>>, msg: Message, g: G) -> Option<Message> {
    if let Some(idx) = filters.iter().position(|f| f.rule.matches(&msg)) {
        if !g(&mut filters[idx].callback, msg) { filters.remove(idx); }
        None
    } else {
        crate::channel::default_reply(&msg)
    }
}

/// A connection to D-Bus, thread local + non-async version
pub struct Connection {
    channel: Channel,
    filters: RefCell<Vec<Filter<LocalFilterCb>>>,
    filter_nextid: Cell<u32>,
}

type
  SyncFilterCb = Box<dyn FnMut(Message, &SyncConnection) -> bool + Send + Sync + 'static>;
type
  LocalFilterCb = Box<dyn FnMut(Message, &Connection) -> bool + 'static>;

/// A connection to D-Bus, Send + Sync + non-async version
pub struct SyncConnection {
    channel: Channel,
    filters: Mutex<(u32, Vec<Filter<SyncFilterCb>>)>
}

use crate::blocking::stdintf::org_freedesktop_dbus;

impl Connection {
    /// Create a new connection to the session bus.
    pub fn new_session() -> Result<Self, Error> { Ok(Connection {
        channel: Channel::get_private(BusType::Session)?,
        filters: Default::default(),
        filter_nextid: Default::default()
    })}

    /// Create a new connection to the system-wide bus.
    pub fn new_system() -> Result<Self, Error> { Ok(Connection {
        channel: Channel::get_private(BusType::System)?,
        filters: Default::default(),
        filter_nextid: Default::default()
    })}

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> BusName { self.channel.unique_name().unwrap().into() }

    /// Tries to handle an incoming message if there is one. If there isn't one,
    /// it will wait up to timeout
    pub fn process(&mut self, timeout: Duration) -> Result<bool, Error> {
        if let Some(msg) = self.channel.blocking_pop_message(timeout)? {
            if let Some(reply) = dispatch(&mut self.filters.borrow_mut(), msg, |cb, msg| { cb(msg, self) }) {
                let _ = self.channel.send(reply);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_proxy<'a, 'b, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'b self, dest: D, path: P, timeout: Duration) ->
    Proxy<'a, &'b Connection> {
        Proxy { connection: self, destination: dest.into(), path: path.into(), timeout }
    }

    /// Request a name on the D-Bus.
    ///
    /// For detailed information on the flags and return values, see the libdbus documentation.
    pub fn request_name<'a, N: Into<BusName<'a>>>(&self, name: N, allow_replacement: bool, replace_existing: bool, do_not_queue: bool)
    -> Result<org_freedesktop_dbus::RequestNameReply, Error> {
        org_freedesktop_dbus::request_name(&self.channel, &name.into(), allow_replacement, replace_existing, do_not_queue)
    }

    /// Release a previously requested name on the D-Bus.
    pub fn release_name<'a, N: Into<BusName<'a>>>(&self, name: N) -> Result<org_freedesktop_dbus::ReleaseNameReply, Error> {
        org_freedesktop_dbus::release_name(&self.channel, &name.into())
    }

    /// Adds a new match to the connection, without setting up a callback when this message arrives.
    pub fn add_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.add_match(match_str)
    }

    /// Removes a match from the connection, without removing any callbacks.
    pub fn remove_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.remove_match(match_str)
    }

    /// Adds a new match to the connection, and sets up a callback when this message arrives.
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn add_match<S: ReadAll, F>(&self, match_rule: MatchRule<'static>, f: F) -> Result<u32, Error>
    where F: FnMut(S, &Self, &Message) -> bool + 'static {
        let m = match_rule.match_str();
        self.add_match_no_cb(&m)?;
        use channel::MatchingReceiver;
        Ok(self.start_receive(match_rule, MakeSignal::make(f, m)))
    }

    /// Removes a previously added match and callback from the connection.
    pub fn remove_match(&self, id: u32) -> Result<(), Error> {
        use channel::MatchingReceiver;
        let (mr, _) = self.stop_receive(id).ok_or_else(|| Error::new_failed("No match with that id found"))?;
        self.remove_match_no_cb(&mr.match_str())
    }
}

impl SyncConnection {
    /// Create a new connection to the session bus.
    pub fn new_session() -> Result<Self, Error> { Ok(SyncConnection {
        channel: Channel::get_private(BusType::Session)?,
        filters: Default::default(),
    })}

    /// Create a new connection to the system-wide bus.
    pub fn new_system() -> Result<Self, Error> { Ok(SyncConnection {
        channel: Channel::get_private(BusType::System)?,
        filters: Default::default(),
    })}

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> BusName { self.channel.unique_name().unwrap().into() }

    /// Tries to handle an incoming message if there is one. If there isn't one,
    /// it will wait up to timeout
    ///
    /// Note: Might deadlock if called recursively.
    pub fn process(&self, timeout: Duration) -> Result<bool, Error> {
        if let Some(msg) = self.channel.blocking_pop_message(timeout)? {
            if let Some(reply) = dispatch(&mut self.filters.lock().unwrap().1, msg, |cb, msg| { cb(msg, self) }) {
                let _ = self.channel.send(reply);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_proxy<'a, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'a self, dest: D, path: P, timeout: Duration) ->
    Proxy<'a, &'a SyncConnection> {
        Proxy { connection: self, destination: dest.into(), path: path.into(), timeout }
    }

    /// Request a name on the D-Bus.
    ///
    /// For detailed information on the flags and return values, see the libdbus documentation.
    pub fn request_name<'a, N: Into<BusName<'a>>>(&self, name: N, allow_replacement: bool, replace_existing: bool, do_not_queue: bool)
    -> Result<org_freedesktop_dbus::RequestNameReply, Error> {
        org_freedesktop_dbus::request_name(&self.channel, &name.into(), allow_replacement, replace_existing, do_not_queue)
    }

    /// Release a previously requested name on the D-Bus.
    pub fn release_name<'a, N: Into<BusName<'a>>>(&self, name: N) -> Result<org_freedesktop_dbus::ReleaseNameReply, Error> {
        org_freedesktop_dbus::release_name(&self.channel, &name.into())
    }

    /// Adds a new match to the connection, without setting up a callback when this message arrives.
    pub fn add_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.add_match(match_str)
    }

    /// Removes a match from the connection, without removing any callbacks.
    pub fn remove_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.remove_match(match_str)
    }

    /// Adds a new match to the connection, and sets up a callback when this message arrives.
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn add_match<S: ReadAll, F>(&self, match_rule: MatchRule<'static>, f: F) -> Result<u32, Error>
    where F: FnMut(S, &Self, &Message) -> bool + Send + Sync + 'static {
        let m = match_rule.match_str();
        self.add_match_no_cb(&m)?;
        use channel::MatchingReceiver;
        Ok(self.start_receive(match_rule, MakeSignal::make(f, m)))
    }

    /// Removes a previously added match and callback from the connection.
    pub fn remove_match(&self, id: u32) -> Result<(), Error> {
        use channel::MatchingReceiver;
        let (mr, _) = self.stop_receive(id).ok_or_else(|| Error::new_failed("No match with that id found"))?;
        self.remove_match_no_cb(&mr.match_str())
    }


}

/// Abstraction over different connections
pub trait BlockingSender {
    /// Sends a message over the D-Bus and blocks, waiting for a reply or a timeout. This is used for method calls.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error>;
}

impl BlockingSender for Channel {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        Channel::send_with_reply_and_block(self, msg, timeout)
    }
}

impl BlockingSender for Connection {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        self.channel.send_with_reply_and_block(msg, timeout)
    }
}

impl BlockingSender for SyncConnection {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        self.channel.send_with_reply_and_block(msg, timeout)
    }
}

impl channel::Sender for Connection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

impl channel::Sender for SyncConnection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

impl channel::MatchingReceiver for Connection {
    type F = LocalFilterCb;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> u32 {
        let id = self.filter_nextid.get();
        self.filter_nextid.set(id+1);
        self.filters.borrow_mut().push(Filter { id, rule: m, callback: f } );
        id
    }
    fn stop_receive(&self, id: u32) -> Option<(MatchRule<'static>, Self::F)> {
        let mut filters = self.filters.borrow_mut();
        if let Some(idx) = filters.iter().position(|f| f.id == id) {
            let x = filters.remove(idx);
            Some((x.rule, x.callback))
        }
        else { None }
    }
}

impl channel::MatchingReceiver for SyncConnection {
    type F = SyncFilterCb;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> u32 {
        let mut filters = self.filters.lock().unwrap();
        let id = filters.0 + 1;
        filters.0 = id;
        filters.1.push(Filter { id, rule: m, callback: f } );
        id
    }
    fn stop_receive(&self, id: u32) -> Option<(MatchRule<'static>, Self::F)> {
        let mut filters = self.filters.lock().unwrap();
        if let Some(idx) = filters.1.iter().position(|f| f.id == id) {
            let x = filters.1.remove(idx);
            Some((x.rule, x.callback))
        }
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
    /// Timeout for method calls
    pub timeout: Duration,
    /// Some way to send and/or receive messages, either blocking or non-blocking.
    pub connection: C,
}

impl<'a, C> Proxy<'a, C> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, timeout: Duration, connection: C) -> Self {
        Proxy { destination: dest.into(), path: path.into(), timeout, connection }
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
    /// let proxy = Proxy::new("org.freedesktop.DBus", "/", std::time::Duration::from_millis(5000), &conn);
    /// let (has_owner,): (bool,) = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",))?;
    /// assert_eq!(has_owner, false);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn method_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A) -> Result<R, Error> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let r = self.connection.send_with_reply_and_block(msg, self.timeout)?;
        Ok(R::read(&mut r.iter_init())?)
    }

    /// Starts matching incoming messages on this destination and path.
    ///
    /// For matching signals, match_signal_local or match_signal_sync might be more convenient.
    ///
    /// The match rule will be modified to include this path and destination only.
    ///
    /// If call_add_match is true, will notify the D-Bus server that matching should start.
    pub fn match_start(&self, mut mr: MatchRule<'static>, call_add_match: bool, f: <T as channel::MatchingReceiver>::F) -> Result<u32, Error>
    where T: channel::MatchingReceiver {
        mr.path = Some(self.path.clone().into_static());
        mr.sender = Some(self.destination.clone().into_static());
        if call_add_match {
            use crate::blocking::stdintf::org_freedesktop::DBus;
            let proxy = stdintf::proxy(&*self.connection);
            proxy.add_match(&mr.match_str())?;
        }

        Ok(self.connection.start_receive(mr, f))
    }

    /// Stops matching a signal added with match_start, match_signal_local or match_signal_sync.
    ///
    /// If call_add_match is true, will notify the D-Bus server that matching should stop,
    /// this should be true in case match_signal_local or match_signal_sync was used.
    pub fn match_stop(&self, id: u32, call_remove_match: bool) -> Result<(), Error>
    where T: channel::MatchingReceiver {
        if let Some((mr, _)) = self.connection.stop_receive(id) {
            if call_remove_match {
                use crate::blocking::stdintf::org_freedesktop::DBus;
                let proxy = stdintf::proxy(&*self.connection);
                proxy.remove_match(&mr.match_str())?;
            }
        }
        Ok(())
    }

    /// Sets up an incoming signal match, that calls the supplied callback every time the signal is received.
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn match_signal<S: SignalArgs + ReadAll, F>(&self, f: F) -> Result<u32, Error>
    where T: channel::MatchingReceiver,
          F: MakeSignal<<T as channel::MatchingReceiver>::F, S, T>
    {
        let mr = S::match_rule(Some(&self.destination), Some(&self.path)).static_clone();
        let ff = f.make(mr.match_str());
        self.match_start(mr, true, ff)
    }
}

/// Internal helper trait
pub trait MakeSignal<G, S, T> {
    /// Internal helper trait
    fn make(self, mstr: String) -> G;
}

impl<S: ReadAll, F: FnMut(S, &SyncConnection, &Message) -> bool + Send + Sync + 'static> MakeSignal<SyncFilterCb, S, SyncConnection> for F {
    fn make(mut self, mstr: String) -> SyncFilterCb {
        Box::new(move |msg: Message, conn: &SyncConnection| {
            if let Ok(s) = S::read(&mut msg.iter_init()) {
                if self(s, conn, &msg) { return true };
                let proxy = stdintf::proxy(conn);
                use crate::blocking::stdintf::org_freedesktop::DBus;
                let _ = proxy.remove_match(&mstr);
                false
            } else { true }
        })
    }
}

impl<S: ReadAll, F: FnMut(S, &Connection, &Message) -> bool + 'static> MakeSignal<LocalFilterCb, S, Connection> for F {
    fn make(mut self, mstr: String) -> LocalFilterCb {
        Box::new(move |msg: Message, conn: &Connection| {
            if let Ok(s) = S::read(&mut msg.iter_init()) {
                if self(s, conn, &msg) { return true };
                let proxy = stdintf::proxy(conn);
                use crate::blocking::stdintf::org_freedesktop::DBus;
                let _ = proxy.remove_match(&mstr);
                false
            } else { true }
        })
    }
}

#[test]
fn test_add_match() {
    use self::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as Ppc;
    let c = Connection::new_session().unwrap();
    let x = c.add_match(Ppc::match_rule(None, None), |_: Ppc, _, _| { true }).unwrap();
    c.remove_match(x).unwrap();
}

#[test]
fn test_conn_send_sync() {
    fn is_send<T: Send>(_: &T) {}
    fn is_sync<T: Sync>(_: &T) {}
    let c = SyncConnection::new_session().unwrap();
    is_send(&c);
    is_sync(&c);
}

#[test]
fn test_peer() {
    let mut c = Connection::new_session().unwrap();

    let c_name = c.unique_name().into_static();
    use std::sync::Arc;
    let done = Arc::new(false);
    let d2 = done.clone();
    let j = std::thread::spawn(move || {
        let c2 = Connection::new_session().unwrap();

        let proxy = c2.with_proxy(c_name, "/", Duration::from_secs(5));
        let (s2,): (String,) = proxy.method_call("org.freedesktop.DBus.Peer", "GetMachineId", ()).unwrap();
        println!("{}", s2);
        assert_eq!(Arc::strong_count(&d2), 2);
        s2
    });
    assert_eq!(Arc::strong_count(&done), 2);

    for _ in 0..30 {
        c.process(Duration::from_millis(100)).unwrap();
        if Arc::strong_count(&done) < 2 { break; }
    }

    let s2 = j.join().unwrap();

    let proxy = c.with_proxy("org.a11y.Bus", "/org/a11y/bus", Duration::from_secs(5));
    let (s1,): (String,) = proxy.method_call("org.freedesktop.DBus.Peer", "GetMachineId", ()).unwrap();

    assert_eq!(s1, s2);

}
