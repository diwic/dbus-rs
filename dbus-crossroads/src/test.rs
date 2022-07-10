use crate::*;
use dbus::Message;
use std::cell::RefCell;
use dbus::arg::{Variant, RefArg, PropMap};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_send() {
    fn is_send<T: Send>(_: &T) {}
    let c = Crossroads::new();
    dbg!(&c);
    is_send(&c);

    let ctx = Context::new(Message::new_method_call("a.b", "/", "a.b", "c").unwrap()).unwrap();
    dbg!(&ctx);
    is_send(&ctx);
}

fn dispatch_helper2(cr: &mut Crossroads, mut msg: Message) -> Vec<Message> {
    msg.set_serial(57);
    let r = RefCell::new(vec!());
    cr.handle_message(msg, &r).unwrap();
    r.into_inner()
}

fn dispatch_helper(cr: &mut Crossroads, msg: Message) -> Message {
    let mut r = dispatch_helper2(cr, msg);
    assert_eq!(r.len(), 1);
    r[0].as_result().unwrap();
    r.into_iter().next().unwrap()
}

#[test]
fn score() {
    struct Score(u16, u32);
    use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PPC;
    use dbus::message::SignalArgs;

    let mut cr = Crossroads::new();

    let iface = cr.register("com.example.dbusrs.crossroads.score", |b: &mut IfaceBuilder<Score>| {
        b.method("UpdateScore", ("change",), ("new_score", "call_times"), |_, score, (change,): (u16,)| {
            score.0 += change;
            score.1 += 1;
            Ok((score.0, score.1))
        });

        let prop_ch = b.property::<u16, _>("Score")
            .get(|_, score| { Ok(score.0) })
            .set(|_, score, val| { score.0 = val; Ok(Some(val)) })
            .changed_msg_fn();
        let msg = prop_ch(&"/somePath".into(), &734u16).unwrap();
        let ppc = PPC::from_message(&msg).unwrap();
        assert_eq!(&*ppc.interface_name, "com.example.dbusrs.crossroads.score");
        assert_eq!(ppc.changed_properties.get("Score").unwrap().0.as_u64(), Some(734));

    });

    cr.insert("/", &[iface], Score(2, 0));

    let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
        "org.freedesktop.DBus.Properties", "Set", ("com.example.dbusrs.crossroads.score", "Score", Variant(7u16)));
    let mut r = dispatch_helper2(&mut cr, msg);
    assert_eq!(r.len(), 2);
    if r[0].msg_type() == dbus::message::MessageType::Signal { r.swap(0, 1); }
    assert_eq!(r[0].msg_type(), dbus::message::MessageType::MethodReturn);
    assert_eq!(r[0].get_reply_serial().unwrap(), 57);

    let ppc = PPC::from_message(&r[1]).unwrap();
    assert_eq!(&*ppc.interface_name, "com.example.dbusrs.crossroads.score");
    assert_eq!(ppc.changed_properties.get("Score").unwrap().0.as_u64(), Some(7));

    let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
        "org.freedesktop.DBus.Properties", "Get", ("com.example.dbusrs.crossroads.score", "Score"));
    let r = dispatch_helper(&mut cr, msg);
    let q: Variant<u16> = r.read1().unwrap();
    assert_eq!(q.0, 7);

    let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
        "com.example.dbusrs.crossroads.score", "UpdateScore", (5u16,));
    let r = dispatch_helper(&mut cr, msg);
    let (new_score, call_times): (u16, u32) = r.read2().unwrap();
    assert_eq!(new_score, 12);
    assert_eq!(call_times, 1);

    let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
        "org.freedesktop.DBus.Properties", "GetAll", ("com.example.dbusrs.crossroads.score",));
    let r = dispatch_helper(&mut cr, msg);
    let q: HashMap<String, Variant<Box<dyn RefArg>>> = r.read1().unwrap();
    assert_eq!(q.get("Score").unwrap().0.as_u64(), Some(12));

}

