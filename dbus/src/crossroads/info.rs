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
    pub (crate) name: MemberName<'a>,
    pub (crate) handlers: DebugProp<H>,
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
    pub fn new(name: MemberName<'static>, f: H::Method) -> Self {
        MethodInfo { name: name, handler: DebugMethod(f),
            i_args: Default::default(), o_args: Default::default(), anns: Default::default() }
    }
}

impl<H: Handlers> PropInfo<'_, H> {
    pub fn new(name: MemberName<'static>, sig: Signature<'static>, get: Option<H::GetProp>, 
        set: Option<H::SetProp>) -> Self {
        let a = match (&get, &set) {
            (Some(_), Some(_)) => Access::ReadWrite,
            (Some(_), None) => Access::Read,
            (None, Some(_)) => Access::Write,
            _ => unimplemented!(),
        };
        PropInfo { name, handlers: DebugProp(get, set), sig, auto_emit: true, rw: a, 
            emits: EmitsChangedSignal::True, anns: Default::default() }
    }
}

impl<'a, H: Handlers> IfaceInfo<'a, H> {
    pub fn new<N, M, P, S>(name: N, methods: M, properties: P, signals: S) -> Self where
        N: Into<IfaceName<'a>>, 
        M: IntoIterator<Item=MethodInfo<'a, H>>, 
        P: IntoIterator<Item=PropInfo<'a, H>>,
        S: IntoIterator<Item=SignalInfo<'a>>
    {
        IfaceInfo {
            name: name.into(),
            methods: methods.into_iter().collect(),
            props: properties.into_iter().collect(),
            signals: signals.into_iter().collect()
        }
    }
}

