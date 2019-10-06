use crate::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use crate::Message;
use std::borrow::Cow;
use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::mem;
use crate::arg::{Arg, Append, AppendAll, ReadAll, ArgAll, Get, TypeMismatchError, IterAppend};
use std::marker::PhantomData;
use super::MethodErr;
use super::handlers::{Handlers, MakeHandler, DebugMethod, DebugProp, Par};
use super::crossroads::{Crossroads, RegEntry};
use super::path::Path;

fn build_argvec<A: ArgAll>(a: A::strs) -> Vec<Argument<'static>> {
    let mut v = vec!();
    A::strs_sig(a, |name, sig| {
        v.push(Argument { name: name.into(), sig, anns: Default::default() })
    });
    v
}

pub (super) type Annotations = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct Argument<'a> {
    pub (super) name: Cow<'a, str>,
    pub (super) sig: Signature<'a>,
    pub (super) anns: Annotations,
}

#[derive(Debug)]
pub struct IfaceInfo<'a, H: Handlers> {
    pub (super) name: IfaceName<'a>,
    pub (super) methods: Vec<MethodInfo<'a, H>>,
    pub (super) props: Vec<PropInfo<'a, H>>,
    pub (super) signals: Vec<SignalInfo<'a>>,
    pub (super) anns: Annotations,
}

#[derive(Debug)]
pub struct MethodInfo<'a, H: Handlers> {
    pub (super) name: MemberName<'a>,
    handler: DebugMethod<H>,
    pub (super) i_args: Vec<Argument<'a>>,
    pub (super) o_args: Vec<Argument<'a>>,
    pub (super) anns: Annotations,
}

impl<'a, H: Handlers> MethodInfo<'a, H> {
    pub fn name(&self) -> &MemberName<'a> { &self.name }
    pub fn handler(&self) -> &H::Method { &self.handler.0 }
    pub fn handler_mut(&mut self) -> &mut H::Method { &mut self.handler.0 }
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
    pub (super) name: MemberName<'a>,
    pub (super) handlers: DebugProp<H>,
    pub (super) anns: Annotations,
    pub (super) sig: Signature<'a>,
    pub (super) access: Access,
}

#[derive(Debug)]
pub struct SignalInfo<'a> {
    pub (super) name: MemberName<'a>,
    pub (super) args: Vec<Argument<'a>>,
    pub (super) anns: Annotations,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum MetSigProp { Method, Signal, Prop }

#[derive(Debug)]
pub struct IfaceInfoBuilder<'a, I: 'static, H: Handlers> {
    cr: Option<&'a mut Crossroads<H>>,
    entry: RegEntry<H>,
    last: Option<MetSigProp>,
    _dummy: PhantomData<*const I>,
}

