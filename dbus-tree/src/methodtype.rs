// Methods and method types. Glue to make stuff generic over MFn, MFnMut and MSync

use std::fmt;
use dbus::Message;
use dbus::ffidisp::stdintf;
use dbus::arg::{Iter, IterAppend};
use std::marker::PhantomData;
use super::{Method, Interface, Property, ObjectPath, Tree};
use std::cell::RefCell;
use dbus::MethodErr;

/// Result containing the Messages returned from the Method, or a MethodErr.
pub type MethodResult = Result<Vec<Message>, MethodErr>;

/// Associated data for different objects in a tree.
///
/// These currently require a debug bound, due to https://github.com/rust-lang/rust/issues/31518
pub trait DataType: Sized + Default {
    /// Type of associated data on the Tree.
    type Tree: fmt::Debug;
    /// Type of associated data on every ObjectPath.
    type ObjectPath: fmt::Debug;
    /// Type of associated data on every Property.
    type Property: fmt::Debug;
    /// Type of associated data on every Interface.
    type Interface: fmt::Debug;
    /// Type of associated data on every Method.
    type Method: fmt::Debug;
    /// Type of associated data on every Signal.
    type Signal: fmt::Debug;
}

/// No associated data for the tree.
impl DataType for () {
    type Tree = ();
    type ObjectPath = ();
    type Interface = ();
    type Property = ();
    type Method = ();
    type Signal = ();
}

/// A helper trait used internally to make the tree generic over MTFn, MTFnMut and MTSync.
///
/// You should not need to call these methods directly, it's primarily for internal use.
pub trait MethodType<D: DataType>: Sized + Default {
    /// For internal use.
    type Method: ?Sized;
    /// For internal use.
    type GetProp: ?Sized;
    /// For internal use.
    type SetProp: ?Sized;

    /// For internal use.
    fn call_getprop(_: &Self::GetProp, _: &mut IterAppend, _: &PropInfo<Self, D>) -> Result<(), MethodErr>;
    /// For internal use.
    fn call_setprop(_: &Self::SetProp, _: &mut Iter, _: &PropInfo<Self, D>) -> Result<(), MethodErr>;
    /// For internal use.
    fn call_method(_: &Self::Method, _: &MethodInfo<Self, D>) -> MethodResult;

    /// For internal use.
    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    /// For internal use.
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static;
}


/// An abstract type to represent Fn functions.
#[derive(Default, Debug, Copy, Clone)]
pub struct MTFn<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTFn<D> {
    type GetProp = dyn Fn(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    type SetProp = dyn Fn(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    type Method = dyn Fn(&MethodInfo<Self, D>) -> MethodResult;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { p(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static { Box::new(h) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(h) }
}

/// An abstract type to represent FnMut functions.
#[derive(Default, Debug, Copy, Clone)]
pub struct MTFnMut<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTFnMut<D> {
    type GetProp = RefCell<dyn FnMut(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr>>;
    type SetProp = RefCell<dyn FnMut(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr>>;
    type Method = RefCell<dyn FnMut(&MethodInfo<Self, D>) -> MethodResult>;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { (&mut *p.borrow_mut())(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { (&mut *p.borrow_mut())(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { (&mut *p.borrow_mut())(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static { Box::new(RefCell::new(h)) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(RefCell::new(h)) }

}

/// An abstract type to represent Fn + Send + Sync functions (that can be called from several threads in parallel).
#[derive(Default, Debug, Copy, Clone)]
pub struct MTSync<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTSync<D> {
    type GetProp = dyn Fn(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    type SetProp = dyn Fn(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    type Method = dyn Fn(&MethodInfo<Self, D>) -> MethodResult + Send + Sync + 'static;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { p(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static  { Box::new(h) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(h) }
}



#[derive(Debug, Copy, Clone)]
/// Contains information about the incoming method call.
pub struct MethodInfo<'a, M: 'a + MethodType<D>, D: 'a + DataType> {
    /// Message
    pub msg: &'a Message,
    /// The method to be called
    pub method: &'a Method<M, D>,
    /// Interface
    pub iface: &'a Interface<M, D>,
    /// Object path
    pub path: &'a ObjectPath<M, D>,
    /// Tree
    pub tree: &'a Tree<M, D>,
}

impl<'a, M: 'a + MethodType<D>, D: 'a + DataType> MethodInfo<'a, M, D> {
    /// MethodInfo to PropInfo conversion
    pub fn to_prop_info(&self, iface: &'a Interface<M, D>, prop: &'a Property<M, D>) -> PropInfo<'a, M, D> {
        PropInfo { msg: self.msg, method: self.method, iface: iface, prop: prop, path: self.path, tree: self.tree }
    }
}


impl<'a, M: 'a + MethodType<D>, D: 'a + DataType> stdintf::OrgFreedesktopDBusIntrospectable for MethodInfo<'a, M, D> {
    type Err = MethodErr;
    fn introspect(&self) -> Result<String, Self::Err> { Ok(self.path.introspect(self.tree)) }
}

// Mostly autogenerated by dbus-codegen
pub fn org_freedesktop_dbus_introspectable_server<M, D>(factory: &super::Factory<M, D>, data: D::Interface) -> super::Interface<M, D>
where
    D: super::DataType,
    D::Method: Default,
    M: MethodType<D>,
{
    let i = factory.interface("org.freedesktop.DBus.Introspectable", data);
    let h = move |minfo: &super::MethodInfo<M, D>| {
        let d: &dyn stdintf::OrgFreedesktopDBusIntrospectable<Err=super::MethodErr> = minfo;
        let arg0 = d.introspect()?;
        let rm = minfo.msg.method_return();
        let rm = rm.append1(arg0);
        Ok(vec!(rm))
    };
    let m = factory.method_sync("Introspect", Default::default(), h);
    let m = m.out_arg(("xml_data", "s"));
    i.add_m(m)
}

#[derive(Debug, Copy, Clone)]
/// Contains information about the incoming property get/set request.
pub struct PropInfo<'a, M: 'a + MethodType<D>, D: 'a + DataType> {
    /// Message
    pub msg: &'a Message,
    /// Get, Set or GetAll
    pub method: &'a Method<M, D>,
    /// The property to be set/get
    pub prop: &'a Property<M, D>,
    /// The interface the property belongs to
    pub iface: &'a Interface<M, D>,
    /// Object path
    pub path: &'a ObjectPath<M, D>,
    /// Tree
    pub tree: &'a Tree<M, D>,
}

impl<'a, M: 'a + MethodType<D>, D: 'a + DataType> PropInfo<'a, M, D> {
    /// PropInfo to MethodInfo conversion.
    pub fn to_method_info(&self) -> MethodInfo<'a, M, D> {
        MethodInfo { msg: self.msg, method: self.method, iface: self.iface, path: self.path, tree: self.tree }
    }
}
