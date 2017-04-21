/// Async server-side trees

use std::{ops, fmt, error};
use dbus::tree::{Factory, MethodType, DataType, MTFn, Method, MethodInfo, MethodErr, MethodResult};
use dbus::{Member, Message};
use std::marker::PhantomData;
use std::cell::RefCell;
use futures::{IntoFuture, Future, Poll};

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
    pub fn amethod<H, R, T, E>(&self, t: T, data: D::Method, handler: H) -> Method<MTFn<ATree<D>>, ATree<D>>
    where H: 'static + Fn(&MethodInfo<MTFn<ATree<D>>, ATree<D>>) -> R, T: Into<Member<'static>>,
        E: error::Error, R: IntoFuture<Item=Vec<Message>, Error=E> {
        self.0.method(t, data, move |minfo| {
            let r = handler(minfo);
            minfo.tree.get_data().push(AMethodResult::new(r));
            Ok(Vec::new())
        })
    }
}

#[derive(Debug)]
/// TODO
pub struct AMethodResult(());

impl AMethodResult {
    fn new<E: error::Error, F: IntoFuture<Item=Vec<Message>, Error=E>>(_: F) -> Self { unimplemented!() }
}

impl Future for AMethodResult {
    type Item = Vec<Message>;
    type Error = MethodErr;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> { unimplemented!() }
}

