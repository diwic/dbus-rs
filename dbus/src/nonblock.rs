//! Experimental async version of connection.
//!
//! You're probably going to need a companion crate - dbus-tokio - for this connection to make sense.
//! When async/await is stable, expect more here.

use crate::{Error, Message};
use crate::channel::{MatchingReceiver, Channel, Sender};
use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::message::MatchRule;

use std::sync::{Arc, Mutex};
use std::{future, task, pin, mem};
use std::collections::{HashMap, BTreeMap};
use std::cell::{Cell, RefCell};

pub mod stdintf;

/// Thread local + async Connection 
pub struct LocalConnection {
    channel: Channel,
    replies: RefCell<HashMap<u32, Box<dyn FnOnce(Message, &LocalConnection)>>>,
    filters: RefCell<BTreeMap<u32, (MatchRule<'static>, Box<dyn FnMut(Message, &LocalConnection) -> bool>)>>,
    filter_nextid: Cell<u32>,
}

impl AsRef<Channel> for LocalConnection {
    fn as_ref(&self) -> &Channel { &self.channel }
}

impl From<Channel> for LocalConnection {
    fn from(x: Channel) -> Self {
        LocalConnection {
            channel: x,
            replies: Default::default(),
            filters: Default::default(),
            filter_nextid: Default::default(),
        }
    }
}

impl Sender for LocalConnection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

/// async Connection where handlers are Send + Sync 
pub struct SyncConnection {
    channel: Channel,
    replies: Mutex<HashMap<u32, <Self as NonblockReply>::F>>,
    filters: Mutex<(BTreeMap<u32, (MatchRule<'static>, Box<dyn FnMut(Message, &Self) -> bool + Send>)>, u32)>,
}

impl AsRef<Channel> for SyncConnection {
    fn as_ref(&self) -> &Channel { &self.channel }
}

impl From<Channel> for SyncConnection {
    fn from(x: Channel) -> Self {
        SyncConnection {
            channel: x,
            replies: Default::default(),
            filters: Default::default(),
        }
    }
}

impl Sender for SyncConnection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}



pub trait NonblockReply {
    type F;
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<u32, ()>;
    fn cancel_reply(&self, id: u32) -> Option<Self::F>;
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F where Self: Sized;
}

impl NonblockReply for LocalConnection {
    type F = Box<dyn FnOnce(Message, &LocalConnection)>;
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<u32, ()> {
        self.channel.send(msg).map(|x| {
            self.replies.borrow_mut().insert(x, f);
            x
        })
    }
    fn cancel_reply(&self, id: u32) -> Option<Self::F> { self.replies.borrow_mut().remove(&id) }
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F { Box::new(g) }
}

impl MatchingReceiver for LocalConnection {
    type F = Box<dyn FnMut(Message, &LocalConnection) -> bool>;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> u32 {
        let id = self.filter_nextid.get();
        self.filter_nextid.set(id+1);
        self.filters.borrow_mut().insert(id, (m, f));
        id
    }
    fn stop_receive(&self, id: u32) -> Option<(MatchRule<'static>, Self::F)> {
        self.filters.borrow_mut().remove(&id)
    }
}

impl NonblockReply for SyncConnection {
    type F = Box<dyn FnOnce(Message, &SyncConnection) + Send>;
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<u32, ()> {
        self.channel.send(msg).map(|x| {
            self.replies.lock().unwrap().insert(x, f);
            x
        })
    }
    fn cancel_reply(&self, id: u32) -> Option<Self::F> { self.replies.lock().unwrap().remove(&id) }
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F { Box::new(g) }
}

pub trait Process: Sender + AsRef<Channel> {
    /// Dispatches all pending messages, without blocking.
    ///
    /// This is usually called from the reactor only, after read_write.
    fn process_all(&self) {
        let c: &Channel = self.as_ref();
        while let Some(msg) = c.pop_message() {
            self.process_one(msg);
        }
    }

    /// Dispatches a message.
    fn process_one(&self, msg: Message);
}

impl Process for LocalConnection {
    fn process_one(&self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            if let Some(f) = self.replies.borrow_mut().remove(&serial) {
                f(msg, self);
                return;
            }
        }
        let mut filters = self.filters.borrow_mut();
        if let Some(k) = filters.iter_mut().find(|(_, v)| v.0.matches(&msg)).map(|(k, _)| *k) {
            let mut v = filters.remove(&k).unwrap();
            drop(filters);
            if v.1(msg, &self) {
                let mut filters = self.filters.borrow_mut();
                filters.insert(k, v);
            }
            return;
        }
        if let Some(reply) = crate::channel::default_reply(&msg) {
            let _ = self.send(reply);
        }
    }
}

impl Process for SyncConnection {
    fn process_one(&self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            if let Some(f) = self.replies.lock().unwrap().remove(&serial) {
                f(msg, self);
                return;
            }
        }
/*        let mut filters = self.filters.lock().unwrap();
        if let Some(k) = filters.iter_mut().find(|(_, v)| v.0.matches(&msg)).map(|(k, _)| *k) {
            let mut v = filters.remove(&k).unwrap();
            drop(filters);
            if v.1(msg, &self) {
                let mut filters = self.filters.borrow_mut();
                filters.insert(k, v);
            }
            return;
        } */
        if let Some(reply) = crate::channel::default_reply(&msg) {
            let _ = self.send(reply);
        }
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

