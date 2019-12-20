use super::crossroads::Crossroads;
use super::handlers::{self, Par, Handlers, MakeHandler, SendMethod, LocalMethod};
use super::info::{IfaceInfo, MethodInfo, PropInfo, Annotations, Argument, Access, EmitsChangedSignal};
use crate::{arg, Message};
use super::MethodErr;
use crate::arg::{Arg, Variant, Append, IterAppend};
use std::collections::{HashMap, HashSet, Bound};
use super::path::{Path, PathData};
use super::context::{MsgCtx, RefCtx};
use std::ffi::{CStr, CString};
use crate::strings::{Member, Signature, Interface as IfaceName, Path as PathName};

pub struct DBusProperties;

type PathIntfProps = HashMap<PathName<'static>, IntfProps>;
type IntfProps = HashMap<String, Props>;
type Props = HashMap<String, Variant<u8>>;

fn append_prop<F>(iter: &mut IterAppend, name: &str, sig: &Signature, f: F) -> Result<(), MethodErr>
where F: FnOnce(&mut IterAppend) -> Result<(), MethodErr> {
    let mut z = None;
    iter.append_dict_entry(|mut iter3| {
        name.append_by_ref(&mut iter3);
        iter3.append_variant(&sig, |iter4| {
            z = Some(f(iter4));
        });
    });
    z.unwrap()
}

fn append_props_ref<H: Handlers, F>(iter: &mut IterAppend, iinfo: &IfaceInfo<H>, mut f: F) -> Result<(), MethodErr>
where F: FnMut(&mut IterAppend, &H::GetProp) -> Result<(), MethodErr> {
    let mut ret = Ok(());
    iter.append_dict(&String::signature(), &Variant::<u8>::signature(), |iter2| {
        for propinfo in iinfo.props.iter() {
            if propinfo.access == Access::Write { continue; }
            if let Some(handler) = propinfo.handlers.0.as_ref() {
                if let Err(e) = append_prop(iter2, &propinfo.name, &propinfo.sig, |ia| { f(ia, handler) }) {
                    ret = Err(e);
                    return;
                }
            }
        }
    });
    ret
}

fn append_props_mut<H: Handlers, F>(iter: &mut IterAppend, iinfo: &mut IfaceInfo<H>, mut f: F) -> Result<(), MethodErr>
where F: FnMut(&mut IterAppend, &mut H::GetProp) -> Result<(), MethodErr> {
    let mut ret = Ok(());
    iter.append_dict(&String::signature(), &Variant::<u8>::signature(), |iter2| {
        for propinfo in iinfo.props.iter_mut() {
            if propinfo.access == Access::Write { continue; }
            if let Some(handler) = propinfo.handlers.0.as_mut() {
                if let Err(e) = append_prop(iter2, &propinfo.name, &propinfo.sig, |ia| { f(ia, handler) }) {
                    ret = Err(e);
                    return;
                }
            }
        }
    });
    ret
}

fn setprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&mut H::SetProp, &mut Path<H>, &mut arg::Iter, &mut MsgCtx) -> Result<Option<Box<dyn arg::RefArg>>, MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata, emits) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;
    if propinfo.access == Access::Read { Err(MethodErr::no_property(&"Property is read only"))? };
    let handler = propinfo.handlers.1.as_mut()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not written to") })?;

    // Now descend into the variant.
    use arg::Arg;
    let mut subiter = iter.recurse(Variant::<bool>::ARG_TYPE).ok_or_else(|| MethodErr::invalid_arg(&2))?;
    if *subiter.signature() != *propinfo.sig {
        Err(MethodErr::failed(&format!("Property {} cannot change type", propinfo.name)))?;
    }
    if let Some(r) = f(handler, pathdata, &mut subiter, ctx)? {
        match emits {
            EmitsChangedSignal::True => ctx.dbus_signals_mut().add_changed_property(
                path.into_static(), IfaceName::from(iname).into_static(), propname.into(), r
            ),
            EmitsChangedSignal::False => {},
            EmitsChangedSignal::Invalidates => ctx.dbus_signals_mut().add_invalidated_property(
                path.into_static(), IfaceName::from(iname).into_static(), propname.into()
            ),
            EmitsChangedSignal::Const => {}, // Panic here because the property cannot change?
        }
    }
    Ok(ctx.message.method_return())
}

