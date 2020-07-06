use std::future::Future;
use std::marker::PhantomData;
use crate::{Context, PropContext, MethodErr, Crossroads, utils::Dbg};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::borrow::Cow;
use dbus::arg;

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

    pub fn prop_names_readable(&self, t: usize) -> impl Iterator<Item=&str> {
        self.0[t].properties.iter().filter_map(|(k, v)| {
            if v.get_cb.is_some() { Some(&**k) } else { None }
        })
    }

    pub fn take_prop(&mut self, t: usize, name: &str, is_set: bool) -> Result<PropCb, MethodErr> {
        let pdesc = self.0[t].properties.get_mut(name).ok_or_else(|| MethodErr::no_property(name))?;
        let cb = if is_set { pdesc.set_cb.take() } else { pdesc.get_cb.take() };
        let rw = if is_set { "writable" } else { "readable" };
        let cb = cb.ok_or_else(|| MethodErr::failed(&format!("Property {} is not {}", name, rw)))?;
        Ok(cb.0)
    }

    pub fn give_prop(&mut self, t: usize, name: &str, cb: PropCb, is_set: bool) {
        let x = self.0[t].properties.get_mut(name).unwrap();
        let cb = Some(Dbg(cb));
        if is_set { x.set_cb = cb } else { x.get_cb = cb };
    }

    pub fn has_props(&self, t: usize) -> bool { !self.0[t].properties.is_empty() }

    pub fn find_annotation(&self, t: usize, annotation_name: &str, prop_name: Option<&str>) -> Option<&str> {
        let desc = &self.0[t];
        if let Some(prop_name) = prop_name {
            if let Some(prop) = desc.properties.get(prop_name) {
                if let Some(value) = prop.annotations.get(annotation_name) {
                    return Some(value);
                }
            }
        }
        desc.annotations.get(annotation_name)
    }

    pub fn introspect(&self, ifaces: &HashSet<usize>) -> String {
        let mut v: Vec<_> = ifaces.iter().filter_map(|&t| self.0[t].name.as_ref().map(|n| (n, t))).collect();
        v.sort_unstable();
        let mut r = String::new();
        for (n, t) in v.into_iter() {
            r += &format!("  <interface name=\"{}\">\n", n);
            let desc = &self.0[t];

            let mut v2: Vec<_> = desc.methods.keys().collect();
            v2.sort_unstable();
            for name in v2.into_iter() {
                r += &format!("    <method name=\"{}\">\n", name);
                let x = &desc.methods[name];
                r += &x.input_args.introspect(Some("in"), "      ");
                r += &x.output_args.introspect(Some("out"), "      ");
                r += &x.annotations.introspect("      ");
                r += "    </method>\n";
            }

            let mut v2: Vec<_> = desc.signals.keys().collect();
            v2.sort_unstable();
            for name in v2.into_iter() {
                r += &format!("    <signal name=\"{}\">\n", name);
                let x = &desc.signals[name];
                r += &x.args.introspect(None, "      ");
                r += &x.annotations.introspect("      ");
                r += "    </signal>\n";
            }

            let mut v2: Vec<_> = desc.properties.keys().collect();
            v2.sort_unstable();
            for name in v2.into_iter() {
                let x = &desc.properties[name];
                let a = match (x.get_cb.is_some(), x.set_cb.is_some()) {
                    (true, true) => "readwrite",
                    (true, false) => "read",
                    (false, true) => "write",
                    _ => unreachable!(),
                };
                r += &format!("    <property name=\"{}\" type=\"{}\" access=\"{}\"", name, x.sig, a);
                if x.annotations.is_empty() {
                    r += "/>\n";
                } else {
                    r += &format!(">\n{}    </property>\n",  x.annotations.introspect("      "));
                }
            }
            desc.annotations.introspect("    ");
            r += "  </interface>\n";
        };
        r
    }

    pub fn get_intf_name(&self, t: usize) -> Option<&dbus::strings::Interface<'static>> {
        self.0.get(t)?.name.as_ref()
    }
}

