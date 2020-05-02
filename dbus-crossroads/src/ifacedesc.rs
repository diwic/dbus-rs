use std::marker::PhantomData;
use crate::{Context, MethodErr, Crossroads, MethodFactory};
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
pub struct IfaceDesc {
    name: Option<dbus::strings::Interface<'static>>,
    annotations: Option<Annotations>,
    methods: HashMap<dbus::strings::Member<'static>, MethodDesc>,
}

impl IfaceDesc {
    pub fn new<N: Into<dbus::strings::Interface<'static>>>(name: N, annotations: Option<Annotations>) -> Self {
        IfaceDesc { name: Some(name.into()), annotations, methods: Default::default() }
    }

    pub fn method<IA, OA, N, CB>(mut self, name: N, annotations: Option<Annotations>, cb: CB) -> Self
    where N: Into<dbus::strings::Member<'static>>, CB: MethodFactory<IA, OA>,
    {
        let (input_args, output_args, cb) = cb.make_method();
        let md = MethodDesc { cb: Some(CallbackDbg(cb)), input_args, output_args, annotations };
        self.methods.insert(name.into(), md);
        self
    }
}

fn build_argvec<A: dbus::arg::ArgAll>(a: A::strs) -> Arguments {
    let mut v = vec!();
    A::strs_sig(a, |name, sig| {
        v.push(Argument { name: name.into(), sig, annotations: None })
    });
    Arguments(v)
}

#[derive(Debug)]
pub struct IfaceBuilder<T: Send + 'static>(IfaceDesc, PhantomData<&'static T>);

impl<T: Send + 'static> IfaceBuilder<T> {
    pub fn method<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: dbus::arg::ArgAll + dbus::arg::ReadAll, OA: dbus::arg::ArgAll + dbus::arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&mut Context, &mut T, IA) -> Result<OA, MethodErr> + Send + 'static {
        let boxed = Box::new(move |mut ctx: Context, cr: &mut Crossroads| {
            let _ = ctx.check(|ctx| {
                let ia = ctx.message().read_all()?;
                let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
                let oa = cb(ctx, data, ia)?;
                ctx.set_reply(|mut msg| oa.append(&mut dbus::arg::IterAppend::new(&mut msg)));
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

    pub (crate) fn build<F>(name: Option<dbus::strings::Interface<'static>>, f: F) -> IfaceDesc
    where F: FnOnce(&mut IfaceBuilder<T>) {
        let mut b = IfaceBuilder(IfaceDesc { name, annotations: None, methods: Default::default() }, PhantomData);
        f(&mut b);
        b.0
    }
}
