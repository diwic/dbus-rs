// Third attempt.

use mio::{self, unix, Ready};
use mio::unix::UnixReady;
use std::io;
use dbus::{Connection, ConnectionItems, ConnectionItem, Watch, WatchEvent, Message, Error as DBusError};
use futures::{Async, Future, Stream, Poll};
use futures::sync::{oneshot, mpsc};
use tokio_core::reactor::{PollEvented, Handle as CoreHandle};
use std::rc::Rc;
use std::os::raw::c_uint;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::RawFd;

type MCallMap = Rc<RefCell<HashMap<u32, oneshot::Sender<Message>>>>;

type MStream = Rc<RefCell<Option<mpsc::UnboundedSender<Message>>>>;

#[derive(Debug)]
/// A Tokio enabled D-Bus connection.
///
/// While an AConnection exists, it will consume all incoming messages.
/// Creating more than one AConnection for the same Connection is not recommended.
pub struct AConnection {
    conn: Rc<Connection>,
    quit: Option<oneshot::Sender<()>>,
    callmap: MCallMap,
    msgstream: MStream,
}

impl AConnection {
    /// Create an AConnection, which spawns a task on the core.
    ///
    /// The task handles incoming messages, and continues to do so until the
    /// AConnection is dropped.
    pub fn new(c: Rc<Connection>, h: CoreHandle) -> io::Result<AConnection> {
        let (tx, rx) = oneshot::channel();
        let map: MCallMap = Default::default();
        let istream: MStream = Default::default();
        let mut d = ADriver {
            conn: c.clone(),
            fds: HashMap::new(),
            core: h.clone(),
            quit: rx,
            callmap: map.clone(),
            msgstream: istream.clone(),
        };
        let i = AConnection {
            conn: c,
            quit: Some(tx),
            callmap: map,
            msgstream: istream,
        };
        for w in i.conn.watch_fds() { d.modify_watch(w, false)?; }
        h.spawn(d);
        Ok(i)
    }

    /// Sends a method call message, and returns a Future for the method return.
    pub fn method_call(&self, m: Message) -> Result<AMethodCall, &'static str> {
        let r = self.conn.send(m).map_err(|_| "D-Bus send error")?;
        let (tx, rx) = oneshot::channel();
        let mut map = self.callmap.borrow_mut();
        map.insert(r, tx); // TODO: error check for duplicate entries. Should not happen, but if it does... 
        let mc = AMethodCall { serial: r, callmap: self.callmap.clone(), inner: rx };
        Ok(mc)
    }

    /// Returns a stream of incoming messages.
    ///
    /// Creating more than one stream for the same AConnection is not supported; this function will
    /// fail with an error if you try. Drop the first stream if you need to create a second one.
    pub fn messages(&self) -> Result<AMessageStream, &'static str> {
        let mut i = self.msgstream.borrow_mut();
        if i.is_some() { return Err("Another instance of AMessageStream already exists"); }
        let (tx, rx) = mpsc::unbounded();
        *i = Some(tx);
        Ok(AMessageStream { inner: rx, stream: self.msgstream.clone() })
    }
}

impl Drop for AConnection {
    fn drop(&mut self) {
        debug!("Dropping AConnection");
        let _ = self.quit.take().unwrap().send(());
    }
}

#[derive(Debug)]
// Internal struct; this is the future spawned on the core.
struct ADriver {
    conn: Rc<Connection>,
    fds: HashMap<RawFd, PollEvented<AWatch>>,
    core: CoreHandle,
    quit: oneshot::Receiver<()>,
    callmap: MCallMap,
    msgstream: MStream,
}

impl ADriver {
    fn modify_watch(&mut self, w: Watch, poll_now: bool) -> io::Result<()> {
        debug!("Modify_watch: {:?}, poll_now: {:?}", w, poll_now);
        if !w.readable() && !w.writable() {
            self.fds.remove(&w.fd());
        } else {

            if let Some(evented) = self.fds.get(&w.fd()) {
                let ww = evented.get_ref().0;
                if ww.readable() == w.readable() && ww.writable() == w.writable() {
                    return Ok(())
                };
            }
            self.fds.remove(&w.fd());

            let z = PollEvented::new(AWatch(w), &self.core)?;

            if poll_now && z.get_ref().0.readable() { z.need_read() };
            if poll_now && z.get_ref().0.writable() { z.need_write() };

            self.fds.insert(w.fd(), z);
        }
        Ok(())
    }

    fn send_stream(&self, m: Message) {
        self.msgstream.borrow().as_ref().map(|z| { z.unbounded_send(m).unwrap() });
    }

    fn handle_items(&mut self, items: ConnectionItems) {
        // TODO: What about all unwrapping in this function
        for i in items {
            debug!("handle_items: {:?}", i);
            match i {
                ConnectionItem::WatchFd(w) => { self.modify_watch(w, true).unwrap(); },
                ConnectionItem::MethodReturn(m) => {
                    let mut map = self.callmap.borrow_mut();
                    let serial = m.get_reply_serial().unwrap();
                    let r = map.remove(&serial);
                    debug!("Serial {:?}, found: {:?}", serial, r.is_some());
                    if let Some(r) = r { r.send(m).unwrap(); }
                    else { self.send_stream(m) }
                }
                ConnectionItem::Nothing => unreachable!(),
                ConnectionItem::Signal(m) => self.send_stream(m),
                ConnectionItem::MethodCall(m) => self.send_stream(m),
            }
        }
    }
}

