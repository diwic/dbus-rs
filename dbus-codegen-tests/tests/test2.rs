extern crate dbus;

use std::sync::atomic::*;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_client;


impl policykit::OrgFreedesktopDBusProperties for () {
    fn get(&self, interfacename: &str, propertyname: &str) -> Result<::dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>, ::dbus::tree::MethodErr> {
        assert_eq!(interfacename, "Interface.Name");
        assert_eq!(propertyname, "Property.Name");
        Ok(::dbus::arg::Variant(Box::new(5u8)))
    }

    fn get_all(&self, _interfacename: &str) ->
    Result<::std::collections::HashMap<String, ::dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>, ::dbus::tree::MethodErr> { unimplemented!() }

    fn set(&self, _interfacename: &str, _propertyname: &str, value: ::dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>) -> Result<(), ::dbus::tree::MethodErr> {
        assert_eq!(dbus::arg::RefArg::as_str(&value), Some("Hello"));
        Err(("A.B.C", "Error.Message").into())
    }

}

#[test]
fn test2() {
    let f = dbus::tree::Factory::new_fn::<()>();
    let i1 = policykit::org_freedesktop_dbus_properties_server(&f, (), |minfo| minfo.path.get_data());
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
