use crate::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature, Message};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::any::Any;
use super::handlers::{Handlers, DebugMethod, DebugProp};
use super::crossroads::{Crossroads, PathData};

#[derive(Default, Debug, Clone)]
struct Annotations(Option<BTreeMap<String, String>>);

#[derive(Debug, Clone)]
struct Argument<'a> {
    name: Option<Cow<'a, str>>,
    sig: Signature<'a>,
}

#[derive(Debug)]
pub struct IfaceInfo<'a, H: Handlers> {
    pub (crate) name: IfaceName<'a>,
    pub (crate) methods: Vec<MethodInfo<'a, H>>,
    pub (crate) props: Vec<PropInfo<'a, H>>,
    pub (crate) signals: Vec<SignalInfo<'a>>,
}

#[derive(Debug)]
pub struct MethodInfo<'a, H: Handlers> {
    pub (crate) name: MemberName<'a>,
    pub (crate) handler: DebugMethod<H>,
    i_args: Vec<Argument<'a>>,
    o_args: Vec<Argument<'a>>,
    anns: Annotations,
}

#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
/// Enumerates the different signaling behaviors a Property can have
/// to being changed.
pub enum EmitsChangedSignal {
    /// The Property emits a signal that includes the new value.
    True,
    /// The Property emits a signal that does not include the new value.
    Invalidates,
    /// The Property cannot be changed.
    Const,
    /// The Property does not emit a signal when changed.
    False,
}

#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
/// The possible access characteristics a Property can have.
pub enum Access {
    /// The Property can only be read (Get).
    Read,
    /// The Property can be read or written.
    ReadWrite,
    /// The Property can only be written (Set).
    Write,
}

#[derive(Debug)]
pub struct PropInfo<'a, H: Handlers> {
    name: MemberName<'a>,
    handlers: DebugProp<H>,
    anns: Annotations,
    sig: Signature<'a>,
    emits: EmitsChangedSignal,
    auto_emit: bool,
    rw: Access,
}

#[derive(Debug)]
pub struct SignalInfo<'a> {
    name: MemberName<'a>,
    args: Vec<Argument<'a>>,
    anns: Annotations,
}

impl<H: Handlers> MethodInfo<'_, H> {
    fn new(name: MemberName<'static>, f: H::Method) -> Self {
        MethodInfo { name: name, handler: DebugMethod(f),
            i_args: Default::default(), o_args: Default::default(), anns: Default::default() }
    }
}

impl MethodInfo<'_, ()> {
    pub fn new_sync<N, F>(name: N, f: F) -> Self where
    F: Fn(&Crossroads<()>, &PathData<()>, &(dyn Any + Send + Sync), &Message) -> Vec<Message> + Send + Sync + 'static,
    N: Into<MemberName<'static>>, 
    {
        Self::new(name.into(), Box::new(f))
    }
}


impl<'a, H: Handlers> IfaceInfo<'a, H> {
    pub fn new<N, M>(name: N, methods: M) -> Self where
        N: Into<IfaceName<'a>>, 
        M: IntoIterator<Item=MethodInfo<'a, H>> 
    {
        IfaceInfo { name: name.into(), methods: methods.into_iter().collect(),
            props: vec!(), signals: vec!() }
    }
}

