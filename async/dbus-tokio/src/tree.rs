/// Async server-side trees

use std::{ops, fmt, mem};
use dbus::tree::{Factory, Tree, MethodType, DataType, MTFn, Method, MethodInfo, MethodErr};
use dbus::{Member, Message, Connection};
use std::marker::PhantomData;
use std::cell::RefCell;
use futures::{IntoFuture, Future, Poll, Stream, Async};
use std::rc::Rc;
use super::AMessageStream;
use std::ffi::CString;

pub trait ADataType: fmt::Debug + Sized + Default {
    type ObjectPath: fmt::Debug;
    type Property: fmt::Debug;
    type Interface: fmt::Debug + Default;
    type Method: fmt::Debug + Default;
    type Signal: fmt::Debug;    
}

#[derive(Debug, Default)]
pub struct ATree<D: ADataType>(RefCell<Vec<AMethodResult>>, PhantomData<*const D>);

impl<D: ADataType> ATree<D> {
    pub fn new() -> Self { Default::default() }
    fn push(&self, a: AMethodResult) { self.0.borrow_mut().push(a); }
}

impl<D: ADataType> DataType for ATree<D> {
    type Tree = ATree<D>;
    type ObjectPath = D::ObjectPath;
    type Property = D::Property;
    type Interface = D::Interface;
    type Method = D::Method;
    type Signal = D::Signal;
}

impl ADataType for () {
    type ObjectPath = ();
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
}

pub struct AFactory<M: MethodType<D>, D: DataType = ()>(Factory<M, D>);

impl AFactory<MTFn<()>, ()> {
    pub fn new_afn<D: ADataType>() -> AFactory<MTFn<ATree<D>>, ATree<D>> { AFactory(Factory::new_fn()) }
}

impl<M: MethodType<D>, D: DataType> ops::Deref for AFactory<M, D> {
    type Target = Factory<M, D>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<D: ADataType> AFactory<MTFn<ATree<D>>, ATree<D>> {
    pub fn amethod<H, R, T>(&self, t: T, data: D::Method, handler: H) -> Method<MTFn<ATree<D>>, ATree<D>>
    where H: 'static + Fn(&MethodInfo<MTFn<ATree<D>>, ATree<D>>) -> R, T: Into<Member<'static>>,
        R: 'static + IntoFuture<Item=Vec<Message>, Error=MethodErr> {
        self.0.method(t, data, move |minfo| {
            let r = handler(minfo);
            minfo.tree.get_data().push(AMethodResult::new(r));
            Ok(Vec::new())
        })
    }
}


pub struct AMethodResult(Box<Future<Item=Vec<Message>, Error=MethodErr>>, Option<Message>);

impl fmt::Debug for AMethodResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "AMethodResult({:?})", self.1) }
}

impl AMethodResult {
    fn new<F: 'static + IntoFuture<Item=Vec<Message>, Error=MethodErr>>(f: F) -> Self {
        AMethodResult(Box::new(f.into_future()), None)
    }
}

impl Future for AMethodResult {
    type Item = Vec<Message>;
    type Error = MethodErr;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}


#[derive(Debug)]
pub struct ATreeServer<'a, D: ADataType + 'a> {
   conn: Rc<Connection>,
   tree: &'a Tree<MTFn<ATree<D>>, ATree<D>>,
   stream: AMessageStream,
   pendingresults: Vec<AMethodResult>,
}

impl<'a, D: ADataType> ATreeServer<'a, D> {
    pub fn new(c: Rc<Connection>, t: &'a Tree<MTFn<ATree<D>>, ATree<D>>, stream: AMessageStream) -> Self {
        ATreeServer { conn: c, tree: t, stream: stream, pendingresults: vec![] }
    }

    fn spawn_method_results(&mut self, msg: Message) {
        let v = mem::replace(&mut *self.tree.get_data().0.borrow_mut(), vec![]);
        let mut msg = Some(msg);
        for mut r in v {
            if r.1.is_none() { r.1 = msg.take() };
            println!("Pushing {:?}", r);
            self.pendingresults.push(r);
        }
    }

    fn check_pending_results(&mut self) {
        let v = mem::replace(&mut self.pendingresults, vec!());
        self.pendingresults = v.into_iter().filter_map(|mut mr| {
            let z = mr.poll();
            println!("Polling {:?} returned {:?}", mr, z);
            match z {
                Ok(Async::NotReady) => Some(mr),
                Ok(Async::Ready(t)) => { for msg in t { println!("Sending {:?}", msg); self.conn.send(msg).expect("D-Bus send error"); }; None },
                Err(e) => {
                    let m = mr.1.take().unwrap(); 
                    let msg = m.error(&e.errorname(), &CString::new(e.description()).unwrap());
                    self.conn.send(msg).expect("D-Bus send error");
                    None
                }
            }
        }).collect();
    }
}

impl<'a, D: ADataType> Stream for ATreeServer<'a, D> {
    type Item = Message;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            self.check_pending_results();
            let z = self.stream.poll();
            if let Ok(Async::Ready(Some(m))) = z {
                println!("treeserver {:?}", m);
                let hh = self.tree.handle(&m);
                println!("hh: {:?}", hh);
                if let Some(v) = hh {
                    self.spawn_method_results(m);
                    for msg in v { self.conn.send(msg)?; }
                    // We consumed the message. Poll again
                } else { return Ok(Async::Ready(Some(m))) }
            } else { return z }
        }
    }
}

