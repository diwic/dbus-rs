// Low-level details, dealing with file descriptors etc

use mio::{self, unix, Ready};
use mio::unix::UnixReady;
use std::{io, mem, fmt};
use dbus::{Connection, ConnectionItems, ConnectionItem, Watch, WatchEvent, MsgHandler};
use futures::{Async, Future, task, Stream, Poll};
use futures::sync::oneshot;
use tokio_core::reactor::{PollEvented, Handle as CoreHandle};
use std::rc::{Rc, Weak};
use std::os::raw::c_uint;
use std::cell::RefCell;
use std::collections::{VecDeque, HashMap};
use std::os::unix::io::RawFd;

pub struct AConnection {
    conn: Rc<Connection>,
    core: CoreHandle,
    fds: HashMap<RawFd, PollEvented<AWatch>>,
    quit: Option<Box<Future<Item=(), Error=()>>>,
}

impl AConnection {
    fn modify_watch(&mut self, w: Watch) -> io::Result<()> {
        if !w.readable() && !w.writable() {
            self.fds.remove(&w.fd());
        } else {
            let z = PollEvented::new(AWatch(w), &self.core)?;
            self.fds.insert(w.fd(), z);
        }
        Ok(())
    }

    pub fn new(c: Rc<Connection>, h: CoreHandle) -> io::Result<AConnection> {
        let mut i = AConnection {
            conn: c,
            core: h,
            fds: HashMap::new(),
            quit: None,
        };
        for w in i.conn.watch_fds() { i.modify_watch(w)?; }
        Ok(i)
    }

    fn handle_items(&mut self, items: ConnectionItems) {
        for i in items {
            println!("Got: {:?}", i); 
        }
    }
}

impl Future for AConnection {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<()>, ()> {
        if let &mut Some(ref mut qrx) = &mut self.quit {
            let q = qrx.poll();
            if q != Ok(Async::NotReady) { return Ok(Async::Ready(())); }
        }
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

//#[derive(Debug)]
struct AWInner {
    conn: Rc<Connection>,
    items: RefCell<VecDeque<ConnectionItem>>,
    handlers: RefCell<Vec<Box<MsgHandler>>>,
    task: RefCell<Option<task::Task>>,
    fds: RefCell<HashMap<i32, AWatch>>,
}

impl fmt::Debug for AWInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "AWInner {{ task: {:?}, items: {:?} }}", self.task, self.items)
    }
}

impl AWInner {
    // Called from child's task
    fn handle_items(&self, mut items: ConnectionItems) {
        {
            mem::swap(items.msg_handlers(), &mut *self.handlers.borrow_mut());
            let mut ci = self.items.borrow_mut();
            while let Some(item) = items.next() {
                ci.push_back(item);
            }
            mem::swap(items.msg_handlers(), &mut *self.handlers.borrow_mut());
        }
        let t = self.task.borrow_mut().take();
        t.map(|t| t.unpark());
    }
}

#[derive(Debug, Clone)]
pub struct AWatcher(Rc<AWInner>);

impl AWatcher {
    pub fn new(c: Rc<Connection>, h: &CoreHandle) -> io::Result<AWatcher> {
        let i = Rc::new(AWInner {
            conn: c,
            task: RefCell::new(None),
            items: RefCell::new(VecDeque::new()),
            handlers: RefCell::new(vec!()),
            fds: RefCell::new(HashMap::new()),

        });
        for w in i.conn.watch_fds() {
            let (tx, rx) = oneshot::channel();
            // i.subtasks.borrow_mut().insert(w.fd(), tx);
            let child = AWatch2 {
                io: PollEvented::new(AWatch(w), h)?,
                parent: Rc::downgrade(&i),
                quitrx: rx,
            };
            h.spawn(child);
        }
        Ok(AWatcher(i))
    }

    /// Builder method that adds a new msg handler.
    pub fn with<H: 'static + MsgHandler>(self, h: H) -> Self {
        self.append(h); self
    }

    /// Non-builder method that adds a new msg handler.
    pub fn append<H: 'static + MsgHandler>(&self, h: H) {
        self.0.handlers.borrow_mut().push(Box::new(h));
    }
}

