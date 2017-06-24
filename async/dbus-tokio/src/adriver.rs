// Third attempt.
#![allow(dead_code, unused_imports)] // Because WIP

use mio::{self, unix, Ready};
use mio::unix::UnixReady;
use std::{io, mem, fmt};
use dbus::{Connection, ConnectionItems, ConnectionItem, Watch, WatchEvent, MsgHandler, Message, Error as DBusError};
use futures::{Async, Future, task, Stream, Poll};
use futures::sync::oneshot;
use tokio_core::reactor::{PollEvented, Handle as CoreHandle};
use std::rc::{Rc, Weak};
use std::os::raw::c_uint;
use std::cell::RefCell;
use std::collections::{VecDeque, HashMap};
use std::os::unix::io::RawFd;

type MCallMap = Rc<RefCell<HashMap<u32, oneshot::Sender<Message>>>>;

#[derive(Debug)]
pub struct AConnection {
    conn: Rc<Connection>,
    quit: Option<oneshot::Sender<()>>,
    callmap: MCallMap,
}

impl AConnection {
    pub fn new(c: Rc<Connection>, h: CoreHandle) -> io::Result<AConnection> {
        let (tx, rx) = oneshot::channel();
        let map: MCallMap = Default::default();
        let mut d = ADriver {
            conn: c.clone(),
            fds: HashMap::new(),
            core: h.clone(),
            quit: rx,
            callmap: map.clone(),
        };
        let i = AConnection {
            conn: c,
            quit: Some(tx),
            callmap: map,
        };
        for w in i.conn.watch_fds() { d.modify_watch(w)?; }
        h.spawn(d);
        Ok(i)
    }

    pub fn method_call(&self, m: Message) -> Result<AMethodCall, &'static str> {
        let r = self.conn.send(m).map_err(|_| "D-Bus send error")?;
        let (tx, rx) = oneshot::channel();
        let mut map = self.callmap.borrow_mut();
        map.insert(r, tx); // TODO: error check for duplicate entries. Should not happen, but if it does... 
        let mc = AMethodCall { serial: r, callmap: self.callmap.clone(), inner: rx };
        Ok(mc)
    }
}

impl Drop for AConnection {
    fn drop(&mut self) { let _ = self.quit.take().unwrap().send(()); }
}

#[derive(Debug)]
struct ADriver {
    conn: Rc<Connection>,
    fds: HashMap<RawFd, PollEvented<AWatch>>,
    core: CoreHandle,
    quit: oneshot::Receiver<()>,
    callmap: MCallMap,
}

impl ADriver {
    fn modify_watch(&mut self, w: Watch) -> io::Result<()> {
        if !w.readable() && !w.writable() {
            self.fds.remove(&w.fd());
        } else {

            if let Some(evented) = self.fds.get(&w.fd()) {
                let ww = evented.get_ref().0;
                if ww.readable() == w.readable() || ww.readable() == w.readable() { return Ok(()) }; 
            }
            self.fds.remove(&w.fd());

            let z = PollEvented::new(AWatch(w), &self.core)?;
            self.fds.insert(w.fd(), z);
        }
        Ok(())
    }

    fn handle_items(&mut self, items: ConnectionItems) {
        // TODO: What about all unwrapping in this function
        for i in items {
            println!("Got: {:?}", i);
            match i {
                ConnectionItem::WatchFd(w) => self.modify_watch(w).unwrap(),
                ConnectionItem::MethodReturn(m) => {
                    let mut map = self.callmap.borrow_mut();
                    let serial = m.get_reply_serial().unwrap();
                    let r = map.remove(&serial);
                    if let Some(r) = r { r.send(m).unwrap(); };
                }
                _ => {},
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
        let mut items = None;
        for w in self.fds.values() {
            let mut mask = UnixReady::hup() | UnixReady::error();
            if w.get_ref().0.readable() { mask = mask | Ready::readable().into(); }
            if w.get_ref().0.writable() { mask = mask | Ready::writable().into(); }
            let pr = w.poll_ready(*mask);
            let ur = if let Async::Ready(t) = pr { UnixReady::from(t) } else { continue };
            let flags =
                if ur.is_readable() { WatchEvent::Readable as c_uint } else { 0 } +
                if ur.is_writable() { WatchEvent::Writable as c_uint } else { 0 } +
                if ur.is_hup() { WatchEvent::Hangup as c_uint } else { 0 } +
                if ur.is_error() { WatchEvent::Error as c_uint } else { 0 };
            println!("{:?} is {:?}", w.get_ref().0.fd(), ur);
            items = Some(cc.watch_handle(w.get_ref().0.fd(), flags));
            if ur.is_readable() { w.need_read() };
            if ur.is_writable() { w.need_write() };
        };
        if let Some(items) = items { self.handle_items(items) };
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
                opts: mio::PollOpt) -> io::Result<()>
    {
        if !self.0.readable() { interest.remove(mio::Ready::readable()) };
        if !self.0.writable() { interest.remove(mio::Ready::writable()) };
        unix::EventedFd(&self.0.fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &mio::Poll,
                  token: mio::Token,
                  interest: mio::Ready,
                  opts: mio::PollOpt) -> io::Result<()>
    {
        self.register(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        unix::EventedFd(&self.0.fd()).deregister(poll)
    }
}

#[derive(Debug)]
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

#[test]
fn aconnection_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let aconn = AConnection::new(conn.clone(), core.handle()).unwrap();

    let m = ::dbus::Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let reply = core.run(aconn.method_call(m).unwrap()).unwrap();
    let z: Vec<&str> = reply.get1().unwrap();
    println!("got reply: {:?}", z);
}

