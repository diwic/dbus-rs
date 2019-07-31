extern crate dbus;

#[allow(dead_code)]
mod policykit;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_client;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_blocking;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_nonblock;


use std::sync::atomic::*;

impl policykit::OrgFreedesktopDBusIntrospectable for () {
   fn introspect(&self) -> Result<String, dbus::tree::MethodErr> { Ok("I feel so introspected right now".into()) }
}

#[test]
fn test_main() {
    let f = dbus::tree::Factory::new_fn::<()>();
    let i1 = policykit::org_freedesktop_dbus_introspectable_server(&f, (), |minfo| minfo.path.get_data());
    let t = f.tree(()).add(f.object_path("/test", ()).add(i1));
    let c = dbus::ffidisp::Connection::new_session().unwrap();
    t.set_registered(&c, true).unwrap();
    let cname = c.unique_name();
    let quit = std::sync::Arc::new(AtomicBool::new(false));
    let quit2 = quit.clone();
    let _ = std::thread::spawn(move || {
        let c2 = dbus::ffidisp::Connection::new_session().unwrap();
        // Older way
        {
            let m = dbus::Message::new_method_call(&cname, "/test", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
            let mut mrep = c2.send_with_reply_and_block(m, 1000).unwrap();
            let m2 = mrep.as_result().unwrap();
            assert_eq!(m2.read1(), Ok("I feel so introspected right now"));
        }

        // Old way
        {
            let p = c2.with_path(&cname, "/test", 1000);
            use policykit_client::OrgFreedesktopDBusIntrospectable;
            assert_eq!(p.introspect().unwrap(), "I feel so introspected right now");
        }

        // New way
        {
            let c3 = dbus::blocking::Connection::new_session().unwrap();
            let p = c3.with_proxy(cname, "/test", std::time::Duration::from_millis(1000));
            use policykit_blocking::OrgFreedesktopDBusIntrospectable;
            assert_eq!(p.introspect().unwrap(), "I feel so introspected right now");
        }

        quit2.store(true, Ordering::SeqCst);
    });

    for _ in t.run(&c, c.iter(100)) { if quit.load(Ordering::SeqCst) { break; } }
}