// Copy-pasted from the D-Bus spec. Edited to avoid reordering issues
const INTROSPECT: &str = r###"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/com/example/sample_object0">
  <interface name="com.example.SampleInterface0">
    <method name="Bazify">
      <arg name="bar" type="(iiu)" direction="in"/>
      <arg name="bar" type="v" direction="out"/>
    </method>
    <method name="Frobate">
      <arg name="foo" type="i" direction="in"/>
      <arg name="bar" type="s" direction="out"/>
      <arg name="baz" type="a{us}" direction="out"/>
      <annotation name="org.freedesktop.DBus.Deprecated" value="true"/>
    </method>
    <method name="Mogrify">
      <arg name="bar" type="(iiav)" direction="in"/>
    </method>
    <signal name="Changed">
      <arg name="new_value" type="b"/>
    </signal>
    <property name="Bar" type="y" access="readwrite"/>
  </interface>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
  <interface name="org.freedesktop.DBus.Properties">
    <method name="Get">
      <arg name="interface_name" type="s" direction="in"/>
      <arg name="property_name" type="s" direction="in"/>
      <arg name="value" type="v" direction="out"/>
    </method>
    <method name="GetAll">
      <arg name="interface_name" type="s" direction="in"/>
      <arg name="properties" type="a{sv}" direction="out"/>
    </method>
    <method name="Set">
      <arg name="interface_name" type="s" direction="in"/>
      <arg name="property_name" type="s" direction="in"/>
      <arg name="value" type="v" direction="in"/>
    </method>
    <signal name="PropertiesChanged">
      <arg name="interface_name" type="s"/>
      <arg name="changed_properties" type="a{sv}"/>
      <arg name="invalidated_properties" type="as"/>
    </signal>
  </interface>
  <node name="another_child_of_sample_object"/>
  <node name="child_of_sample_object"/>
</node>"###;

#[test]
fn introspect() {
    let mut cr = Crossroads::new();
    let token = cr.register("com.example.SampleInterface0", |b| {
        b.method("Frobate", ("foo",), ("bar","baz"), |_, _, _: (i32,)| {
            let x: HashMap<u32, String> = Default::default();
            Ok(("",x))
        }).deprecated();
        b.method("Bazify", ("bar",), ("bar",), |_, _, _: ((i32, i32, u32),)| {
            Ok((Variant(5u8),))
        });
        b.method("Mogrify", ("bar",), (), |_, _, _: ((i32, i32, Vec<Variant<Box<dyn RefArg>>>),)| {
            Ok(())
        });
        b.signal::<(bool,), _>("Changed", ("new_value",));
        b.property("Bar").get(|_,_| Ok(0u8)).set(|_,_,_| Ok(None));
    });
    cr.insert("/com/example/sample_object0", &[token], ());
    cr.insert("/com/example/sample_object0/child_of_sample_object", &[], ());
    cr.insert("/com/example/sample_object0123", &[], ());
    cr.insert("/com/example/sample_object0/another_child_of_sample_object", &[], ());

    let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/com/example/sample_object0",
        "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
    let r = dispatch_helper(&mut cr, msg);
    let xml_data: &str = r.read1().unwrap();
    println!("{}", xml_data);
    assert_eq!(INTROSPECT, xml_data);
}

#[test]
fn object_manager() {
    struct Apple { radius: u32, weight: u32 }

    let mut cr = Crossroads::new();

    let add_remove = std::sync::Arc::new(std::sync::Mutex::new(vec!()));
    cr.set_object_manager_support(Some(add_remove.clone()));

    let weight_token = cr.register::<Apple, _, _>("com.example.dbusrs.weight", |b| {
        b.property("Weight").get(|_, apple| { Ok(apple.weight) });
    });
    let radius_token = cr.register::<Apple, _, _>("com.example.dbusrs.radius", |b| {
        b.property("Radius").get(|_, apple| { Ok(apple.radius) });
    });

    cr.insert("/list", &[cr.object_manager()], ());

    cr.insert("/list/grannysmith", &[weight_token, radius_token], Apple { radius: 10, weight: 20 });
    let v: Vec<_> = add_remove.lock().unwrap().drain(..).collect();
    assert_eq!(v.len(), 1);

    use dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesAdded as OMIA;
    use dbus::message::SignalArgs;
    assert_eq!(&*v[0].path().unwrap(), "/list");
    let omia = OMIA::from_message(&v[0]).unwrap();
    dbg!(&omia);
    assert_eq!(&*omia.object, "/list/grannysmith");
    let radius_iface = &omia.interfaces["com.example.dbusrs.radius"]["Radius"];
    let radius = radius_iface.0.as_u64().unwrap();
    assert_eq!(radius, 10);
    assert!(omia.interfaces.get("org.freedesktop.DBus.Introspectable").is_some());

    let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/list",
        "org.freedesktop.DBus.ObjectManager", "GetManagedObjects").unwrap();
    let r = dispatch_helper(&mut cr, msg);

    type GMO = HashMap<dbus::Path<'static>, HashMap<String, PropMap>>;
    let mo: GMO = r.read1().unwrap();
    dbg!(&mo);
    let v = &mo[&"/list/grannysmith".into()]["com.example.dbusrs.weight"]["Weight"];
    assert_eq!(v.0.as_u64().unwrap(), 20);
}

