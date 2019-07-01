use dbus;
use std::sync::Arc;
use std::pin::Pin;
use futures::channel::{oneshot, mpsc};

use dbus::connection::{RequestNameReply, ReleaseNameReply};
use dbus::strings::{BusName, Interface, Member};
use dbus::message::{SignalArgs, MatchRule};


use futures::task;
use futures::future::ready;

pub type Error = dbus::tree::MethodErr;

pub mod stdintf;

// To be sent to the backend
#[derive(Debug)]
enum Command {
    AddReply(u32, oneshot::Sender<dbus::Message>),
    AddStream(MatchRule<'static>, mpsc::UnboundedSender<dbus::Message>),
    Quit,
}

#[derive(Debug)]
pub struct ReplyMessage(Result<oneshot::Receiver<dbus::Message>, Option<Error>>);

impl futures::TryFuture for ReplyMessage {
    type Ok = dbus::Message;
    type Error = Error;
    fn try_poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Result<Self::Ok, Self::Error>> {
        match &mut self.0 {
            Err(e) => task::Poll::Ready(Err(e.take().unwrap())),
            Ok(ref mut recv) => {
                use futures::Future;
                let p: Pin<&mut oneshot::Receiver<dbus::Message>> = Pin::new(recv);
                let mut r: dbus::Message = futures::try_ready!(p.poll(ctx).map_err(|e| { Error::failed(&e) }));
                task::Poll::Ready((|| { r.as_result()?; Ok(r) })())
            }
        }
    }
}

impl ReplyMessage {
    pub fn new(serial: u32, handle: &ConnHandle) -> Self {
        let (s, r) = oneshot::channel();
        ReplyMessage(
            handle.1.unbounded_send(Command::AddReply(serial, s))
                .map_err(|e| { Some(Error::failed(&e)) })
                .map(|_| r)
        )
    }
}

pub struct MethodReply<T> {
    f: Pin<Box<futures::Future<Output=Result<T, Error>>>>,
}

impl<T> futures::TryFuture for MethodReply<T> {
    type Ok = T;
    type Error = Error;
    fn try_poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Result<Self::Ok, Self::Error>> {
        let p = Pin::new(&mut self.f);
        p.try_poll(ctx)
    }
}

impl<T: 'static> MethodReply<T> {
    pub fn from_msg<F: FnOnce(dbus::Message) -> Result<T, Error> + 'static>(msg: ReplyMessage, parse_fn: F) -> Self {
        use futures::TryFutureExt;
        MethodReply { f: Box::pin(msg.and_then(|m| ready(parse_fn(m))).into_future()) }
    }
}

impl<T: 'static + dbus::arg::ReadAll> MethodReply<T> {
    pub fn new(msg: ReplyMessage) -> Self {
        Self::from_msg(msg, |m| {
            let mut ii = m.iter_init();
            Ok(T::read(&mut ii)?)
        })
    }
}

#[derive(Debug)]
pub struct MessageStream(Result<mpsc::UnboundedReceiver<dbus::Message>, Option<Error>>);

impl futures::TryStream for MessageStream {
    type Ok = dbus::Message;
    type Error = Error;
    fn try_poll_next(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Option<Result<Self::Ok, Self::Error>>> {
        match &mut self.0 {
            Err(e) => { let e = e.take(); task::Poll::Ready(e.map(|e| Err(e))) },
            Ok(ref mut recv) => {
                use futures::Stream;
                let p: Pin<&mut mpsc::UnboundedReceiver<dbus::Message>> = Pin::new(recv);
                p.poll_next(ctx).map(|x| x.map(|x| Ok(x)))
            }
        }
    }
}

pub struct SignalStream<T> {
    f: Box<futures::TryStream<Ok=T, Error=Error> + Unpin>,
    handle: ConnHandle,
    match_str: String,
}

impl<T> futures::TryStream for SignalStream<T> {
    type Ok = T;
    type Error = Error;
    fn try_poll_next(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Option<Result<Self::Ok, Self::Error>>> {
        let p = Pin::new(&mut *self.f);
        p.try_poll_next(ctx)
    }
}

impl<T: SignalArgs + 'static> SignalStream<T> {
    fn new(mr: MatchRule<'static>, handle: ConnHandle) -> Self {
        use crate::stdintf::org_freedesktop::DBus;
        use futures::{TryFutureExt, TryStreamExt, FutureExt, StreamExt};

        // Let's try to make a stream that first has the error - if any - from the addMatch method call,
        // and then continues with parsed items
        let mr_str = mr.match_str();
        let s = handle.add_stream(mr).into_stream();
        let mcall = handle.with_dbus_path().add_match(&mr_str);
        let stream = mcall.into_future().into_stream();
        let stream = stream.filter_map(|x| futures::future::ready(x.err().map(|x| Err(x))));
        let s = stream.chain(s);
        let s = s.then(|r| futures::future::ready(r.and_then(|msg|
            T::from_message(&msg).ok_or_else(|| Error::failed(&"Received signal with invalid arguments"))
        )));
        SignalStream { f: Box::new(s), handle: handle, match_str: mr_str }
    }
}

impl<T> Drop for SignalStream<T> {
    fn drop(&mut self) {
        use crate::stdintf::org_freedesktop::DBus;
        self.handle.with_dbus_path().remove_match(&self.match_str);
    }
}

#[derive(Clone, Debug)]
pub struct ConnPath<'a> {
    /// A clone of the connection handle
    pub conn: ConnHandle,
    /// Destination, i e what D-Bus service you're communicating with
    pub dest: BusName<'a>,
    /// Object path on the destination
    pub path: dbus::Path<'a>,
}