pub type Callback = Box<dyn FnMut(Context, &mut Crossroads) -> Option<Context> + Send + 'static>;
pub type PropCb = Box<dyn FnMut(PropContext, &mut Crossroads) -> Option<PropContext> + Send + 'static>;

struct CallbackDbg(Callback);

impl fmt::Debug for CallbackDbg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Callback") }
}

#[derive(Debug, Clone, Default)]
pub struct Annotations(Option<HashMap<String, String>>);

impl Annotations {
    pub fn insert<K: Into<String>, V: Into<String>>(&mut self, k: K, v: V) {
        let mut x = self.0.take().unwrap_or_default();
        x.insert(k.into(), v.into());
        self.0 = Some(x);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.as_ref()?.get(key).map(|x| &**x)
    }

    fn is_empty(&self) -> bool {
        self.0.as_ref().map(|s| s.len()).unwrap_or(0) == 0
    }

    fn introspect(&self, prefix: &str) -> String {
        let mut r = String::new();
        if let Some(anns) = &self.0 {
            for (k, v) in anns.iter() {
                r += &format!("{}<annotation name=\"{}\" value=\"{}\"/>\n", prefix, k, v);
            }
        }
        r
    }
}

#[derive(Debug, Clone)]
struct Argument {
    name: Cow<'static, str>,
    sig: dbus::Signature<'static>,
    annotations: Annotations,
}

#[derive(Debug, Clone)]
pub struct Arguments(Vec<Argument>);

impl Arguments {
    fn introspect(&self, dir: Option<&str>, prefix: &str) -> String {
        let mut r = String::new();
        for a in &self.0 {
            r += &format!("{}<arg name=\"{}\" type=\"{}\"", prefix, a.name, a.sig);
            if let Some(dir) = dir { r += &format!(" direction=\"{}\"", dir); }
            if a.annotations.is_empty() {
                r += "/>\n";
            } else {
                let inner_prefix = format!("{}  ", prefix);
                r += &format!(">\n{}{}</arg>\n", a.annotations.introspect(&inner_prefix), prefix);
            }
        }
        r
    }
}

/// Struct used to describe a method when building an interface.
#[derive(Debug)]
pub struct MethodDesc {
    cb: Option<CallbackDbg>,
    input_args: Arguments,
    output_args: Arguments,
    annotations: Annotations,
}

impl MethodDesc {
    pub fn annotate<N: Into<String>, V: Into<String>>(&mut self, name: N, value: V) -> &mut Self {
        self.annotations.insert(name, value);
        self
    }
    pub fn deprecated(&mut self) -> &mut Self { self.annotate(DEPRECATED, "true") }
}


/// Struct used to describe a signal when building an interface.
///
/// For now, this is only used for introspection.
#[derive(Debug)]
pub struct SignalDesc {
    args: Arguments,
    annotations: Annotations,
}

impl SignalDesc {
    pub fn annotate<N: Into<String>, V: Into<String>>(&mut self, name: N, value: V) -> &mut Self {
        self.annotations.insert(name, value);
        self
    }
    pub fn deprecated(&mut self) -> &mut Self { self.annotate(DEPRECATED, "true") }
}

#[derive(Debug)]
pub struct PropDesc {
    annotations: Annotations,
    sig: dbus::Signature<'static>,
    get_cb: Option<Dbg<PropCb>>,
    set_cb: Option<Dbg<PropCb>>,
}

#[derive(Debug)]
pub struct IfaceDesc {
    name: Option<dbus::strings::Interface<'static>>,
    annotations: Annotations,
    methods: HashMap<dbus::strings::Member<'static>, MethodDesc>,
    signals: HashMap<dbus::strings::Member<'static>, SignalDesc>,
    properties: HashMap<String, PropDesc>,
}

fn build_argvec<A: arg::ArgAll>(a: A::strs) -> Arguments {
    let mut v = vec!();
    A::strs_sig(a, |name, sig| {
        v.push(Argument { name: name.into(), sig, annotations: Default::default() })
    });
    Arguments(v)
}

/// Struct used to describe a property when building an interface.
#[derive(Debug)]
pub struct PropBuilder<'a, T:'static, A: 'static>(&'a mut PropDesc, PhantomData<&'static (T, A)>);

