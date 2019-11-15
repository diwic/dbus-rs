//! Async version of connection.
//!
//! You're probably going to need a companion crate - dbus-tokio - for this connection to make sense,
//! (although you can also just call read_write and process_all at regular intervals).
//!
//! When async/await is stable, expect more here.

use crate::{Error, Message};
use crate::channel::{MatchingReceiver, Channel, Sender, Token};
use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::message::MatchRule;

use std::sync::{Arc, Mutex};
use std::{future, task, pin, mem};
use std::cell::RefCell;
use crate::filters::{Filters, Replies};

mod generated_org_freedesktop_notifications;
mod generated_org_freedesktop_dbus;

/// This module contains some standard interfaces and an easy way to call them.
///
/// See the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces) for more information about these standard interfaces.
///
/// The code was created by dbus-codegen.
pub mod stdintf {
    #[allow(missing_docs)]
    pub mod org_freedesktop_dbus {
        pub use super::super::generated_org_freedesktop_notifications::*;
        #[allow(unused_imports)]
        pub(crate) use super::super::generated_org_freedesktop_dbus::*;
    }
}

/// A connection to D-Bus, thread local + async version
pub struct LocalConnection {
    channel: Channel,
    filters: RefCell<Filters<LocalFilterCb>>,
    replies: RefCell<Replies<LocalRepliesCb>>,
}

/// A connection to D-Bus, async version where callbacks are Send but not Sync.
pub struct Connection {
    channel: Channel,
    filters: RefCell<Filters<FilterCb>>,
    replies: RefCell<Replies<RepliesCb>>,
}

/// A connection to D-Bus, Send + Sync + async version
pub struct SyncConnection {
    channel: Channel,
    filters: Mutex<Filters<SyncFilterCb>>,
    replies: Mutex<Replies<SyncRepliesCb>>,
}



macro_rules! connimpl {
     ($c: ident, $cb: ident, $rcb: ident $(, $ss:tt)*) =>  {

type
    $cb = Box<dyn FnMut(Message, &$c) -> bool $(+ $ss)* + 'static>;
type
    $rcb = Box<dyn FnOnce(Message, &$c) $(+ $ss)* + 'static>;

impl From<Channel> for $c {
    fn from(x: Channel) -> Self {
        $c {
            channel: x,
            replies: Default::default(),
            filters: Default::default(),
        }
    }
}

impl AsRef<Channel> for $c {
    fn as_ref(&self) -> &Channel { &self.channel }
}

impl Sender for $c {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

impl MatchingReceiver for $c {
    type F = $cb;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> Token {
        self.filters_mut().add(m, f)
    }
    fn stop_receive(&self, id: Token) -> Option<(MatchRule<'static>, Self::F)> {
        self.filters_mut().remove(id)
    }
}

impl NonblockReply for $c {
    type F = $rcb;
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<Token, ()> {
        self.channel.send(msg).map(|x| {
            let t = Token(x as usize);
            self.replies_mut().insert(t, f);
            t
        })
    }
    fn cancel_reply(&self, id: Token) -> Option<Self::F> { self.replies_mut().remove(&id) }
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F { Box::new(g) }
}


impl Process for $c {
    fn process_one(&self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            if let Some(f) = self.replies_mut().remove(&Token(serial as usize)) {
                f(msg, self);
                return;
            }
        }
        let ff = self.filters_mut().remove_matching(&msg);
        if let Some(mut ff) = ff {
            if ff.2(msg, self) {
                self.filters_mut().insert(ff);
            }
        } else if let Some(reply) = crate::channel::default_reply(&msg) {
            let _ = self.send(reply);
        }
    }
}

    }
}

connimpl!(Connection, FilterCb, RepliesCb, Send);
connimpl!(LocalConnection, LocalFilterCb, LocalRepliesCb);
connimpl!(SyncConnection, SyncFilterCb, SyncRepliesCb, Send);

impl Connection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<FilterCb>> { self.filters.borrow_mut() }
    fn replies_mut(&self) -> std::cell::RefMut<Replies<RepliesCb>> { self.replies.borrow_mut() }
}

impl LocalConnection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<LocalFilterCb>> { self.filters.borrow_mut() }
    fn replies_mut(&self) -> std::cell::RefMut<Replies<LocalRepliesCb>> { self.replies.borrow_mut() }
}

