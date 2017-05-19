// Low-level details, dealing with file descriptors etc

use mio::{self, unix};
use std::io;
use dbus::{Connection, ConnectionItems, ConnectionItem, Watch, WatchEvent};
use futures::{Async, Future, task, Stream, Poll};
use tokio_core::reactor::{PollEvented, Handle as CoreHandle};
use std::rc::Rc;
use std::os::raw::c_uint;
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Debug)]
struct AWInner(Rc<Connection>, RefCell<Option<task::Task>>, RefCell<VecDeque<ConnectionItem>>);

impl AWInner {
    fn handle_items(&self, items: ConnectionItems) {
        {
            let mut ci = self.2.borrow_mut();
            for item in items {
                println!("Transfer {:?}", item);
                ci.push_back(item);
            }
        }
        let t = self.1.borrow_mut().take();
        println!("Unparking stream {:?}", t);
        t.map(|t| t.unpark());
    }
}

#[derive(Debug)]
pub struct AWatcher(Rc<AWInner>);

impl AWatcher {
    pub fn new(c: Rc<Connection>, h: &CoreHandle) -> io::Result<AWatcher> {
        let i = Rc::new(AWInner(c, RefCell::new(None), RefCell::new(VecDeque::new())));
        for w in i.0.watch_fds() {
            h.spawn(AWatch2(PollEvented::new(AWatch(w), h)?, i.clone()).map_err(|e| panic!(e)));
        }
        Ok(AWatcher(i))
    }
}

impl Stream for AWatcher {
    type Item = ConnectionItem;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("Poll stream");
        let inner = &*self.0;
        match inner.2.borrow_mut().pop_front() {
            Some(item) => {
                Ok(Async::Ready(Some(item)))
            }
            None => {
                let p = task::park();
                println!("Parking stream {:?}", p);
                *inner.1.borrow_mut() = Some(p);
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
        println!("Register: {:?} {:?}", self, interest);
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
struct AWatch2(PollEvented<AWatch>, Rc<AWInner>);

impl Future for AWatch2 {
    type Item = ();
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<()>> {
        println!("Wakeup: {:?} {:?} {:?}", self, self.0.poll_read(), self.0.poll_write());

        let canread = self.0.poll_read().is_ready();
        let canwrite = self.0.poll_write().is_ready();
        let flags = 
           if canread { WatchEvent::Readable as c_uint } else { 0 } +
           if canwrite { WatchEvent::Writable as c_uint } else { 0 };

        if flags == 0 { return Ok(Async::NotReady) }; // Not sure why are we woken up if we can't do anything, but seems to happen in practice

        let items = (self.1).0.watch_handle(self.0.get_ref().0.fd(), flags);
        self.1.handle_items(items);
        if canread { self.0.need_read() };
        if canwrite { self.0.need_write() };
        Ok(Async::NotReady) // Continue forever
    }
}

#[test]
fn watch_test() {
    let conn = Rc::new(Connection::get_private(::dbus::BusType::Session).unwrap());
    let mut core = ::tokio_core::reactor::Core::new().unwrap();

    conn.register_object_path("/test").unwrap();

    let awatcher = AWatcher::new(conn.clone(), &core.handle()).unwrap();
    let conn2 = conn.clone();

    let m = ::dbus::Message::new_method_call(&conn.unique_name(), "/test", "com.example.dbusrs.asynctest", "AsyncTest").unwrap();
    let serial = conn.send(m).unwrap();
    println!("sent serial {}", serial);

    let (tx, rx) = ::futures::sync::oneshot::channel();
    let mut tx = Some(tx);
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
    // core.run(::futures::empty::<(), ()>()).unwrap();
}