impl<T, A> Drop for PropBuilder<'_, T, A> {
    fn drop(&mut self) {
        // Need to set at least one callback!
        assert!(self.0.get_cb.is_some() || self.0.set_cb.is_some());
    }
}

impl<T: Send, A: Send + arg::RefArg + arg::Arg + arg::Append> PropBuilder<'_, T, A> {
    pub fn get<CB>(self, mut cb: CB) -> Self
    where CB: FnMut(&mut PropContext, &mut T) -> Result<A, MethodErr> + Send + 'static {
        self.get_with_cr(move |ctx, cr| {
            let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            cb(ctx, data)
        })
    }

    pub fn get_with_cr<CB>(self, mut cb: CB) -> Self
    where CB: FnMut(&mut PropContext, &mut Crossroads) -> Result<A, MethodErr> + Send + 'static {
        self.get_custom(move |mut ctx, cr| {
            let r = cb(&mut ctx, cr);
            ctx.reply(r);
            Some(ctx)
        })
    }

    pub (crate) fn get_custom<CB>(mut self, mut cb: CB) -> Self
    where CB: FnMut(PropContext, &mut Crossroads) -> Option<PropContext> + Send + 'static {
        self.0.get_cb = Some(Dbg(Box::new(move |ctx, cr| {
            cb(ctx, cr)
        })));
        self
    }

    pub fn get_with_cr_async<R, CB>(self, mut cb: CB) -> Self
    where
        CB: FnMut(PropContext, &mut Crossroads) -> R + Send + 'static,
        R: Future<Output=PhantomData<(A, ())>> + Send + 'static
    {
        self.get_custom(move |mut ctx, cr| {
            cr.run_async_method(|sender, cr| {
                ctx.set_send_on_drop(sender);
                let r = cb(ctx, cr);
                async move { r.await; }
            });
            None
        })
    }

    pub fn get_async<R, CB>(self, mut cb: CB) -> Self
    where
        CB: FnMut(PropContext, &mut T) -> R + Send + 'static,
        R: Future<Output=PhantomData<(A, ())>> + Send + 'static
    {
        self.get_with_cr_async(move |ctx, cr| {
            // It should be safe to unwrap here, the path has already been checked once (when dispatching the method)
            let data = cr.data_mut(ctx.path()).unwrap();
            cb(ctx, data)
        })
    }
}

pub const EMITS_CHANGED: &'static str = "org.freedesktop.DBus.Property.EmitsChangedSignal";
const DEPRECATED: &'static str = "org.freedesktop.DBus.Deprecated";

