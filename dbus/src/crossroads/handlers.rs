use std::fmt;
use std::any::Any;

pub struct DebugWrapper<T>(pub T);
impl<T> fmt::Debug for DebugWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

impl<T> From<T> for DebugWrapper<T> {
    fn from(t: T) -> Self { DebugWrapper(t) }
}

pub trait Handlers {
    type Method: fmt::Debug;
    type GetProp: fmt::Debug;
    type SetProp: fmt::Debug;
    type DynHandlers: Handlers;
    type Iface;
}

impl Handlers for () {
    type Method = ();
    type GetProp = ();
    type SetProp = ();
    type DynHandlers = ();   
    type Iface = Box<dyn Any + 'static + Send + Sync>;
}