#[test]
fn object_manager_root() {
    let bus = dbus::blocking::Connection::new_session().unwrap();
    bus.request_name("com.example.dbusrs.objmgr_root", false, false, false).unwrap();
    let mut cr = Crossroads::new();
    cr.set_object_manager_support(Some(std::sync::Arc::new(std::sync::Mutex::new(vec!()))));
    cr.insert("/", &[cr.object_manager()], ());
    cr.insert("/foo", &[], ());

    use dbus::channel::MatchingReceiver;
    let shared_cr = std::sync::Arc::new(std::sync::Mutex::new(cr));
    let altcr = shared_cr.clone();
    bus.start_receive(dbus::message::MatchRule::new_method_call(), Box::new(move |msg, conn| {
	altcr.lock().unwrap().handle_message(msg, conn).unwrap();
	true
    }));

    let service_thread = std::thread::spawn(move || {
        bus.process(std::time::Duration::new(u64::MAX, 0)).unwrap();
    });

    let msg = Message::new_method_call("com.example.dbusrs.objmgr_root", "/",
        "org.freedesktop.DBus.ObjectManager", "GetManagedObjects").unwrap();
    dispatch_helper(&mut shared_cr.lock().unwrap(), msg);
    service_thread.join().unwrap();
}

#[test]
fn properties_get_all() {
    let bus = dbus::blocking::Connection::new_session().unwrap();
    bus.request_name("com.example.dbusrs.properties", false, false, false).unwrap();

    let mut cr = Crossroads::new();
    let iface = cr.register("com.example.dbusrs.properties", |b| {
        b.property("One").get(|_, _| Ok(1));
        b.property("Two").get(|_, _| Ok(2));
    });
    cr.insert("/", &[iface], ());

    let msg = Message::call_with_args(
        "com.example.dbusrs.properties",
        "/",
        "org.freedesktop.DBus.Properties",
        "GetAll",
        ("com.example.dbusrs.properties",),
    );
    let r = dispatch_helper(&mut cr, msg);
    let response: HashMap<String, Variant<Box<dyn RefArg>>> = r.read1().unwrap();
    assert_eq!(response.get("One").unwrap().as_i64(), Some(1));
    assert_eq!(response.get("Two").unwrap().as_i64(), Some(2));
    assert_eq!(response.len(), 2);
}

#[tokio::test]
async fn properties_get_all_async() {
    use dbus::channel::MatchingReceiver;

    let (resource, bus) = dbus_tokio::connection::new_session_sync().unwrap();
    tokio::spawn(async {resource.await;});
    bus.request_name("com.example.dbusrs.properties", false, false, false).await.unwrap();

    let mut cr = Crossroads::new();
    let spawner = Box::new(|fut| { tokio::spawn(fut); });
    cr.set_async_support(Some((bus.clone(), spawner)));

    let iface = cr.register("com.example.dbusrs.properties", |b| {
        b.property("Sync").get(|_, _| Ok(1));
        b.property("Async").get_async(|mut ctx, _| async move {
            ctx.reply(Ok(2))
        });
    });
    cr.insert("/", &[iface], ());
    bus.start_receive(
        dbus::message::MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            cr.handle_message(msg, conn).unwrap();
            true
        })
    );

    let proxy = dbus::nonblock::Proxy::new("com.example.dbusrs.properties", "/", Duration::from_secs(3600), bus);
    let (response,): (HashMap<String, Variant<Box<dyn RefArg>>>,) = proxy.method_call(
        "org.freedesktop.DBus.Properties",
        "GetAll",
        ("com.example.dbusrs.properties",)
    ).await.unwrap();
    assert_eq!(response.get("Sync").unwrap().as_i64(), Some(1));
    assert_eq!(response.get("Async").unwrap().as_i64(), Some(2));
    assert_eq!(response.len(), 2);
}
