use super::crossroads::{Crossroads, PathData};
use super::handlers::{ParInfo, Par, Handlers, MakeHandler};
use super::info::{IfaceInfo, MethodInfo, PropInfo, Annotations, Argument, Access};
use crate::{arg, Message, Path as PathName};
use super::MethodErr;
use crate::arg::Variant;
use std::collections::HashMap;

pub struct DBusProperties;

pub fn set_mut<H: Handlers>(cr: &mut Crossroads<H>, msg: &Message) -> Result<Message, MethodErr> {
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
    if H::call_setprop_mut(handler, pathdata, &mut subiter, msg)? {
        unimplemented!("Emits signal here");
    }
    Ok(msg.method_return())
}

impl DBusProperties {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) {
        cr.register::<Self,_>("org.freedesktop.DBus.Properties")
            .method_custom::<(String, String), (Variant<u8>,)>("Get".into(), ("interface_name", "property_name"), ("value",),
                H::custom_method_helper(None))
            .method_custom::<(String,), (HashMap<String, Variant<u8>>,)>("GetAll".into(), ("interface_name",), ("props",),
                H::custom_method_helper(None))
            .method_custom::<(String, String, Variant<u8>), ()>("Set".into(), ("interface_name", "property_name", "value"), (), 
                H::custom_method_helper(Some(set_mut)));
    }

    pub fn register_par(cr: &mut Crossroads<Par>) {
        cr.register_custom::<Self>(IfaceInfo::new("org.freedesktop.DBus.Properties",
            vec!(MethodInfo::new_par("Get", |_: &DBusProperties, info| {
                let (iname, propname) = info.msg().read2()?; 
                let (lookup, pinfo) = info.crossroads().reg_prop_lookup(info.path_data(), iname, propname)
                    .ok_or_else(|| { MethodErr::no_property(&"Could not find property") })?;
                let handler = &pinfo.handlers.0.as_ref()
                    .ok_or_else(|| { MethodErr::no_property(&"Property can not be read") })?;
                let iface = &**lookup.iface;
                let mut pinfo = ParInfo::new(info.msg(), lookup);
                let mut mret = info.msg().method_return();
                {
                    let mut ia = arg::IterAppend::new(&mut mret);
                    (handler)(iface, &mut ia, &mut pinfo)?;
                }
                Ok(Some(mret))
            })),
            vec!(), vec!()
        ));
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

fn introspect<H: Handlers>(cr: &Crossroads<H>, data: &PathData<H>, path: PathName) -> String {
    use std::ffi::{CStr, CString};
    use std::collections::Bound;
    let mut p = Vec::<u8>::from(&*path);
    p.push(b'/');
    let mut children = cr.paths.range::<CStr,_>((Bound::Excluded(path.as_cstr()), Bound::Unbounded));
    let mut childstr = String::new();
    while let Some((c, _)) = children.next() {
        if !c.as_bytes().starts_with(&p) { break; }
        let csub: &str = &c.to_str().unwrap()[p.len()..];
        childstr = format!("{}  <node name=\"{}\"/>\n", childstr, csub);
    }

    let mut ifacestr = String::new();
    for (_, (typeid, info)) in &cr.reg {
        if data.contains_key(*typeid) {
            ifacestr += &introspect_iface(info);
        }
    }

    let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, path, ifacestr, childstr);
    nodestr
}

impl DBusIntrospectable {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) {
        cr.register::<Self,_>("org.freedesktop.DBus.Introspectable")
            .method("Introspect", (), ("xml_data",), |cr: &Crossroads<H>, data: &PathData<H>, msg: &Message, _: ()| {
                Ok((introspect(cr, data, msg.path().unwrap()),))
            });
    }
}