impl Stream for AWatcher {
    type Item = ConnectionItem;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let inner = &*self.0;
        match inner.items.borrow_mut().pop_front() {
            Some(item) => {
                Ok(Async::Ready(Some(item)))
            }
            None => {
                let p = task::park();
                *inner.task.borrow_mut() = Some(p);
                Ok(Async::NotReady)
            }
        }
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
struct AWatch2 {
    io: PollEvented<AWatch>, 
    parent: Weak<AWInner>,
    quitrx: oneshot::Receiver<()>,
}

impl Future for AWatch2 {
    type Item = ();
    type Error = ();
    
    fn poll(&mut self) -> Result<Async<()>, ()> {
        let q = self.quitrx.poll();
        if q != Ok(Async::NotReady) { return Ok(Async::Ready(())); }

        let canread = self.io.poll_read().is_ready();
        let canwrite = self.io.poll_write().is_ready();
        let flags = 
           if canread { WatchEvent::Readable as c_uint } else { 0 } +
           if canwrite { WatchEvent::Writable as c_uint } else { 0 };

        // Not sure why are we woken up if we can't do anything, but seems to happen in practice
        if flags == 0 { return Ok(Async::NotReady) };

        if let Some(parent) = self.parent.upgrade() {
            let items = parent.conn.watch_handle(self.io.get_ref().0.fd(), flags);
            parent.handle_items(items);

            if canread { self.io.need_read() };
            if canwrite { self.io.need_write() };
            Ok(Async::NotReady) // Continue as normal
        } else { Ok(Async::Ready(())) }
    }
}

#[test]
fn aconnection_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let aconn = AConnection::new(conn.clone(), core.handle()).unwrap();

    let (tx, rx) = ::futures::sync::oneshot::channel();
    let m = ::dbus::Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let reply = conn.send_with_reply(m, move |r| {
        let z: Vec<&str> = r.get1().unwrap();
        println!("got reply: {:?}", z);
        tx.send(()).unwrap();
    });
    // awatcher.append();

    core.handle().spawn(aconn);
    core.run(rx).unwrap();
}

/*
#[test]
fn watch_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();

    conn.register_object_path("/test").unwrap();

    let awatcher = AWatcher::new(conn.clone(), &core.handle()).unwrap();

    let m = ::dbus::Message::new_method_call(&conn.unique_name(), "/test", "com.example.dbusrs.asynctest", "AsyncTest").unwrap();
    let serial = conn.send(m).unwrap();
    println!("sent serial {}", serial);

    let (tx, rx) = ::futures::sync::oneshot::channel();
    let mut tx = Some(tx);
    let conn2 = conn.clone();
    let f = awatcher.for_each(move |i| {
        println!("Received {:?}", i);
        match i {
            ConnectionItem::MethodCall(m) => {
               assert_eq!(m.headers(), (::dbus::MessageType::MethodCall, Some("/test".to_string()),
                   Some("com.example.dbusrs.asynctest".into()), Some("AsyncTest".to_string())));
                let mr = ::dbus::Message::method_return(&m).append1("Tokio");
                println!("Sending reply {:?}", mr);
                conn2.send(mr).unwrap();
            }
            ConnectionItem::MethodReturn(m) => {
                assert_eq!(m.get_reply_serial().unwrap(), serial);
                assert_eq!(m.get1(), Some("Tokio"));
                println!("Success!");
                tx.take().unwrap().send(()).unwrap();
            }
            _ => {}
        }
        Ok(())
    });

    core.handle().spawn(f);
    core.run(rx).unwrap();
}

#[test]
fn watch2_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let awatcher = AWatcher::new(conn.clone(), &core.handle()).unwrap();

    let (tx, rx) = ::futures::sync::oneshot::channel();
    let m = ::dbus::Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    awatcher.append(conn.send_with_reply(m, move |r| {
        let z: Vec<&str> = r.get1().unwrap();
        println!("got reply: {:?}", z);
        tx.send(()).unwrap();
    }));

    core.handle().spawn(awatcher.for_each(|i| { println!("Received {:?}", i); Ok(()) } ));
    core.run(rx).unwrap();
} */
