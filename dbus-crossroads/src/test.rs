use crate::*;
use dbus::Message;
use std::cell::RefCell;
use dbus::arg::{Variant, RefArg};
use std::collections::HashMap;

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

    let mut cr = Crossroads::new();

    let iface = cr.register("com.example.dbusrs.crossroads.score", |b: &mut IfaceBuilder<Score>| {
        b.method("UpdateScore", ("change",), ("new_score", "call_times"), |_, score, (change,): (u16,)| {
            score.0 += change;
            score.1 += 1;
            Ok((score.0, score.1))
        });
        b.property::<u16, _>("Score")
            .get(|_, score| { Ok(score.0) })
            .set(|_, score, val| { score.0 = val; Ok(()) });
    });

    cr.insert("/", &[iface], Score(2, 0));

    let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
        "org.freedesktop.DBus.Properties", "Set", ("com.example.dbusrs.crossroads.score", "Score", Variant(7u16)));
    let r = dispatch_helper(&mut cr, msg);

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
        b.property("Bar").get(|_,_| Ok(0u8)).set(|_,_,_| Ok(()));
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
