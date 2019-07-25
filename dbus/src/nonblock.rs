use crate::{Error, Message};
use crate::channel::{Channel, Sender};
use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};

use std::sync::Mutex;
use std::{future, task, pin, mem};
use std::collections::HashMap;

enum PollReply<T, P> {
    Pending(P),
    Ready(T),
    Consumed,
}

#[derive(Default)]
struct Dispatcher {
    replies: HashMap<u32, PollReply<Message, task::Waker>>,
}

impl Dispatcher {
    fn dispatch(&mut self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            let v = self.replies.entry(serial).or_insert(PollReply::Consumed);
            let w = mem::replace(v, PollReply::Ready(msg));
            if let PollReply::Pending(w) = w { w.wake() };
            return;
        }
        // TODO: Default dispatch etc
    }
}

/// Thread local + async Connection 
pub struct Connection {
    channel: Channel,
    dispatcher: Mutex<Dispatcher>,
}

impl From<Channel> for Connection {
    fn from(x: Channel) -> Self {
        Connection {
            channel: x,
            dispatcher: Default::default(),
        }
    }
}

impl Sender for Connection {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}


impl Connection {
    /// Reads/writes data to the connection, without blocking.
    ///
    /// This is usually called from the reactor when there is input on the file descriptor.
    pub fn read_write(&self) -> Result<(), Error> {
        self.channel.read_write(Some(0)).map_err(|_| Error::new_custom("org.freedesktop.DBus.Error.Failed", "Read/write failed"))
    }

    /// Dispatches all pending messages, without blocking.
    ///
    /// This is usually called from the reactor, after read_write.
    pub fn dispatch_all(&self) {
        let mut d = self.dispatcher.lock().unwrap();
        while let Some(msg) = self.channel.pop_message() {
            d.dispatch(msg);
        }
    }

    fn check_reply(&self, serial: u32, ctx: &mut task::Context) -> Option<Message> {
        let mut d = self.dispatcher.lock().unwrap();
        let mut result = None;
        d.replies.entry(serial)
            .and_modify(|v| {
                let x = mem::replace(v, PollReply::Consumed);
                if let PollReply::Ready(msg) = x {
                    result = Some(msg)
                } else {
                    *v = PollReply::Pending(ctx.waker().clone());
                }
            })
            .or_insert_with(|| PollReply::Pending(ctx.waker().clone()));
        result
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

impl<'a, C: std::ops::Deref<Target=Connection> + Clone> Proxy<'a, C> {
    pub fn method_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A)
    -> MethodReply<R, C> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));

        match self.connection.send(msg) {
            Err(_) => MethodReply(
                PollReply::Ready(Err(Error::new_custom("org.freedesktop.DBus.Error.Failed", "Sending message failed"))),
                None
            ),
            Ok(s) => MethodReply(
               PollReply::Pending((s, self.connection.clone())),
               Some(Box::new(|r| Ok(R::read(&mut r.iter_init())?)))
            ),
        }
    }
}

type ReadFn<T> = Box<FnOnce(&mut Message) -> Result<T, Error>>;

/// Future method reply, used while waiting for a method call reply from the server.
pub struct MethodReply<T, C>(PollReply<Result<(), Error>, (u32, C)>, Option<ReadFn<T>>); 

impl<T: 'static, C> MethodReply<T, C> {
    /// Convenience combinator in case you want to post-process the result after reading it
    pub fn map<T2>(mut self, f: impl FnOnce(T) -> Result<T2, Error> + 'static) -> MethodReply<T2, C> {
        let first = self.1.take().unwrap();
        MethodReply(self.0, Some(Box::new(|r| first(r).and_then(f))))
    }
}

impl<T, C: Unpin + std::ops::Deref<Target=Connection>> future::Future for MethodReply<T, C> {
    type Output = Result<T, Error>;
    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Result<T, Error>> {
        let inner = &mut (*self).0;
        if let PollReply::Pending((serial, conn)) = inner {
            match conn.check_reply(*serial, ctx) {
                None => task::Poll::Pending,
                Some(mut msg) => {
                    *inner = PollReply::Consumed;
                    let reader = (*self).1.take().unwrap();
                    task::Poll::Ready(msg.as_result().and_then(|r| reader(r)))
                }
            }
        } else { panic!("Polled MethodReply after having returned Poll::Ready") }
    }
}
