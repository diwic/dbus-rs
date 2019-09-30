use super::crossroads::Crossroads;
use super::handlers::{Par, Mut, Handlers, MakeHandler};
use super::info::{IfaceInfo, MethodInfo, PropInfo, Annotations, Argument, Access};
use crate::{arg, Message, Path as PathName};
use super::MethodErr;
use crate::arg::{Arg, Variant, Append};
use std::collections::HashMap;
use super::path::Path;
use super::context::{MsgCtx, RefCtx};
use std::ffi::CStr;

pub struct DBusProperties;

fn setprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, msg: &Message, f: F) -> Result<Message, MethodErr> 
where F: FnOnce(&mut H::SetProp, &mut Path<H>, &mut arg::Iter, &Message) -> Result<bool, MethodErr>
{
    let mut iter = msg.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = msg.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
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
    if f(handler, pathdata, &mut subiter, msg)? {
        unimplemented!("Emits signal here");
    }
    Ok(msg.method_return())
}

fn setprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr> 
where F: FnOnce(&H::SetProp, &mut arg::Iter, &mut MsgCtx, &RefCtx<H>) -> Result<bool, MethodErr>
{
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &CStr) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| x.name.as_cstr() == propname)
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


fn getprop_mut<H: Handlers, F>(cr: &mut Crossroads<H>, msg: &Message, f: F) -> Result<Message, MethodErr>
where F: FnOnce(&mut H::GetProp, &mut arg::IterAppend, &Message) -> Result<(), MethodErr>
{
    let mut iter = msg.iter_init();
    let (iname, propname) = (iter.read()?, iter.read()?);
    let path = msg.path().ok_or_else(|| { MethodErr::no_property(&"Message has no path") })?;
    let (propinfo, pathdata) = cr.prop_lookup_mut(path.as_cstr(), iname, propname)
        .ok_or_else(|| { MethodErr::no_property(&"Property not found") })?;
    if propinfo.access == Access::Write { Err(MethodErr::no_property(&"Property is write only"))? };
    let handler = propinfo.handlers.0.as_mut()
        .ok_or_else(|| { MethodErr::no_property(&"Property can not read from") })?;

    let mut mret = msg.method_return();
    {
        let mut iter = arg::IterAppend::new(&mut mret);
        let mut z = None;
        iter.append_variant(&propinfo.sig, |subi| {
            z = Some(f(handler, subi, msg));
        });
        z.unwrap()?;
    }
    Ok(mret)
}

fn getprop_ref<H: Handlers, F>(ctx: &mut MsgCtx, refctx: &RefCtx<H>, f: F) -> Result<Message, MethodErr> 
where F: FnOnce(&H::GetProp, &mut arg::IterAppend, &mut MsgCtx, &RefCtx<H>) -> Result<(), MethodErr> {
    let mut iter = ctx.message.iter_init();
    let (iname, propname): (&CStr, &CStr) = (iter.read()?, iter.read()?);
    let refctx = refctx.with_iface(iname)
        .ok_or_else(|| { MethodErr::no_property(&"Interface not found") })?;
    let propinfo = refctx.iinfo.props.iter().find(|x| x.name.as_cstr() == propname)
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

    let mut ret = Ok(());
    let mut mret = ctx.message.method_return();
    {
        let mut iter1 = arg::IterAppend::new(&mut mret);
        iter1.append_dict(&String::signature(), &Variant::<u8>::signature(), |iter2| {
            for propinfo in refctx.iinfo.props.iter() {
                let mut z = None;
                if propinfo.access == Access::Write { continue; }
                if let Some(handler) = propinfo.handlers.0.as_ref() {
                    iter2.append_dict_entry(|mut iter3| {
                        (&*propinfo.name).append_by_ref(&mut iter3);
                        iter3.append_variant(&propinfo.sig, |iter4| {
                            z = Some(f(handler, iter4, ctx, &refctx));
                        });
                    });
                }
                if let Err(e) = z.unwrap() { ret = Err(e); return; }
            }
        });
    }
    ret.map(|_| { mret })

}


impl DBusProperties {
    fn register<H: Handlers>(cr: &mut Crossroads<H>, get: H::Method, getall: H::Method, set: H::Method) {
        cr.register::<Self,_>("org.freedesktop.DBus.Properties")
            .method_custom::<(String, String), (Variant<u8>,)>("Get".into(), ("interface_name", "property_name"), ("value",), get)
            .method_custom::<(String,), (HashMap<String, Variant<u8>>,)>("GetAll".into(), ("interface_name",), ("props",), getall)
            .method_custom::<(String, String, Variant<u8>), ()>("Set".into(), ("interface_name", "property_name", "value"), (), set);
    }

    pub fn register_par(cr: &mut Crossroads<Par>) {
        Self::register(cr,
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

    pub fn register_mut(cr: &mut Crossroads<Mut>) {
//        Self::register(cr, unimplemented!(), unimplemented!(), unimplemented!());
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
    use std::collections::Bound;
    let name = path.name();
    let mut p = Vec::<u8>::from(name.as_bytes());
    p.push(b'/');
    let mut children = cr.paths.range::<CStr,_>((Bound::Excluded(name.as_cstr()), Bound::Unbounded));
    let mut childstr = String::new();
    while let Some((c, _)) = children.next() {
        if !c.as_bytes().starts_with(&p) { break; }
        let csub: &str = &c.to_str().unwrap()[p.len()..];
        childstr = format!("{}  <node name=\"{}\"/>\n", childstr, csub);
    }

    let mut ifacestr = String::new();
    for (_, (typeid, info)) in &cr.reg {
        if path.get_from_typeid(*typeid).is_some() {
            ifacestr += &introspect_iface(info);
        }
    }

    let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, name, ifacestr, childstr);
    nodestr
}

impl DBusIntrospectable {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) {
        cr.register::<Self,_>("org.freedesktop.DBus.Introspectable")
            .method("Introspect", (), ("xml_data",), |_: &mut MsgCtx, c: &RefCtx<H>, _: ()| {
                Ok((introspect(c.crossroads, c.path),))
            });
    }
}



