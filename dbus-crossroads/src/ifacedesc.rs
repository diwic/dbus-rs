use std::marker::PhantomData;
use crate::{Context, MethodErr, Crossroads};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::borrow::Cow;

#[derive(Default, Debug)]
pub struct Registry(Vec<IfaceDesc>);

impl Registry {
    pub fn push(&mut self, x: IfaceDesc) -> usize {
        self.0.push(x);
        self.0.len() - 1
    }

    pub fn find_token(&self, name: Option<&dbus::strings::Interface>, tokens: &HashSet<usize>) -> Result<usize, MethodErr> {
        for &t in tokens.iter() {
            let desc = &self.0[t];
            if desc.name.as_ref() == name { return Ok(t) }
        }
        Err(name.map(MethodErr::no_interface).unwrap_or_else(|| MethodErr::no_interface("")))
    }

    pub fn take_method(&mut self, t: usize, name: &dbus::strings::Member<'static>) -> Result<Callback, MethodErr> {
        let mdesc = self.0[t].methods.get_mut(name).ok_or_else(|| MethodErr::no_method(name))?;
        let cb = mdesc.cb.take();
        let cb = cb.ok_or_else(|| MethodErr::failed(&format!("Detected recursive call to {}", name)))?;
        Ok(cb.0)
    }

    pub fn give_method(&mut self, t: usize, name: &dbus::strings::Member<'static>, cb: Callback) {
        let x = self.0[t].methods.get_mut(name).unwrap();
        x.cb = Some(CallbackDbg(cb));
    }

    pub fn prop_names_readable(&self, t: usize) -> Vec<String> {
        self.0[t].properties.iter().filter_map(|(k, v)| {
            if v.access != Access::Write && v.get_cb.is_some() { Some(k.clone()) } else { None }
        }).collect()
    }

    pub fn take_getprop(&mut self, t: usize, name: &str) -> Result<Callback, MethodErr> {
        let pdesc = self.0[t].properties.get_mut(name).ok_or_else(|| MethodErr::no_property(name))?;
        let cb = pdesc.get_cb.take();
        let cb = cb.ok_or_else(|| MethodErr::failed(&format!("Detected recursive call to get property {}", name)))?;
        Ok(cb.0)
    }

    pub fn give_getprop(&mut self, t: usize, name: &str, cb: Callback) {
        self.0[t].properties.get_mut(name).unwrap().get_cb = Some(CallbackDbg(cb));
    }

    pub fn has_props(&self, t: usize) -> bool { !self.0[t].properties.is_empty() }
}

pub type Callback = Box<dyn FnMut(Context, &mut Crossroads) -> Option<Context> + Send + 'static>;

struct CallbackDbg(Callback);

impl fmt::Debug for CallbackDbg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Callback") }
}

#[derive(Debug, Clone, Default)]
pub struct Annotations(HashMap<String, String>);

#[derive(Debug, Clone)]
struct Argument {
    name: Cow<'static, str>,
    sig: dbus::Signature<'static>,
    annotations: Option<Annotations>,
}

#[derive(Debug, Clone)]
pub struct Arguments(Vec<Argument>);

#[derive(Debug)]
pub struct MethodDesc {
    cb: Option<CallbackDbg>,
    input_args: Arguments,
    output_args: Arguments,
    annotations: Option<Annotations>,
}

#[derive(Debug)]
pub struct SignalDesc {
    args: Arguments,
    annotations: Option<Annotations>,
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
pub struct PropDesc {
    annotations: Option<Annotations>,
    sig: dbus::Signature<'static>,
    get_cb: Option<CallbackDbg>,
    set_cb: Option<CallbackDbg>,
    access: Access,
}

#[derive(Debug)]
pub struct IfaceDesc {
    name: Option<dbus::strings::Interface<'static>>,
    annotations: Option<Annotations>,
    methods: HashMap<dbus::strings::Member<'static>, MethodDesc>,
    signals: HashMap<dbus::strings::Member<'static>, SignalDesc>,
    properties: HashMap<String, PropDesc>,
}

fn build_argvec<A: dbus::arg::ArgAll>(a: A::strs) -> Arguments {
    let mut v = vec!();
    A::strs_sig(a, |name, sig| {
        v.push(Argument { name: name.into(), sig, annotations: None })
    });
    Arguments(v)
}


#[derive(Debug)]
pub struct PropBuilder<'a, T:'static, A: 'static>(&'a mut PropDesc, PhantomData<&'static (T, A)>);

impl<T: std::marker::Send, A: dbus::arg::Arg +  dbus::arg::RefArg + dbus::arg::Append> PropBuilder<'_, T, A> {
    pub fn get<CB>(self, mut cb: CB) -> Self
    where CB: FnMut(&mut Context, &mut T) -> Result<A, MethodErr> + Send + 'static {
        self.get_with_cr(move |ctx, cr| {
            let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            cb(ctx, data)
        })
    }

    pub fn get_with_cr<CB>(mut self, mut cb: CB) -> Self
    where CB: FnMut(&mut Context, &mut Crossroads) -> Result<A, MethodErr> + Send + 'static {
        self.0.get_cb = Some(CallbackDbg(Box::new(move |mut ctx, cr| {
            let _ = ctx.check(|ctx| {
                let r = cb(ctx, cr)?;
                ctx.prop_ctx().add_get_result(r);
                Ok(())
            });
            Some(ctx)
        })));
        self
    }
}