impl SyncConnection {
    fn filters_mut(&self) -> std::sync::MutexGuard<Filters<SyncFilterCb>> { self.filters.lock().unwrap() }
    fn replies_mut(&self) -> std::sync::MutexGuard<Replies<SyncRepliesCb>> { self.replies.lock().unwrap() }
}

/// Internal helper trait for async method replies.
pub trait NonblockReply {
    /// Callback type
    type F;
    /// Sends a message and calls the callback when a reply is received.
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<Token, ()>;
    /// Cancels a pending reply.
    fn cancel_reply(&self, id: Token) -> Option<Self::F>;
    /// Internal helper function that creates a callback.
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F where Self: Sized;
}


/// Internal helper trait, implemented for connections that process incoming messages.
pub trait Process: Sender + AsRef<Channel> {
    /// Dispatches all pending messages, without blocking.
    ///
    /// This is usually called from the reactor only, after read_write.
    /// Despite this taking &self and not "&mut self", it is a logic error to call this
    /// recursively or from more than one thread at a time.
    fn process_all(&self) {
        let c: &Channel = self.as_ref();
        while let Some(msg) = c.pop_message() {
            self.process_one(msg);
        }
    }

    /// Dispatches a message.
    fn process_one(&self, msg: Message);
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
    /// Some way to send and/or receive messages, non-blocking.
    pub connection: C,
}

impl<'a, C> Proxy<'a, C> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, connection: C) -> Self {
        Proxy { destination: dest.into(), path: path.into(), connection }
    }
}

impl<'a, T, C> Proxy<'a, C>
where
    T: NonblockReply,
    C: std::ops::Deref<Target=T>
{

    /// Make a method call using typed input argument, returns a future that resolves to the typed output arguments.
    pub fn method_call<'i, 'm, R: ReadAll + 'static, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A)
    -> MethodReply<R> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));

        let mr = Arc::new(Mutex::new(MRInner::Neither));
        let mr2 = mr.clone();
        let f = T::make_f(move |msg: Message, _: &T| {
            let mut inner = mr2.lock().unwrap();
            let old = mem::replace(&mut *inner, MRInner::Ready(Ok(msg)));
            if let MRInner::Pending(waker) = old { waker.wake() }
        });
        if let Err(_) = self.connection.send_with_reply(msg, f) {
            *mr.lock().unwrap() = MRInner::Ready(Err(Error::new_failed("Failed to send message")));
        }
        MethodReply(mr, Some(Box::new(|msg: Message| { msg.read_all() })))
    }
}

enum MRInner {
    Ready(Result<Message, Error>),
    Pending(task::Waker),
    Neither,
}

/// Future method reply, used while waiting for a method call reply from the server.
pub struct MethodReply<T>(Arc<Mutex<MRInner>>, Option<Box<dyn FnOnce(Message) -> Result<T, Error> + Send + Sync + 'static>>);

impl<T> future::Future for MethodReply<T> {
    type Output = Result<T, Error>;
    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Result<T, Error>> {
        let r = {
            let mut inner = self.0.lock().unwrap();
            let r = mem::replace(&mut *inner, MRInner::Neither);
            if let MRInner::Ready(r) = r { r }
            else {
                mem::replace(&mut *inner, MRInner::Pending(ctx.waker().clone()));
                return task::Poll::Pending
            }
        };
        let readfn = self.1.take().expect("Polled MethodReply after Ready");
        task::Poll::Ready(r.and_then(readfn))
    }
}

impl<T: 'static> MethodReply<T> {
    /// Convenience combinator in case you want to post-process the result after reading it
    pub fn and_then<T2>(self, f: impl FnOnce(T) -> Result<T2, Error> + Send + Sync + 'static) -> MethodReply<T2> {
        let MethodReply(inner, first) = self;
        MethodReply(inner, Some({
            let first = first.unwrap();
            Box::new(|r| first(r).and_then(f))
        }))
    }
}


#[test]
fn test_conn_send_sync() {
    fn is_send<T: Send>(_: &T) {}
    fn is_sync<T: Sync>(_: &T) {}
    let c = SyncConnection::from(Channel::get_private(crate::channel::BusType::Session).unwrap());
    is_send(&c);
    is_sync(&c);

    let c = Connection::from(Channel::get_private(crate::channel::BusType::Session).unwrap());
    is_send(&c);
}