impl<'a, I: 'static, H: Handlers> IfaceInfoBuilder<'a, I, H> {
    pub fn new(cr: Option<&'a mut Crossroads<H>>, name: IfaceName<'static>) -> Self {
        let entry = RegEntry::new::<I>(name);
        IfaceInfoBuilder { cr, _dummy: PhantomData, entry, last: None }
    }

    /// Adds a signal to this interface.
    pub fn signal<A: ArgAll, N: Into<MemberName<'static>>>(mut self, name: N, args: A::strs) -> Self {
        let s = SignalInfo { name: name.into(), args: build_argvec::<A>(args), anns: Default::default() };
        self.entry.info.signals.push(s);
        self.last = Some(MetSigProp::Signal);
        self
    }

    /// Annotates the last added method, signal or property, or the interface itself if nothing is added.
    pub fn annotate<N: Into<String>, V: Into<String>>(mut self, name: N, value: V) -> Self {
         let x: &mut Annotations = match self.last {
             None => &mut self.entry.info.anns,
             Some(MetSigProp::Method) => &mut self.entry.info.methods.last_mut().unwrap().anns,
             Some(MetSigProp::Signal) => &mut self.entry.info.signals.last_mut().unwrap().anns,
             Some(MetSigProp::Prop) => &mut self.entry.info.props.last_mut().unwrap().anns,
         };
         x.insert(name.into(), value.into());
         self
    }

    /// Adds a deprecated annotation to the last added method/signal/property.
    pub fn deprecated(self) -> Self { self.annotate("org.freedesktop.DBus.Deprecated", "true") }

    /// Adds an EmitChangedSignal annotation to the last added property, or the interface itself if nothing is added.
    ///
    /// Panics if the last added thing was a method or a signal.
    pub fn emits_changed(self, e: EmitsChangedSignal) -> Self {
        let c = match e {
            EmitsChangedSignal::True => "true",
            EmitsChangedSignal::False => "false",
            EmitsChangedSignal::Invalidates => "invalidates",
            EmitsChangedSignal::Const => "const",
        };
        match self.last {
            None | Some(MetSigProp::Prop) => self.annotate("org.freedesktop.DBus.Property.EmitsChangedSignal", c),
            _ => panic!("Cannot add EmitsChangedSignal to a method or signal"),
        }
    }

    /// Modifies the "Access" attribute for a property.
    ///
    /// Panics if the last added thing was not a property.
    pub fn access(mut self, a: Access) -> Self {
        if self.last != Some(MetSigProp::Prop) { panic!("Cannot modify access on a non property") }
        self.entry.info.props.last_mut().unwrap().access = a;
        self
    }

    /// Add a method to this interface. Input and output argument types will automatically be converted.
    pub fn method<IA: ReadAll + ArgAll, OA: AppendAll + ArgAll, N, F, D>(self, name: N, in_args: IA::strs, out_args: OA::strs, f: F) -> Self
    where N: Into<MemberName<'static>>, F: MakeHandler<<H as Handlers>::Method, ((), IA, OA, I), D> {
        self.method_custom::<IA, OA>(name.into(), in_args, out_args, f.make())
    }

    /// Adds a method to this interface. Input and output arguments are used for introspection, callback
    /// needs to convert arguments.
    pub fn method_custom<IA: ArgAll, OA: ArgAll>(mut self, name: MemberName<'static>, in_args: IA::strs, out_args: OA::strs, f: H::Method) -> Self {
        let m = MethodInfo { name, handler: DebugMethod(f), 
            i_args: build_argvec::<IA>(in_args), o_args: build_argvec::<OA>(out_args), anns: Default::default() };
        self.entry.info.methods.push(m);
        self.last = Some(MetSigProp::Method);
        self
    }

    /// Adds a property to this interface.
    pub fn prop_custom(mut self, name: MemberName<'static>, sig: Signature<'static>, get: Option<H::GetProp>, set: Option<H::SetProp>) -> Self {
        let p = PropInfo::new(name, sig, get, set);
        self.entry.info.props.push(p);
        self.last = Some(MetSigProp::Prop);
        self
    }

    /// Adds a read-only property to this interface.
    pub fn prop_ro<T: Arg, N: Into<MemberName<'static>>, G, D>(self, name: N, getf: G) -> Self
    where G: MakeHandler<<H as Handlers>::GetProp, (i64, T, I), D> {
        self.prop_custom(name.into(), T::signature(), Some(getf.make()), None)
    }

    /// Adds a read-write property to this interface.
    pub fn prop_rw<T: Arg, N: Into<MemberName<'static>>, G, S, D>(self, name: N, getf: G, setf: S) -> Self
    where G: MakeHandler<<H as Handlers>::GetProp, (i64, T, I), D>,
        S: MakeHandler<<H as Handlers>::SetProp, (u64, T, I), D> {
        self.prop_custom(name.into(), T::signature(), Some(getf.make()), Some(setf.make()))
    }

    pub (super) fn on_path_insert<F: Fn(&mut Path<H>, &Crossroads<H>) + 'static + Send + Sync>(mut self, f: F) -> Self {
        self.entry.path_insert = Some(Box::new(f));
        self
    }
}

impl<'a, I: 'static, H: Handlers> Drop for IfaceInfoBuilder<'a, I, H> {
    fn drop(&mut self) {
        if let Some(ref mut cr) = self.cr {
            let n = self.entry.info.name.clone();
            let entry = mem::replace(&mut self.entry, RegEntry::new::<()>(n.clone())); // workaround for not being able to consume self.entry
            cr.reg.insert(n.into_cstring(), entry);
        }
    }
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
            _ => Access::Read,
        };
        PropInfo { name, handlers: DebugProp(get, set), sig, access: a, anns: Default::default() }
    }
}

impl<'a, H: Handlers> IfaceInfo<'a, H> {
    pub fn new_empty(name: IfaceName<'static>) -> Self {
        IfaceInfo { name, methods: vec!(), props: vec!(), signals: vec!(), anns: Default::default(), }
    }

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
            signals: signals.into_iter().collect(),
            anns: Default::default(),
        }
    }
}