#[derive(Debug)]
pub struct IfaceBuilder<T: Send + 'static>(IfaceDesc, PhantomData<&'static T>);

impl<T: Send + 'static> IfaceBuilder<T> {
    pub fn property<A: dbus::arg::Arg, N: Into<String>>(&mut self, name: N) -> PropBuilder<T, A> {
        PropBuilder(self.0.properties.entry(name.into()).or_insert(PropDesc {
            access: Access::Read,
            annotations: None,
            get_cb: None,
            set_cb: None,
            sig: A::signature(),
        }), PhantomData)
    }

    pub fn method<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: dbus::arg::ArgAll + dbus::arg::ReadAll, OA: dbus::arg::ArgAll + dbus::arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&mut Context, &mut T, IA) -> Result<OA, MethodErr> + Send + 'static {
        self.method_with_cr(name, input_args, output_args, move |ctx, cr, ia| {
            let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            cb(ctx, data, ia)
        })
    }

    pub fn method_with_cr<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: dbus::arg::ArgAll + dbus::arg::ReadAll, OA: dbus::arg::ArgAll + dbus::arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&mut Context, &mut Crossroads, IA) -> Result<OA, MethodErr> + Send + 'static {
        let boxed = Box::new(move |mut ctx: Context, cr: &mut Crossroads| {
            let _ = ctx.check(|ctx| {
                let ia = ctx.message().read_all()?;
                let oa = cb(ctx, cr, ia)?;
                ctx.do_reply(|mut msg| oa.append(&mut dbus::arg::IterAppend::new(&mut msg)));
                Ok(())
            });
            Some(ctx)
        });
        self.0.methods.entry(name.into()).or_insert(MethodDesc {
            annotations: None,
            input_args: build_argvec::<IA>(input_args),
            output_args: build_argvec::<OA>(output_args),
            cb: Some(CallbackDbg(boxed)),
        })
    }

    pub fn method_with_cr_async<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: dbus::arg::ArgAll + dbus::arg::ReadAll, OA: dbus::arg::ArgAll + dbus::arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(Context, &mut Crossroads, IA) -> Option<Context> + Send + 'static {
        let boxed = Box::new(move |mut ctx: Context, cr: &mut Crossroads| {
            match ctx.check(|ctx| Ok(ctx.message().read_all()?)) {
                Ok(ia) => cb(ctx, cr, ia),
                Err(_) => Some(ctx),
            }
        });
        self.0.methods.entry(name.into()).or_insert(MethodDesc {
            annotations: None,
            input_args: build_argvec::<IA>(input_args),
            output_args: build_argvec::<OA>(output_args),
            cb: Some(CallbackDbg(boxed)),
        })
    }

    pub fn signal<A, N>(&mut self, name: N, args: A::strs) -> &mut SignalDesc
    where A: dbus::arg::ArgAll, N: Into<dbus::strings::Member<'static>> {
        self.0.signals.entry(name.into()).or_insert(SignalDesc {
            annotations: None,
            args: build_argvec::<A>(args),
        })
    }

    pub (crate) fn build<F>(name: Option<dbus::strings::Interface<'static>>, f: F) -> IfaceDesc
    where F: FnOnce(&mut IfaceBuilder<T>) {
        let mut b = IfaceBuilder(IfaceDesc {
            name,
            annotations: None,
            methods: Default::default(),
            signals: Default::default(),
            properties: Default::default(),
        }, PhantomData);
        f(&mut b);
        b.0
    }
}
