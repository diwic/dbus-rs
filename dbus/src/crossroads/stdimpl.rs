use super::crossroads::{Crossroads, PathData};
use super::handlers::{ParInfo, Par, Handlers, MakeHandler};
use super::info::{IfaceInfo, MethodInfo, PropInfo};
use crate::{arg, Message};
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

fn introspect<H: Handlers>(_cr: &Crossroads<H>, _path: &PathData<H>) -> String {
    unimplemented!()
}

impl DBusIntrospectable {
    pub fn register<H: Handlers>(cr: &mut Crossroads<H>) {
        cr.register::<Self,_>("org.freedesktop.DBus.Introspectable")
            .method("Introspect", (), ("xml_data",), |cr: &Crossroads<H>, path: &PathData<H>, _: &Message, _: ()| {
                Ok((introspect(cr, path),))
               // let path = msg.path().unwrap();
               // let path = cr.get(path);
               // Ok(introspect(cr, path))
            });
    }
}