impl<'a> ConnPath<'a> {
    /// Make a D-Bus method call.
    pub fn method_call<A: dbus::arg::AppendAll>(&self, i: &Interface, m: &Member, args: A) -> ReplyMessage {
        let mut msg = dbus::Message::method_call(&self.dest, &self.path, i, m);
        args.append(&mut dbus::arg::IterAppend::new(&mut msg));
        match self.conn.send(msg) {
            Ok(serial) => ReplyMessage::new(serial, &self.conn),
            Err(e) => ReplyMessage(Err(Some(e))),
        }
    }


    /// Make a D-Bus method call, where you can append arguments inside the closure.
    pub fn method_call_with_args<F>(&self, i: &Interface, m: &Member, f: F) -> ReplyMessage 
    where F: FnOnce(&mut dbus::Message)
    {
        let mut msg = dbus::Message::method_call(&self.dest, &self.path, i, m);
        f(&mut msg);
        match self.conn.send(msg) {
            Ok(serial) => ReplyMessage::new(serial, &self.conn),
            Err(e) => ReplyMessage(Err(Some(e))),
        }
    }

    /// Emit a D-Bus signal, where you can append arguments inside the closure.
    pub fn signal_with_args<F: FnOnce(&mut dbus::Message)>(&self, i: &Interface, m: &Member, f: F) -> Result<u32, Error> {
        let mut msg = dbus::Message::signal(&self.path, i, m);
        f(&mut msg);
        self.conn.send(msg)
    }

    /// Emit a D-Bus signal, where the arguments are in a struct.
    pub fn emit<S: SignalArgs>(&self, signal: &S) -> Result<u32, Error> {
        let msg = signal.to_emit_message(&self.path);
        self.conn.send(msg)
    }
}

#[derive(Clone, Debug)]
pub struct ConnHandle(Arc<dbus::channel::Channel>, mpsc::UnboundedSender<Command>);

impl ConnHandle {
    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> &str { self.0.unique_name().unwrap() }

    /// Puts a message into libdbus out queue.
    ///
    /// Returns a serial number than can be used to match against a reply.
    /// This does not flush the out queue, the messages are likely to be written the next time the main loop runs.
    pub fn send(&self, msg: dbus::Message) -> Result<u32, Error> {
        self.0.send(msg).map_err(|_| Error::failed(&"Sending message failed"))
    }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_path<'a, D: Into<BusName<'a>>, P: Into<dbus::Path<'a>>>(&'a self, dest: D, path: P) -> ConnPath<'a> {
        ConnPath { conn: self.clone(), dest: dest.into(), path: path.into() }
    }

    /// The recommended path for talking to the D-Bus server (via stdintf::DBus).
    pub fn with_dbus_path(&self) -> ConnPath {
        self.with_path("org.freedesktop.DBus", "/org/freedesktop/DBus")
    }

    /// If a message matches the rule, it is sent to the stream. Note: Currently only works for signals.
    pub fn add_stream(&self, rule: MatchRule<'static>) -> MessageStream {
        let (s, r) = mpsc::unbounded();
        MessageStream(self.1.unbounded_send(Command::AddStream(rule, s)).map(|_| r).map_err(|e| Some(Error::failed(&e))))
    }

    /// Returns a stream of corresponding signals, optionally filtered on sender and path.
    ///
    /// Makes a call to the D-Bus server to add the match as well.
    pub fn add_signal_stream<T: SignalArgs + 'static>(&self, sender: Option<BusName<'static>>, path: Option<dbus::Path<'static>>) -> SignalStream<T>
    {
        let mr = T::match_rule(sender.as_ref(), path.as_ref()).into_static();
        SignalStream::new(mr, self.clone())
    }

    /// Request a name on the D-Bus.
    ///
    /// For detailed information on the flags and return values, see the libdbus documentation.
    pub fn request_name(&self, name: &str, allow_replacement: bool, replace_existing: bool, do_not_queue: bool) -> MethodReply<RequestNameReply> {
        let flags: u32 = 
            if allow_replacement { 1 } else { 0 } +
            if replace_existing { 2 } else { 0 } +
            if do_not_queue { 4 } else { 0 };
        let m = self.with_dbus_path().method_call(&"org.freedesktop.DBus".into(), &"RequestName".into(), (name, flags));
        MethodReply::from_msg(m, |m| {
            let arg0: u32 = m.read1()?;
            use dbus::connection::RequestNameReply::*;
            let all = [PrimaryOwner, InQueue, Exists, AlreadyOwner];
            all.into_iter().find(|x| **x as u32 == arg0).map(|x| *x).ok_or_else(|| Error::failed(&"Invalid reply from DBus server"))
        })
    }

    /// Release a previoulsy requested name on the D-Bus.
    pub fn release_name(&self, name: &str) -> MethodReply<ReleaseNameReply> {
        let m = self.with_dbus_path().method_call(&"org.freedesktop.DBus".into(), &"ReleaseName".into(), (name,));
        MethodReply::from_msg(m, |m| {
            let arg0: u32 = m.read1()?;
            use dbus::connection::ReleaseNameReply::*;
            let all = [Released, NonExistent, NotOwner];
            all.into_iter().find(|x| **x as u32 == arg0).map(|x| *x).ok_or_else(|| Error::failed(&"Invalid reply from DBus server"))
        })
    }

    /// Tells the TxRx part to quit from the event loop.
    pub fn quit(&self) -> Result<(), ()> {
         self.1.unbounded_send(Command::Quit).map_err(|_| ())
    }
}

#[cfg(feature = "tml_io")]
pub mod thin_main_loop;

#[cfg(feature = "tokio_io")]
pub mod tokio;


mod connection;

pub use connection::Connection;