impl Future for ADriver {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<()>, ()> {
        let q = self.quit.poll();
        if q != Ok(Async::NotReady) { return Ok(Async::Ready(())); }

        let cc = self.conn.clone(); // Borrow checker made me do this
        for w in self.fds.values() {
            let mut mask = UnixReady::hup() | UnixReady::error();
            if w.get_ref().0.readable() { mask = mask | Ready::readable().into(); }
            if w.get_ref().0.writable() { mask = mask | Ready::writable().into(); }
            let pr = w.poll_ready(*mask);
            debug!("D-Bus i/o poll ready: {:?} is {:?}", w.get_ref().0.fd(), pr);
            let ur = if let Async::Ready(t) = pr { UnixReady::from(t) } else { continue };
            let flags =
                if ur.is_readable() { WatchEvent::Readable as c_uint } else { 0 } +
                if ur.is_writable() { WatchEvent::Writable as c_uint } else { 0 } +
                if ur.is_hup() { WatchEvent::Hangup as c_uint } else { 0 } +
                if ur.is_error() { WatchEvent::Error as c_uint } else { 0 };
            debug!("D-Bus i/o unix ready: {:?} is {:?}", w.get_ref().0.fd(), ur);
            cc.watch_handle(w.get_ref().0.fd(), flags);
            if ur.is_readable() { w.need_read() };
            if ur.is_writable() { w.need_write() };
        };
        self.handle_items(ConnectionItems::new(&cc, None, true));
        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
struct AWatch(Watch);

impl mio::Evented for AWatch {
    fn register(&self,
                poll: &mio::Poll,
                token: mio::Token,
                mut interest: mio::Ready,
                mut opts: mio::PollOpt) -> io::Result<()>
    {
        if !self.0.readable() { interest.remove(mio::Ready::readable()) };
        if !self.0.writable() { interest.remove(mio::Ready::writable()) };
        opts.remove(mio::PollOpt::edge());
        opts.insert(mio::PollOpt::level());
        unix::EventedFd(&self.0.fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &mio::Poll,
                  token: mio::Token,
                  mut interest: mio::Ready,
                  mut opts: mio::PollOpt) -> io::Result<()>
    {
        if !self.0.readable() { interest.remove(mio::Ready::readable()) };
        if !self.0.writable() { interest.remove(mio::Ready::writable()) };
        opts.remove(mio::PollOpt::edge());
        opts.insert(mio::PollOpt::level());
        unix::EventedFd(&self.0.fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        unix::EventedFd(&self.0.fd()).deregister(poll)
    }
}

#[derive(Debug)]
/// A Future that resolves when a method call is replied to.
pub struct AMethodCall {
    serial: u32,
    callmap: MCallMap,
    inner: oneshot::Receiver<Message>,
}

impl Future for AMethodCall {
    type Item = Message;
    type Error = DBusError;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let x = self.inner.poll().map_err(|_| DBusError::new_custom("org.freedesktop.DBus.Failed", "Tokio cancelled future"))?;
        if let Async::Ready(mut m) = x {
            m.as_result()?;
            Ok(Async::Ready(m))
        } else { Ok(Async::NotReady) }
    }
}

impl Drop for AMethodCall {
    fn drop(&mut self) {
        let mut map = self.callmap.borrow_mut();
        map.remove(&self.serial);
    }
}

#[derive(Debug)]
/// A Stream of incoming messages.
///
/// Messages already processed (method returns for AMethodCall, fd changes)
/// are already consumed and will not be present in the stream.
pub struct AMessageStream {
    inner: mpsc::UnboundedReceiver<Message>,
    stream: MStream,
}

impl Stream for AMessageStream {
    type Item = Message;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        debug!("Polling message stream");
        let r = self.inner.poll();
        debug!("msgstream found {:?}", r);
        r
    }
}

impl Drop for AMessageStream {
    fn drop(&mut self) {
        *self.stream.borrow_mut() = None;
    }
}

#[test]
fn aconnection_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let aconn = AConnection::new(conn.clone(), core.handle()).unwrap();

    let m = ::dbus::Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let reply = core.run(aconn.method_call(m).unwrap()).unwrap();
    let z: Vec<&str> = reply.get1().unwrap();
    println!("got reply: {:?}", z);
    assert!(z.iter().any(|v| *v == "org.freedesktop.DBus"));
}

#[test]
fn astream_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let aconn = AConnection::new(conn.clone(), core.handle()).unwrap();

    let items: AMessageStream = aconn.messages().unwrap();
    let signals = items.filter_map(|m| if m.msg_type() == ::dbus::MessageType::Signal { Some(m) } else { None });
    let firstsig = core.run(signals.into_future()).map(|(x, _)| x).map_err(|(x, _)| x).unwrap();
    println!("first signal was: {:?}", firstsig);
    assert_eq!(firstsig.unwrap().msg_type(), ::dbus::MessageType::Signal);
}