fn setprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&H::SetProp, &mut arg::Iter, &mut MsgCtx, &RefCtx<H>) -> Result<bool, MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &str) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| &x.name == propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;

    if propinfo.access == Access::Read { Err(MethodErr::no_property(&"Property is read only"))? };
    let handler = propinfo.handlers.1.as_ref()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not written to") })?;

    // Now descend into the variant.
    use arg::Arg;
    let mut subiter = iter.recurse(Variant::<bool>::ARG_TYPE).ok_or_else(|| MethodErr::invalid_arg(&2))?;
    if *subiter.signature() != *propinfo.sig {
        Err(MethodErr::failed(&format!("Property {} cannot change type", propinfo.name)))?;
    }
    if f(handler, &mut subiter, ctx, &refctx)? {
        unimplemented!("Emits signal here");
    }
    Ok(ctx.message.method_return())
}


fn getprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&mut H::GetProp, &mut Path<H>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata, _) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;
    if propinfo.access == Access::Write { Err(MethodErr::no_property(&"Property is write only"))? };
    let handler = propinfo.handlers.0.as_mut()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not be read from") })?;

    let mut mret = ctx.message.method_return();
    {
        let mut iter = arg::IterAppend::new(&mut mret);
        let mut z = None;
        iter.append_variant(&propinfo.sig, |subi| {
            z = Some(f(handler, pathdata, subi, ctx));
        });
        z.unwrap()?;
    }
    Ok(mret)
}

fn getprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&H::GetProp, &mut arg::IterAppend, &mut MsgCtx, &RefCtx<H>) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &str) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| &*x.name == propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;

    if propinfo.access == Access::Write { Err(MethodErr::no_property(&"Property is write only"))? };
    let handler = propinfo.handlers.0.as_ref()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not read from") })?;

    let mut mret = ctx.message.method_return();
    {
        let mut iter = arg::IterAppend::new(&mut mret);
        let mut z = None;
        iter.append_variant(&propinfo.sig, |subi| {
            z = Some(f(handler, subi, ctx, &refctx));
        });
        z.unwrap()?;
    }
    Ok(mret)
}

fn getallprops_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, mut f: F) -> Result<Message, MethodErr>
where F: FnMut(&H::GetProp, &mut arg::IterAppend, &mut MsgCtx, &RefCtx<H>) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let iname: &CStr = iter.read()?;
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;

    let mut mret = ctx.message.method_return();
    {
        append_props_ref(&mut arg::IterAppend::new(&mut mret), &refctx.iinfo, |iter4, handler| {
            f(handler, iter4, ctx, &refctx)
        })?;
    }
    Ok(mret)
}

