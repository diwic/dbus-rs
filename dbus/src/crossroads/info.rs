use crate::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use std::borrow::Cow;
use std::collections::BTreeMap;
use super::handlers::Handlers;

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
    methods: Vec<MethodInfo<'a, H>>,
    props: Vec<PropInfo<'a, H>>,
    signals: Vec<SignalInfo<'a>>,
}

#[derive(Debug)]
pub struct MethodInfo<'a, H: Handlers> {
    name: MemberName<'a>,
    handler: H::Method,
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
    get_handler: H::GetProp,
    set_handler: H::SetProp,
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
    pub fn new<N: Into<MemberName<'static>>, F: Into<H::Method>>(name: N, f: F) -> Self {
        MethodInfo { name: name.into(), handler: f.into(),
            i_args: Default::default(), o_args: Default::default(), anns: Default::default() }
    }
}

