extern crate dbus;

use std::sync::atomic::*;

#[allow(dead_code)]
mod generated;

impl generated::OrgFreedesktopDBusIntrospectable for () {
   fn introspect(&self) -> Result<String, ::dbus::Error> { Ok("I feel so introspected right now".into()) }
}

#[test]
fn test_main() {
    let f = dbus::tree::Factory::new_fn::<()>();
    let i1 = generated::orgfreedesktop_dbus_introspectable_server(&f, (), |minfo| minfo.path.get_data());
    let t = f.tree(()).add(f.object_path("/test", ()).add(i1));
    let c = dbus::Connection::get_private(dbus::BusType::Session).unwrap();
    t.set_registered(&c, true).unwrap();
    let cname = c.unique_name();
    let quit = std::sync::Arc::new(AtomicBool::new(false));
    let quit2 = quit.clone();
    let _ = std::thread::spawn(move || {
        // Old way
        let c2 = dbus::Connection::get_private(dbus::BusType::Session).unwrap();
        let m = dbus::Message::new_method_call(&cname, "/test", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
        let mut mrep = c2.send_with_reply_and_block(m, 1000).unwrap();
        let m2 = mrep.as_result().unwrap();
        assert_eq!(m2.read1(), Ok("I feel so introspected right now"));

        // New way
        let p = c2.with_path(cname, "/test", 1000);
        use generated::OrgFreedesktopDBusIntrospectable;
        assert_eq!(p.introspect().unwrap(), "I feel so introspected right now");

        quit2.store(true, Ordering::SeqCst);
    }); 
    for _ in t.run(&c, c.iter(100)) { if quit.load(Ordering::SeqCst) { break; } }
/*     */
}