impl<T: Send, A: arg::RefArg + Send + for<'x> arg::Get<'x> + arg::Arg + arg::Append> PropBuilder<'_, T, A> {
    /// Adds a set property handler to this property.
    ///
    /// In case an EmitsChangedSignal should be emitted, the callback should return Ok(Some(v)) where
    /// v is the value to be emitted. If no EmitsChangedSignal should be emitted, return Ok(None).
    pub fn set<CB>(self, mut cb: CB) -> Self
    where CB: FnMut(&mut PropContext, &mut T, A) -> Result<Option<A>, MethodErr> + Send + 'static {
        self.set_with_cr(move |ctx, cr, a| {
            let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            cb(ctx, data, a)
        })
    }

    /// Adds a set property handler to this property, and allowing the entire tree to be changed.
    ///
    /// In case an EmitsChangedSignal should be emitted, the callback should return Ok(Some(v)) where
    /// v is the value to be emitted. If no EmitsChangedSignal should be emitted, return Ok(None).
    pub fn set_with_cr<CB>(self, mut cb: CB) -> Self
    where CB: FnMut(&mut PropContext, &mut Crossroads, A) -> Result<Option<A>, MethodErr> + Send + 'static {
        self.set_custom(move |mut ctx, cr, a| {
            match cb(&mut ctx, cr, a) {
                Ok(None) => { ctx.reply_noemit(Ok(())); }
                Ok(Some(x)) => { ctx.reply(Ok(x)); }
                Err(x) => { ctx.reply_noemit(Err(x)); }
            };
            Some(ctx)
        })
    }

    pub fn set_custom<CB>(mut self, mut cb: CB) -> Self
    where CB: FnMut(PropContext, &mut Crossroads, A) -> Option<PropContext> + Send + 'static {
        self.0.set_cb = Some(Dbg(Box::new(move |mut ctx, cr| {
            match ctx.check(|ctx| {
                let ctx = ctx.unwrap();
                let mut i = ctx.message().iter_init();
                i.next(); i.next();
                let a: arg::Variant<_> = i.read()?;
                Ok(a)
            }) {
                Ok(a) => cb(ctx, cr, a.0),
                Err(_) => Some(ctx),
            }
        })));
        self
    }

    pub fn set_with_cr_async<CB, R>(self, mut cb: CB) -> Self
    where
        CB: FnMut(PropContext, &mut Crossroads, A) -> R + Send + 'static,
        R: Future<Output=PhantomData<Option<A>>> + Send + 'static
    {
        self.set_custom(move |mut ctx, cr, a| {
            cr.run_async_method(|sender, cr| {
                ctx.set_send_on_drop(sender);
                let r = cb(ctx, cr, a);
                async move { r.await; }
            });
            None
        })
    }

    pub fn set_async<CB, R>(self, mut cb: CB) -> Self
    where
        CB: FnMut(PropContext, &mut T, A) -> R + Send + 'static,
        R: Future<Output=PhantomData<Option<A>>> + Send + 'static
    {
        self.set_with_cr_async(move |ctx, cr, a| {
            // It should be safe to unwrap here, the path has already been checked once (when dispatching the method)
            let data = cr.data_mut(ctx.path()).unwrap();
            cb(ctx, data, a)
        })
    }
}

impl<T: std::marker::Send, A> PropBuilder<'_, T, A> {

    pub fn annotate<N: Into<String>, V: Into<String>>(self, name: N, value: V) -> Self {
        self.0.annotations.insert(name, value);
        self
    }
    pub fn deprecated(self) -> Self { self.annotate(DEPRECATED, "true") }
    pub fn emits_changed_false(self) -> Self { self.annotate(EMITS_CHANGED, "false") }
    pub fn emits_changed_const(self) -> Self { self.annotate(EMITS_CHANGED, "const") }
    pub fn emits_changed_invalidates(self) -> Self { self.annotate(EMITS_CHANGED, "invalidates") }
    pub fn emits_changed_true(self) -> Self { self.annotate(EMITS_CHANGED, "true") }
}

/// Struct used to build an interface.
///
/// You get an instance of this struct in the call to Crossroads::register.
///
/// Register new methods, properties and signals using the corresponding functions on this struct.
/// You might find several similar functions, e g `method`, `method_with_cr`, `method_with_cr_async` and
/// `method_with_cr_custom`. Methods that have "with_cr" will allow you to access the full mutable Crossroads
/// instance, but beware - trying to recursively handle methods from within a method handler is not allowed
/// and may cause panics.
///
/// Methods that have "_async" will allow you to defer the result of your method. During await points,
/// other tasks with method calls can run as separate tasks. Remember to call Crossroads::set_async_support
/// when using async methods.
///
#[derive(Debug)]
pub struct IfaceBuilder<T: Send + 'static>(IfaceDesc, PhantomData<&'static T>);