fn getallprops_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, mut f: F) -> Result<Message, MethodErr>
where F: FnMut(&mut H::GetProp, &mut Path<H>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let iname: &CStr = iter.read()?;

    let path = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let pdata = cr.paths.get_mut(path.as_cstr())
        .ok_or_else(|| { MethodErr::no_property(&"Path not found") })?;
    let entry = cr.reg.get_mut(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let _ = pdata.get_from_typeid(entry.typeid)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;

    let mut mret = ctx.message.method_return();
    {
        append_props_mut(&mut arg::IterAppend::new(&mut mret), &mut entry.info, |iter4, handler| {
            f(handler, pdata, iter4, ctx)
        })?;
    }
    Ok(mret)
}

fn objmgr_mut<H: Handlers, F>(cr: &mut Crossroads<H>, ctx: &mut MsgCtx, mut f: F) -> Result<Message, MethodErr>
where F: FnMut(&mut H::GetProp, &mut Path<H>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr>
{
    let pathname = ctx.message.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let mut p = Vec::<u8>::from(pathname.as_bytes());
    if !p.ends_with(b"/") { p.push(b'/'); }

    let mut children = cr.paths.range_mut::<CStr,_>((Bound::Included(pathname.as_cstr()), Bound::Unbounded));
    let cr_reg = &mut cr.reg;

    let mut ret = Ok(());
    let mut mret = ctx.message.method_return();
    {
        let mut ia = arg::IterAppend::new(&mut mret);
        ia.append_dict(&PathName::signature(), &IntfProps::signature(), |ia2| {
            while let Some((c, pdata)) = children.next() {
                if !c.as_bytes().starts_with(&p) && pathname.as_bytes() != c.as_bytes() { break; }
                ia2.append_dict_entry(|mut ia3| {
                    pdata.name().append_by_ref(&mut ia3);
                    ia3.append_dict(&String::signature(), &Props::signature(), |ia4| {
                        for entry in cr_reg.values_mut() {
                            if pdata.get_from_typeid(entry.typeid).is_none() { continue };
                            ia4.append_dict_entry(|mut ia5| {
                                entry.info.name.append_by_ref(&mut ia5);
                                if let Err(e) = append_props_mut(&mut ia5, &mut entry.info, |ia6, handler| {
                                    f(handler, pdata, ia6, ctx)
                                }) { ret = Err(e); return };
                            });
                            if ret.is_err() { return; }
                        }
                    });
                    if ret.is_err() { return; }
                });
                if ret.is_err() { return; }
            }
        });
    }
    ret.map(|_| mret)
}

impl DBusProperties {
    fn register_custom<H: Handlers>(cr: &mut Crossroads<H>, get: H::Method, getall: H::Method, set: H::Method) where Self: PathData<H::Iface> {
        cr.register::<Self,_>("org.freedesktop.DBus.Properties")
            .method_custom::<(String, String), (Variant<u8>,)>("Get".into(), ("interface_name", "property_name"), ("value",), get)
            .method_custom::<(String,), (Props,)>("GetAll".into(), ("interface_name",), ("props",), getall)
            .method_custom::<(String, String, Variant<u8>), ()>("Set".into(), ("interface_name", "property_name", "value"), (), set)
            .signal::<(String, Props, Vec<String>), _>("PropertiesChanged",
                ("interface_name", "changed_properties", "invalidated_properties"))
            .on_path_insert(|p, cr| {
                if cr.reg.values().any(|entry| !entry.info.props.is_empty() && p.get_from_typeid(entry.typeid).is_some()) {
                    p.insert(DBusProperties)
                }
            });
    }

    pub fn register_par(cr: &mut Crossroads<Par>) {
        Self::register_custom(cr,
            Box::new(|ctx, refctx| {
                Some(getprop_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            }),
            Box::new(|ctx, refctx| {
                Some(getallprops_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            }),
            Box::new(|ctx, refctx| {
                Some(setprop_ref(ctx, refctx, |h, i, ctx, refctx| h(i, ctx, refctx)).unwrap_or_else(|e| e.to_message(ctx.message)))
            })
        );
    }


    pub fn register_local(cr: &mut Crossroads<handlers::Local>) {
        let getprop = |cr: &mut Crossroads<handlers::Local>, ctx: &mut MsgCtx| {
            getprop_mut(cr, ctx, |f, path, ia, ctx| { f(path, ia, ctx) })
        };
        let getallprop = |cr: &mut Crossroads<handlers::Local>, ctx: &mut MsgCtx| {
            getallprops_mut(cr, ctx, |f, path, ia, ctx| { f(path, ia, ctx) })
        };
        let setprop = |cr: &mut Crossroads<handlers::Local>, ctx: &mut MsgCtx| {
            setprop_mut(cr, ctx, |f, path, iter, ctx| { f(path, iter, ctx) })
        };
        Self::register_custom(cr,
            MakeHandler::make(getprop), MakeHandler::make(getallprop), MakeHandler::make(setprop)
        );
    }

    pub fn register(cr: &mut Crossroads<()>) {
        let getprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            getprop_mut(cr, ctx, |f, path, ia, ctx| { f(path, ia, ctx) })
        };
        let getallprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            getallprops_mut(cr, ctx, |f, path, ia, ctx| { f(path, ia, ctx) })
        };
        let setprop = |cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            setprop_mut(cr, ctx, |f, path, iter, ctx| { f(path, iter, ctx) })
        };
        Self::register_custom(cr,
            MakeHandler::make(getprop), MakeHandler::make(getallprop), MakeHandler::make(setprop)
        );
    }

}

pub struct DBusObjectManager;

impl DBusObjectManager {
    fn register_custom<H: Handlers>(cr: &mut Crossroads<H>, m: H::Method) where Self: PathData<H::Iface> {
        cr.register::<Self,_>("org.freedesktop.DBus.ObjectManager")
            .method_custom::<(), (PathIntfProps,)>("GetManagedObjects".into(), (), ("objpath_interfaces_and_properties",), m)
            .signal::<(PathName, IntfProps), _>("InterfacesAdded",
                ("object_path", "interfaces_and_properties"))
            .signal::<(PathName, Vec<String>), _>("InterfacesRemoved",
                ("object_path", "interfaces"));
    }

    pub fn register(cr: &mut Crossroads<()>) {
        Self::register_custom(cr, MakeHandler::make(|cr: &mut Crossroads<()>, ctx: &mut MsgCtx| {
            objmgr_mut(cr, ctx, |h, path, ia, ctx| h(path, ia, ctx))
        }))
    }

    pub fn register_local(cr: &mut Crossroads<handlers::Local>) {
        Self::register_custom(cr, MakeHandler::make(|cr: &mut Crossroads<handlers::Local>, ctx: &mut MsgCtx| {
            objmgr_mut(cr, ctx, |h, path, ia, ctx| h(path, ia, ctx))
        }))
    }
}


pub struct DBusIntrospectable;

fn introspect_anns(anns: &Annotations, prefix: &str) -> String {
    let mut r = String::new();
    for (k, v) in anns.iter() {
        r += &format!("{}<annotation name=\"{}\" value=\"{}\"/>\n", prefix, k, v);
    }
    r
}

fn introspect_args(args: &[Argument], dir: Option<&str>, prefix: &str) -> String {
    let mut r = String::new();
    for a in args {
        r += &format!("{}<arg name=\"{}\" type=\"{}\"", prefix, a.name, a.sig);
        if let Some(dir) = dir { r += &format!(" direction=\"{}\"", dir); }
        if a.anns.len() == 0 {
            r += "/>\n";
        } else {
            let inner_prefix = format!("{}  ", prefix);
            r += &format!(">\n{}{}</arg>\n", introspect_anns(&a.anns, &inner_prefix), prefix);
        }
    }
    r
}

fn introspect_iface<H: Handlers>(iface: &IfaceInfo<H>) -> String {
    let mut r = format!("  <interface name=\"{}\">\n", iface.name);
    for x in &iface.methods {
        r += &format!("    <method name=\"{}\">\n", x.name);
        r += &introspect_args(&x.i_args, Some("in"), "      ");
        r += &introspect_args(&x.o_args, Some("out"), "      ");
        r += &introspect_anns(&x.anns, "      ");
        r += "    </method>\n";
    }

    for x in &iface.signals {
        r += &format!("    <signal name=\"{}\">\n", x.name);
        r += &introspect_args(&x.args, None, "      ");
        r += &introspect_anns(&x.anns, "      ");
        r += "    </signal>\n";
    }

    for x in &iface.props {
        use super::info::Access;
        let a = match x.access {
            Access::Read => "read",
            Access::Write => "write",
            Access::ReadWrite => "readwrite",
        };
        r += &format!("    <property name=\"{}\" type=\"{}\" access=\"{}\"", x.name, x.sig, a);
        if x.anns.len() == 0 {
            r += "/>\n";
        } else {
            r += &format!(">\n{}    </property>\n", introspect_anns(&x.anns, "      "));
        }
    }

    r += &introspect_anns(&iface.anns, "    ");
    r += "  </interface>\n";
    r
}

fn introspect<H: Handlers>(cr: &Crossroads<H>, path: &Path<H>) -> String {
    use std::ffi::{CStr, CString};
    let name = path.name();
    let mut p = Vec::<u8>::from(name.as_bytes());
    if !p.ends_with(b"/") { p.push(b'/'); }

    let mut children = cr.paths.range::<CStr,_>((Bound::Excluded(name.as_cstr()), Bound::Unbounded));
    let mut childstr = String::new();
    while let Some((c, _)) = children.next() {
        if !c.as_bytes().starts_with(&p) { break; }
        let csub: &str = &c.to_str().unwrap()[p.len()..];
        childstr = format!("{}  <node name=\"{}\"/>\n", childstr, csub);
    }

    let mut ifacestr = String::new();
    for entry in cr.reg.values() {
        if path.get_from_typeid(entry.typeid).is_some() {
            ifacestr += &introspect_iface(&entry.info);
        }
    }

    let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, name, ifacestr, childstr);
    nodestr
}

