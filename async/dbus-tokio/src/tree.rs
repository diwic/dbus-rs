/// Async server-side trees

use std::{ops, fmt};
use dbus::tree::{Factory, Tree, MethodType, DataType, MTFn, Method, MethodInfo, MethodErr};
use dbus::{Member, Message, Connection};
use std::marker::PhantomData;
use std::cell::RefCell;
use futures::{IntoFuture, Future, Poll, Stream, Async};
use std::rc::Rc;
use super::AMessageStream;

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


pub struct AMethodResult(Box<Future<Item=Vec<Message>, Error=MethodErr>>);

impl fmt::Debug for AMethodResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "AMethodResult") }
}

impl AMethodResult {
    fn new<F: 'static + IntoFuture<Item=Vec<Message>, Error=MethodErr>>(f: F) -> Self {
        AMethodResult(Box::new(f.into_future()))
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
}

impl<'a, D: ADataType> ATreeServer<'a, D> {
    pub fn new(c: Rc<Connection>, t: &'a Tree<MTFn<ATree<D>>, ATree<D>>, stream: AMessageStream) -> Self {
        ATreeServer { conn: c, tree: t, stream: stream }
    }
}

impl<'a, D: ADataType> Stream for ATreeServer<'a, D> {
    type Item = Message;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let z = self.stream.poll()?;
println!("treeserver {:?}", z);
        if let Async::Ready(Some(z)) = z {
            let hh = self.tree.handle(&z);
println!("hh: {:?}", hh); 
            match hh {
                None => Ok(Async::Ready(Some(z))),
                Some(v) => {
                    for msg in v { self.conn.send(msg)?; };
                    Ok(Async::NotReady)
                },
            }
        } else { Ok(z) }
    }
}

