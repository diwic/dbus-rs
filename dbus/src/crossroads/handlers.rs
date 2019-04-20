use std::fmt;
use std::any::Any;
use crate::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature, Message, arg};
use super::crossroads::{Crossroads, PathData, MLookup};
use super::info::{MethodInfo, PropInfo};
use super::MethodErr;

pub struct DebugMethod<H: Handlers>(pub H::Method);
impl<H: Handlers> fmt::Debug for DebugMethod<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

pub struct DebugProp<H: Handlers>(pub Option<H::GetProp>, pub Option<H::SetProp>);
impl<H: Handlers> fmt::Debug for DebugProp<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

pub trait Handlers {
    type Method;
    type GetProp;
    type SetProp;
    type Iface;
}

/// Parallel tree - Par
#[derive(Debug, Clone, Copy, Default)]
pub struct Par;

impl Par { 
    pub fn typed_getprop<I: 'static, T: arg::Arg + arg::Append, G>(getf: G) -> <Par as Handlers>::GetProp
    where G: Fn(&I, &ParInfo) -> Result<T, MethodErr> + Send + Sync + 'static {
        Box::new(move |data, ia, info| {
            let iface: &I = data.downcast_ref().unwrap();
            let t = getf(iface, info)?;
            ia.append(t);
            Ok(())
        })
    }

    pub fn typed_setprop<I: 'static, T: arg::Arg + for <'z> arg::Get<'z>, S>(setf: S) -> <Par as Handlers>::SetProp
    where S: Fn(&I, &ParInfo, T) -> Result<(), MethodErr> + Send + Sync + 'static {
        Box::new(move |data, ii, info| {
            let iface: &I = data.downcast_ref().unwrap();
            let t: T = ii.read()?;
            setf(iface, info, t)
        })
    }
}

#[derive(Debug)]
pub struct ParInfo<'a> {
    lookup: MLookup<'a, Par>,
    message: &'a Message,
}

impl<'a> ParInfo<'a> {
    pub fn msg(&self) -> &Message { self.message }
    pub (super) fn new(msg: &'a Message, lookup: MLookup<'a, Par>) -> Self {
        ParInfo { lookup, message: msg }
    }
    pub fn path_data(&self) -> &PathData<Par> { self.lookup.data }
    pub fn crossroads(&self) -> &Crossroads<Par> { self.lookup.cr }
}

impl Handlers for Par {
    type Method = Box<Fn(&(dyn Any + Send + Sync), &ParInfo) -> Option<Message> + Send + Sync + 'static>;
    type GetProp = Box<Fn(&(dyn Any + Send + Sync), &mut arg::IterAppend, &ParInfo) -> Result<(), MethodErr> + Send + Sync + 'static>;
    type SetProp = Box<Fn(&(dyn Any + Send + Sync), &mut arg::Iter, &ParInfo) -> Result<(), MethodErr> + Send + Sync + 'static>;
    type Iface = Box<dyn Any + 'static + Send + Sync>;
}

impl MethodInfo<'_, Par> {
    pub fn new_par<N, F, T>(name: N, f: F) -> Self where
    F: Fn(&T, &ParInfo) -> Result<Option<Message>, MethodErr> + Send + Sync + 'static,
    N: Into<MemberName<'static>>,
    T: Any + Send + Sync + 'static,
    {
        Self::new(name.into(), Box::new(move |data, info| {
            let x = data.downcast_ref().unwrap();
            f(x, info).unwrap_or_else(|e| { Some(e.to_message(info.message)) })
        }))
    }
}