impl DBusIntrospectable {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) where Self: PathData<H::Iface> {
        cr.register::<Self,_>("org.freedesktop.DBus.Introspectable")
            .method("Introspect", (), ("xml_data",), |_: &mut MsgCtx, c: &RefCtx<H>, _: ()| {
                Ok((introspect(c.crossroads, c.path),))
            })
            .on_path_insert(|p, cr| p.insert(DBusIntrospectable));
    }
}
/*
#[derive(Debug, Default)]
struct PropsPerIntf {
    changed: HashMap<String, Box<dyn arg::RefArg>>,
    invalidated: HashSet<String>,
}
*/

use crate::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PPC;

#[derive(Debug, Default)]
struct SignalsPerPath {
    properties: HashMap<IfaceName<'static>, PPC>,
    interfaces_removed: HashSet<CString>,
    interfaces_added: HashMap<CString, HashMap<CString, Box<dyn arg::RefArg>>>,
}

#[derive(Debug, Default)]
pub struct DBusSignals(HashMap<PathName<'static>, SignalsPerPath>);

impl DBusSignals {
    pub fn new() -> Self { Default::default() }

    pub fn add_changed_property(&mut self, path: PathName<'static>, iface: IfaceName<'static>, propname: String, value: Box<dyn arg::RefArg>) {
        let i2 = iface.to_string();
        self.0.entry(path).or_default()
            .properties.entry(iface).or_insert_with(|| {
                PPC { interface_name: i2, changed_properties: Default::default(), invalidated_properties: Default::default() }
            }).changed_properties.insert(propname, Variant(value));
    }

    pub fn add_invalidated_property(&mut self, path: PathName<'static>, iface: IfaceName<'static>, propname: String) {
        let i2 = iface.to_string();
        let inv = &mut self.0.entry(path).or_default()
            .properties.entry(iface).or_insert_with(|| {
                PPC { interface_name: i2, changed_properties: Default::default(), invalidated_properties: Default::default() }
            }).invalidated_properties;
        if !inv.iter().any(|x| x == &propname) { inv.push(propname) }
    }

    pub fn into_messages(self) -> Vec<Message> {
        use crate::message::SignalArgs;
        let mut result = vec!();
        for (pathname, sigs) in self.0 {
            for (_, props) in sigs.properties {
                result.push(props.to_emit_message(&pathname));
            }
        }
        result
    }
}
