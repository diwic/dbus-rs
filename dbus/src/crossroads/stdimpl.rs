use super::crossroads::Crossroads;
use super::handlers::{ParInfo, Par};
use super::info::{IfaceInfo, MethodInfo, PropInfo};
use crate::arg;
use super::MethodErr;

pub struct DBusProperties;

impl DBusProperties {
    pub fn register(cr: &mut Crossroads<Par>) {
        cr.register::<Self>(IfaceInfo::new("org.freedesktop.DBus.Properties",
            vec!(MethodInfo::new_par("Get", |_: &DBusProperties, info| {
                let (iname, propname) = info.msg().read2()?; 
                let (lookup, pinfo) = info.crossroads().reg_prop_lookup(info.path_data(), iname, propname)
                    .ok_or_else(|| { MethodErr::no_property(&"Could not find property") })?;
                let handler = &pinfo.handlers.0.as_ref()
                    .ok_or_else(|| { MethodErr::no_property(&"Property can not be read") })?;
                let iface = &**lookup.iface;
                let mut pinfo = ParInfo::new(info.msg(), lookup);
                let mut mret = info.msg().method_return();
                let r = {
                    let mut ia = arg::IterAppend::new(&mut mret);
                    (handler)(iface, &mut ia, &mut pinfo)
                };
                if r { Ok(Some(mret)) } else { Err(MethodErr::failed(&"Failed to read property")) }
            })),
            vec!(), vec!()
        ));
    }
}

trait Introspectable {
    type Err;
    type Context;
    fn introspect(&self, info: &mut Self::Context) -> Result<String, Self::Err>;
}

pub struct DBusIntrospectable;

use crate::crossroads as cr;

/*
pub fn introspectable_ifaceinfo_par<I>() -> cr::IfaceInfo<'static, cr::Par>
where I: Introspectable<Err=MethodErr, Context=cr::ParInfo> + Send + Sync + 'static { unimplemented!()
}
*/
