use std::sync::atomic::*;
use std::sync::Arc;
use std::convert::AsRef;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_asref_mtsync;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_client;

struct Whatever {}

use dbus::arg;

impl AsRef<dyn policykit_asref_mtsync::OrgFreedesktopDBusProperties + 'static> for Arc<Whatever> {
    fn as_ref(&self) -> &(dyn policykit_asref_mtsync::OrgFreedesktopDBusProperties + 'static) { &**self }
}

impl policykit_asref_mtsync::OrgFreedesktopDBusProperties for Whatever {
    fn get(&self, interfacename: &str, propertyname: &str) -> Result<::dbus::arg::Variant<Box<dyn arg::RefArg>>, ::dbus::MethodErr> {
        assert_eq!(interfacename, "Interface.Name");
        assert_eq!(propertyname, "Property.Name");
        Ok(::dbus::arg::Variant(Box::new(5u8)))
    }

    fn get_all(&self, _interfacename: &str) ->
    Result<::std::collections::HashMap<String, ::dbus::arg::Variant<Box<dyn arg::RefArg>>>, ::dbus::MethodErr> { unimplemented!() }

    fn set(&self, _interfacename: &str, _propertyname: &str, value: ::dbus::arg::Variant<Box<dyn arg::RefArg>>) -> Result<(), ::dbus::MethodErr> {
        assert_eq!(dbus::arg::RefArg::as_str(&value), Some("Hello"));
        Err(("A.B.C", "Error.Message").into())
    }

}


#[test]
fn test_asref_mtsync() {
    let f = dbus_tree::Factory::new_sync::<()>();
    let x = Arc::new(Whatever {});
    let i1 = policykit_asref_mtsync::org_freedesktop_dbus_properties_server(&f, (), move |_| { x.clone() });
    let t = f.tree(()).add(f.object_path("/test", ()).add(i1));
    let c = dbus::ffidisp::Connection::new_session().unwrap();
    t.set_registered(&c, true).unwrap();
    let cname = c.unique_name();
    let quit = std::sync::Arc::new(AtomicBool::new(false));
    let quit2 = quit.clone();
    let _ = std::thread::spawn(move || {
        use policykit_client::OrgFreedesktopDBusProperties;
        use dbus::arg::RefArg;

        let c2 = dbus::ffidisp::Connection::new_session().unwrap();
        let p = c2.with_path(cname, "/test", 1000);
        let v = p.get("Interface.Name", "Property.Name").unwrap();
        assert_eq!(v.as_i64(), Some(5));

        let vv = p.set("Interface.Name", "Property.Name", dbus::arg::Variant(Box::new("Hello".to_string())));
        assert_eq!(vv.unwrap_err().message(), Some("Error.Message"));

        quit2.store(true, Ordering::SeqCst);
    });
    for _ in t.run(&c, c.iter(100)) { if quit.load(Ordering::SeqCst) { break; } }
}
