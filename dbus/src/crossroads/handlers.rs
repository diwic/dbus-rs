use std::fmt;
use std::any::Any;
use crate::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature, Message, arg};
use super::crossroads::{Crossroads, PathData};
use super::info::{MethodInfo, PropInfo};

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
    type DynHandlers: Handlers;
    type Iface;
}

//// Sync tree - ()

pub struct SyncInfo<'a> {
    pub (crate) cr: &'a Crossroads<()>,
    pub (crate) pd: &'a PathData<()>,
}

impl Handlers for () {
    type Method = Box<Fn(&(dyn Any + Send + Sync), &Message, &mut SyncInfo) -> Option<Message> + Send + Sync + 'static>;
    type GetProp = Box<Fn(&(dyn Any + Send + Sync), &Message, &mut arg::IterAppend, &mut SyncInfo) -> bool + Send + Sync + 'static>;
    type SetProp = ();
    type DynHandlers = ();   
    type Iface = Box<dyn Any + 'static + Send + Sync>;
}

impl MethodInfo<'_, ()> {
    pub fn new_sync<N, F, T>(name: N, f: F) -> Self where
    F: Fn(&T, &Message, &mut SyncInfo) -> Option<Message> + Send + Sync + 'static,
    N: Into<MemberName<'static>>,
    T: Any + Send + Sync + 'static,
    {
        Self::new(name.into(), Box::new(move |data, msg, si| {
            let x = data.downcast_ref().unwrap();
            f(x, msg, si)
        }))
    }
}

impl PropInfo<'_, ()> {
    pub fn new_sync_ro<P, N, G, T>(name: N, getf: G) -> Self where
    G: Fn(&T, &Message, &SyncInfo) -> Option<P> + Send + Sync + 'static,
//    S: Fn(&T, &Message, &SyncInfo, P) -> Option<()> + Send + Sync + 'static,
    N: Into<MemberName<'static>>,
    T: Any + Send + Sync + 'static,
    P: arg::Append + arg::Arg,
    {
        Self::new(name.into(), P::signature(), Some(Box::new(move |data, msg, ia, si| {
            let x = data.downcast_ref().unwrap();
            if let Some(t) = getf(x, msg, si) { ia.append(t); true }
            else { false }
        })), None)
    }
}

