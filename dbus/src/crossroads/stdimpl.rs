use super::crossroads::Crossroads;
use super::handlers::SyncInfo;
use super::info::{IfaceInfo, MethodInfo, PropInfo};
use crate::arg;

pub struct DBusProperties;

impl DBusProperties {
    pub fn register(cr: &mut Crossroads<()>) {
        cr.register::<Self>(IfaceInfo::new("org.freedesktop.DBus.Properties",
            vec!(MethodInfo::new_sync("Get", |_: &DBusProperties, msg, info| {
                // FIXME, handle errors correctly here
                let (iname, propname) = msg.read2().unwrap(); 
                let (lookup, pinfo) = info.cr.reg_prop_lookup(info.pd, iname, propname).unwrap();
                let handler = &pinfo.handlers.0.as_ref().unwrap();
                let mut si = SyncInfo { cr: lookup.cr, pd: lookup.data };
                let mut mret = msg.method_return();
                let r = {
                    let mut ia = arg::IterAppend::new(&mut mret);
                    (handler)(&**lookup.iface, msg, &mut ia, &mut si)
                };
                if r { Some(mret) } else { None }
            })),
            vec!(), vec!()
        ));
    }
}
