use std::fmt;
use std::any::Any;
use crate::Message;
use super::crossroads::{Crossroads, PathData};

pub struct DebugMethod<H: Handlers>(pub H::Method);
impl<H: Handlers> fmt::Debug for DebugMethod<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

pub struct DebugProp<H: Handlers>(pub H::GetProp, pub H::SetProp);
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

impl Handlers for () {
    type Method = Box<Fn(&Crossroads<()>, &PathData<()>, &(dyn Any + Send + Sync), &Message) -> Vec<Message> + Send + Sync + 'static>;
    type GetProp = ();
    type SetProp = ();
    type DynHandlers = ();   
    type Iface = Box<dyn Any + 'static + Send + Sync>;
}