impl<T: Send + 'static> IfaceBuilder<T> {
    pub fn property<A: arg::Arg, N: Into<String>>(&mut self, name: N) -> PropBuilder<T, A> {
        PropBuilder(self.0.properties.entry(name.into()).or_insert(PropDesc {
            annotations: Default::default(),
            get_cb: None,
            set_cb: None,
            sig: A::signature(),
        }), PhantomData)
    }

    pub fn method<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: arg::ArgAll + arg::ReadAll, OA: arg::ArgAll + arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&mut Context, &mut T, IA) -> Result<OA, MethodErr> + Send + 'static {
        self.method_with_cr(name, input_args, output_args, move |ctx, cr, ia| {
            let data = cr.data_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            cb(ctx, data, ia)
        })
    }

    pub fn method_with_cr<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: arg::ArgAll + arg::ReadAll, OA: arg::ArgAll + arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&mut Context, &mut Crossroads, IA) -> Result<OA, MethodErr> + Send + 'static {
        let boxed = Box::new(move |mut ctx: Context, cr: &mut Crossroads| {
            let _ = ctx.check(|ctx| {
                let ia = ctx.message().read_all()?;
                let oa = cb(ctx, cr, ia)?;
                ctx.do_reply(|msg| msg.append_all(oa));
                Ok(())
            });
            Some(ctx)
        });
        self.0.methods.entry(name.into()).or_insert(MethodDesc {
            annotations: Default::default(),
            input_args: build_argvec::<IA>(input_args),
            output_args: build_argvec::<OA>(output_args),
            cb: Some(CallbackDbg(boxed)),
        })
    }

    pub fn method_with_cr_custom<IA, OA, N, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: arg::ArgAll + arg::ReadAll, OA: arg::ArgAll + arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(Context, &mut Crossroads, IA) -> Option<Context> + Send + 'static {
        let boxed = Box::new(move |mut ctx: Context, cr: &mut Crossroads| {
            match ctx.check(|ctx| Ok(ctx.message().read_all()?)) {
                Ok(ia) => cb(ctx, cr, ia),
                Err(_) => Some(ctx),
            }
        });
        self.0.methods.entry(name.into()).or_insert(MethodDesc {
            annotations: Default::default(),
            input_args: build_argvec::<IA>(input_args),
            output_args: build_argvec::<OA>(output_args),
            cb: Some(CallbackDbg(boxed)),
        })
    }

    pub fn method_with_cr_async<IA, OA, N, R, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: arg::ArgAll + arg::ReadAll, OA: arg::ArgAll + arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(Context, &mut Crossroads, IA) -> R + Send + 'static,
    R: Future<Output=PhantomData<OA>> + Send + 'static {
        self.method_with_cr_custom::<IA, OA, _, _>(name, input_args, output_args, move |mut ctx, cr, ia| {
            cr.run_async_method(|sender, cr| {
                ctx.set_send_on_drop(sender);
                let r = cb(ctx, cr, ia);
                async move { r.await; }
            });
            None
        })
    }


/*
    pub fn method_with_cr_async<'x, IA, OA, N, R, CB>(&mut self, name: N, input_args: IA::strs, output_args: OA::strs, mut cb: CB) -> &mut MethodDesc
    where IA: arg::ArgAll + arg::ReadAll, OA: arg::ArgAll + arg::AppendAll,
    N: Into<dbus::strings::Member<'static>>,
    CB: FnMut(&'x mut Context, &mut Crossroads, IA) -> R + Send + 'static,
    R: Future<Output = Result<OA, MethodErr>> + Send + 'x {
        self.method_with_cr_custom::<IA, OA, _, _>(name, input_args, output_args, move |mut ctx, cr, ia| {
            cb(&mut ctx, cr, ia);
            todo!()
        })
    }
*/

    pub fn signal<A, N>(&mut self, name: N, args: A::strs) -> &mut SignalDesc
    where A: arg::ArgAll, N: Into<dbus::strings::Member<'static>> {
        self.0.signals.entry(name.into()).or_insert(SignalDesc {
            annotations: Default::default(),
            args: build_argvec::<A>(args),
        })
    }

    pub fn annotate<N: Into<String>, V: Into<String>>(mut self, name: N, value: V) -> Self {
        self.0.annotations.insert(name, value);
        self
    }
    pub fn deprecated(self) -> Self { self.annotate(DEPRECATED, "true") }

    pub (crate) fn build<F>(name: Option<dbus::strings::Interface<'static>>, f: F) -> IfaceDesc
    where F: FnOnce(&mut IfaceBuilder<T>) {
        let mut b = IfaceBuilder(IfaceDesc {
            name,
            annotations: Default::default(),
            methods: Default::default(),
            signals: Default::default(),
            properties: Default::default(),
        }, PhantomData);
        f(&mut b);
        b.0
    }
}
