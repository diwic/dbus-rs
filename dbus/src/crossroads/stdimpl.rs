use super::crossroads::{Crossroads, PathData};
use super::handlers::{ParInfo, Par, Handlers, MakeHandler};
use super::info::{IfaceInfo, MethodInfo, PropInfo, Annotations};
use crate::{arg, Message, Path as PathName};
use super::MethodErr;

pub struct DBusProperties;

impl DBusProperties {
    pub fn register(cr: &mut Crossroads<Par>) {
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
    for (iname, (typeid, info)) in &cr.reg {
        if data.contains_key(*typeid) {
            ifacestr = format!("{}  <interface name=\"{}\">\n{}  </interface>\n", ifacestr,
              iname.to_str().unwrap(), introspect_anns(&info.anns, "    "));
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



