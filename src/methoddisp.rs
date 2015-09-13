#![allow(dead_code)]

use {MessageItem, Message, MessageType, Connection, Error, ErrorName, Signature, Member, Path, Interface as IfaceName};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ffi::CString;
use std::fmt;

type ArcMap<K, V> = BTreeMap<Arc<K>, Arc<V>>;

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct Argument(Option<String>, Signature);

impl Argument {
    pub fn new(name: Option<String>, sig: Signature) -> Argument { Argument(name, sig) }

    fn introspect(&self, indent: &str, dir: &str) -> String { 
        let n = self.0.as_ref().map(|n| format!("name=\"{}\" ", n)).unwrap_or("".into());
        format!("{}<arg {}type=\"{}\"{}/>\n", indent, n, self.1, dir)
    }
    fn introspect_all(args: &[Argument], indent: &str, dir: &str) -> String {
        args.iter().fold("".to_string(), |aa, az| format!("{}{}", aa, az.introspect(indent, dir)))
    }
}

// Doesn't work, conflicting impls
// impl<S: Into<Signature>> From<S> for Argument

impl From<Signature> for Argument {
    fn from(t: Signature) -> Argument { Argument(None, t) }
}

impl<'a> From<&'a str> for Argument {
    fn from(t: &str) -> Argument { Argument(None, t.into()) }
}

impl<N: Into<String>, S: Into<Signature>> From<(N, S)> for Argument {
    fn from((n, s): (N, S)) -> Argument { Argument(Some(n.into()), s.into()) }
}

pub struct MethodErr(ErrorName, String);

impl<T: Into<ErrorName>, M: Into<String>> From<(T, M)> for MethodErr {
    fn from((t, m): (T, M)) -> MethodErr { MethodErr(t.into(), m.into()) }
}

pub type MethodResult = Result<Vec<Message>, MethodErr>;

pub struct MethodFn<'a>(Box<Fn(&Message, &ObjectPath<MethodFn<'a>>, &Interface<MethodFn<'a>>) -> MethodResult + 'a>);
pub struct MethodFnMut<'a>(Box<RefCell<FnMut(&Message, &ObjectPath<MethodFnMut<'a>>, &Interface<MethodFnMut<'a>>) -> MethodResult + 'a>>);
pub struct MethodSync(Box<Fn(&Message, &ObjectPath<MethodSync>, &Interface<MethodSync>) -> MethodResult + Send + Sync + 'static>);

trait MCall: Sized {
    fn call_method(&self, m: &Message, o: &ObjectPath<Self>, i: &Interface<Self>) -> MethodResult;
    fn box_method<H>(h: H) -> Self
    where H: Fn(&Message, &ObjectPath<Self>, &Interface<Self>) -> MethodResult + Send + Sync + 'static;
}

impl<'a> MCall for MethodFn<'a> {
    fn call_method(&self, m: &Message, o: &ObjectPath<MethodFn<'a>>, i: &Interface<MethodFn<'a>>) -> MethodResult { self.0(m, o, i) }

    fn box_method<H>(h: H) -> Self
    where H: Fn(&Message, &ObjectPath<MethodFn<'a>>, &Interface<MethodFn<'a>>) -> MethodResult + Send + Sync + 'static {
        MethodFn(Box::new(h))
    }
}

impl MCall for MethodSync {
    fn call_method(&self, m: &Message, o: &ObjectPath<MethodSync>, i: &Interface<MethodSync>) -> MethodResult { self.0(m, o, i) }

    fn box_method<H>(h: H) -> Self
    where H: Fn(&Message, &ObjectPath<MethodSync>, &Interface<MethodSync>) -> MethodResult + Send + Sync + 'static {
        MethodSync(Box::new(h))
    }
}

impl<'a> MCall for MethodFnMut<'a> {
    fn call_method(&self, m: &Message, o: &ObjectPath<MethodFnMut<'a>>, i: &Interface<MethodFnMut<'a>>) -> MethodResult {
        let mut z = self.0.borrow_mut();
        (&mut *z)(m, o, i)
    }

    fn box_method<H>(h: H) -> Self
    where H: Fn(&Message, &ObjectPath<MethodFnMut<'a>>, &Interface<MethodFnMut<'a>>) -> MethodResult + Send + Sync + 'static {
        MethodFnMut(Box::new(RefCell::new(h)))
    }
}

pub struct Method<M> {
    cb: M,
    name: Arc<Member>,
    i_args: Vec<Argument>,
    o_args: Vec<Argument>,
    anns: BTreeMap<String, String>,
    owner: Option<Arc<Interface<M>>>,
}

/*
impl<'a> Method<MethodFn<'a>> {
    /// Creates a new method for single-thread use.
    pub fn new_fn<H: 'a, T>(t: T, handler: H) -> Self
        where H: Fn(&Message) -> MethodResult, T: Into<Member> {
        Method { name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
            cb: MethodFn(Box::new(handler))
        }
    }
}

impl<'a> Method<MethodSync> {
    /// Creates a new method for multi-thread use.
    /// This puts bounds on the callback to enable it to be called from several threads
    /// in parallel.
    pub fn new_sync<H: 'a, T>(t: T, handler: H) -> Self
        where H: Fn(&Message) -> MethodResult + Send + Sync + 'static, T: Into<Member> {
        Method { name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
            cb: MethodSync(Box::new(handler))
        }
    }
}

impl<'a> Method<MethodFnMut<'a>> {
    /// Creates a new method for single-thread use.
    /// This function can mutate its environment, so if you try to call the
    /// callback from within the callback itself, you'll get a RefCell panic.
    pub fn new_fnmut<H: 'a, T>(t: T, handler: H) -> Self
        where H: FnMut(&Message) -> MethodResult, T: Into<Member> {
        Method {name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
             cb: MethodFnMut(Box::new(RefCell::new(handler)))
        }
    }
}
*/

impl<M> Method<M> {
    pub fn in_arg<A: Into<Argument>>(mut self, a: A) -> Self { self.i_args.push(a.into()); self }
/*    pub fn in_args<A: Into<Argument> + Clone>(mut self, a: &[A]) -> Self {
        self.i_args.extend(a.iter().map(|b| b.clone().into())); self
    }*/
    pub fn in_args<Z: Into<Argument>, A: IntoIterator<Item=Z>>(mut self, a: A) -> Self {
        self.i_args.extend(a.into_iter().map(|b| b.into())); self
    }

    pub fn out_arg<A: Into<Argument>>(mut self, a: A) -> Self { self.o_args.push(a.into()); self }

    /*pub fn out_args<A: Into<Argument> + Clone>(&mut self, a: &[A]) -> &mut Method<M> {
        self.o_args.extend(a.iter().map(|b| b.clone().into())); self
    }*/
    pub fn out_args<Z: Into<Argument>, A: IntoIterator<Item=Z>>(mut self, a: A) -> Self {
        self.o_args.extend(a.into_iter().map(|b| b.into())); self
    }

    /// Add an annotation to the method
    pub fn annotate<N: Into<String>, V: Into<String>>(mut self, name: N, value: V) -> Self {
        self.anns.insert(name.into(), value.into()); self
    }
}

impl<M: MCall> Method<M> {
    pub fn call(&self, m: &Message, o: &ObjectPath<M>, i: &Interface<M>) -> MethodResult { self.cb.call_method(m, o, i) }
}

pub struct Interface<M> {
    name: Arc<IfaceName>,
    methods: ArcMap<Member, Method<M>>,
    signals: ArcMap<Member, Signal>,
    properties: ArcMap<String, Property>,
    anns: BTreeMap<String, String>,
}

/*
#[test]
fn test() {
    Method::new_sync("Mooh", |_a| unreachable!() ).in_arg("s").out_arg(("foo", "b"));
    Method::new_fnmut("EatSandwich", |_| unreachable!()).out_args(vec![("foo", "b"), ("bar", "s")]);
}
*/

impl<M> Interface<M> {
    /// Adds a method to the interface.
    pub fn add_m(mut self, m: Method<M>) -> Self { self.methods.insert(m.name.clone(), Arc::new(m)); self }
    /// Adds a signal to the interface.
    pub fn add_s(mut self, s: Signal) -> Self { self.signals.insert(s.name.clone(), Arc::new(s)); self }
    /// Adds a signal to the interface. Returns a reference to the signal
    /// (which you can use to emit the signal, once it belongs to an object path).
    pub fn add_s_ref(&mut self, s: Signal) -> Arc<Signal> {
        let s = Arc::new(s);
        self.signals.insert(s.name.clone(), s.clone());
        s
    }

    /// Adds a signal to the interface.
    pub fn add_p(mut self, p: Property) -> Self { self.properties.insert(p.name.clone(), Arc::new(p)); self }
    /// Adds a property to the interface. Returns a reference to the property
    /// (which you can use to get and set the current value of the property).
    pub fn add_p_ref(&mut self, p: Property) -> Arc<Property> {
        let p = Arc::new(p);
        self.properties.insert(p.name.clone(), p.clone());
        p
    }

    pub fn annotate<N: Into<String>, V: Into<String>>(mut self, name: N, value: V) -> Self {
        self.anns.insert(name.into(), value.into()); self
    }
    pub fn deprecated(self) -> Self { self.annotate("org.freedesktop.DBus.Deprecated", "true") }
}

#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
pub enum EmitsChangedSignal {
    True,
    Invalidates,
    Const,
    False,
}

pub struct Property {
    name: Arc<String>,
    value: Mutex<MessageItem>,
    emits: EmitsChangedSignal,
    owner: Mutex<Option<(Arc<Path>, Arc<IfaceName>)>>,
    anns: BTreeMap<String, String>,
}

impl Property {
    pub fn get_value(&self) -> MessageItem {
        self.value.lock().unwrap().clone()
    }

/*    pub fn get_signal(&self) -> Option<Message> {
        self.owner.lock().unwrap().map(|p, i| {
            Message::signal(p, "org.freedesktop.DBus.Properties".into(), "PropertiesChanged".into()).append(i)
        })
    }
*/
    /// Returns error if "emits" is "Const", and the property is in a tree.
    /// Returns messages to be sent over a connection, this could be the PropertiesChanged signal.
    pub fn set_value(&self, m: MessageItem) -> Result<Vec<Message>,()> {
/*        self.owner.lock().unwrap().map(|p, i| {
            let s = Message::signal(p, "org.freedesktop.DBus.Properties".into(), "PropertiesChanged".into()).append(i);
            let ss = match self.emits {
                EmitsChangedSignal::True => { s.append_items(&[(self.name, m.clone()).into()]); Ok(Some(s)) },
                EmitsChangedSignal::Invalidates => s.append_items(&[MessageItem::( self.name.into()]),
                EmitsChangedSignal::True => s.append_items(&[(self.name, m.clone()).into()]),
            }
        })
        if let Some(ref p, ref i) = &self.owner.lock().unwrap() {
            let s = Message::signal(p, "org.freedesktop.DBus.Properties".into(), "PropertiesChanged".into())
                .append(i);
        } else { Ok(vec!()) } */
        *self.value.lock().unwrap() = m;
        Ok(vec!())
    }

    pub fn emits_changed(mut self, e: EmitsChangedSignal) -> Self { self.emits = e; self }
}

pub struct Signal {
    name: Arc<Member>,
    arguments: Vec<Argument>,
    owner: Mutex<Option<(Arc<Path>, Arc<IfaceName>)>>,
    anns: BTreeMap<String, String>,
}

impl Signal {
    /// Returns a message which emits the signal when sent.
    /// Panics if the signal is not inserted in an object path.
    pub fn emit(&self, items: &[MessageItem]) -> Message {
        let mut m = {
            let lock = self.owner.lock().unwrap();
            let &(ref p, ref i) = lock.as_ref().unwrap();
            Message::signal(p, i, &self.name)
        };
        m.append_items(items);
        m
    }

    pub fn arg<A: Into<Argument>>(mut self, a: A) -> Self { self.arguments.push(a.into()); self }
    pub fn args<Z: Into<Argument>, A: IntoIterator<Item=Z>>(mut self, a: A) -> Self {
        self.arguments.extend(a.into_iter().map(|b| b.into())); self
    }
}

fn introspect_anns(anns: &BTreeMap<String, String>, indent: &str) -> String {
    anns.iter().fold("".into(), |aa, (ak, av)| {
        format!("{}{}<annotation name=\"{}\" value=\"{}\"/>\n", aa, indent, ak, av)
    })
}

fn introspect_map<T, I: fmt::Display, C: Fn(&T) -> (String, String)>
    (h: &ArcMap<I, T>, name: &str, indent: &str, func: C) -> String {

    h.iter().fold("".into(), |a, (k, v)| {
        let (params, contents) = func(v);
        format!("{}{}<{} name=\"{}\"{}{}>\n",
            a, indent, name, &**k, params, if contents.len() > 0 {
                format!(">\n{}{}</{}", contents, indent, name)
            }
            else { format!("/") }
        )
    })
}

pub struct ObjectPath<M> {
    name: Arc<Path>,
    ifaces: ArcMap<IfaceName, Interface<M>>,
}

impl<M: MCall> ObjectPath<M> {

    fn add_property_handler(&mut self) {
        let ifname = IfaceName::from("org.freedesktop.DBus.Properties");
        if self.ifaces.contains_key(&ifname) { return };
        let f: Factory<M> = Factory(PhantomData);
        let i = f.interface(ifname)
            .add_m(f.method_sync("Get", |_,_,_| unimplemented!())
                .in_arg(("interface_name", "s")).in_arg(("property_name", "s")).out_arg(("value", "v")))
            .add_m(f.method_sync("GetAll", |_,_,_| unimplemented!())
                .in_arg(("interface_name", "s")).out_arg(("props", "a{sv}")))
            .add_m(f.method_sync("Set", |_,_,_| unimplemented!())
                .in_args(vec!(("interface_name", "s"), ("property_name", "s"), ("value", "v"))));
        self.ifaces.insert(i.name.clone(), Arc::new(i));
    }

    pub fn add(mut self, p: Interface<M>) -> Self {
        for s in p.signals.values() {
            *s.owner.lock().unwrap() = Some((self.name.clone(), p.name.clone()))
        };
        for s in p.properties.values() {
            *s.owner.lock().unwrap() = Some((self.name.clone(), p.name.clone()))
        };
        if !p.properties.is_empty() { self.add_property_handler(); }
        self.ifaces.insert(p.name.clone(), Arc::new(p));
        self
    }

    /// Adds introspection support for this object path.
    pub fn introspectable(self) -> Self {
        let ifname: IfaceName = "org.freedesktop.DBus.Introspectable".into();
        if self.ifaces.contains_key(&ifname) { return self };
        let f: Factory<M> = Factory(PhantomData);
        self.add(f.interface(ifname)
            .add_m(f.method_sync("Introspect",
                |m,o,_| Ok(vec!(m.method_return().append(o.introspect()))))
                .out_arg(("xml_data", "s"))))
    }

    fn handle(&self, m: &Message) -> MethodResult {
        let i = try!(m.interface().and_then(|i| self.ifaces.get(&i)).ok_or(
            ("org.freedesktop.DBus.Error.UnknownInterface", "Unknown interface")));
        let me = try!(m.member().and_then(|me| i.methods.get(&me)).ok_or(
            ("org.freedesktop.DBus.Error.UnknownMethod", "Unknown method")));
        me.call(m, &self, i)
    }

    fn introspect(&self) -> String {
        let ifacestr = introspect_map(&self.ifaces, "interface", "  ", |iv|
            (format!(""), format!("{}{}{}",
                introspect_map(&iv.methods, "method", "    ", |m| (format!(""), format!("{}{}{}",
                    Argument::introspect_all(&m.i_args, "      ", " direction=\"in\""),
                    Argument::introspect_all(&m.o_args, "      ", " direction=\"out\""),
                    introspect_anns(&m.anns, "      ")
                ))),
                introspect_map(&iv.properties, "property", "    ", |p| (
                    format!(" type=\"{}\" access=\"{}\"", p.get_value().type_sig(), "read"), // FIXME: rw/wo props too
                    introspect_anns(&p.anns, "      ")
                )),
                introspect_map(&iv.signals, "signal", "    ", |s| (format!(""), format!("{}{}",
                    Argument::introspect_all(&s.arguments, "      ", ""),
                    introspect_anns(&s.anns, "      ")
                )))
            ))
        );
        let childstr = ""; // FIXME
        let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, self.name, ifacestr, childstr);
        nodestr
    }
}


pub struct Tree<M> {
    paths: ArcMap<Path, ObjectPath<M>>
}

impl<M: MCall> Tree<M> {
    pub fn add(mut self, p: ObjectPath<M>) -> Self {
        self.paths.insert(p.name.clone(), Arc::new(p)); self
    }

    /// Registers or unregisters all object paths in the tree.
    /// FIXME: On error, should unregister the already registered paths.
    pub fn set_registered(&self, c: &Connection, b: bool) -> Result<(), Error> {
        for p in self.paths.keys() {
            if b { try!(c.register_object_path(p)); }
            else { c.unregister_object_path(p); }
        }
        Ok(())
    }

    /// Handles a message. Will return None in case the object path was not
    /// found, or otherwise a list of messages to be sent back.
    pub fn handle(&self, m: &Message) -> Option<Vec<Message>> {
        if m.msg_type() != MessageType::MethodCall { None }
        else { m.path().and_then(|p| self.paths.get(&p).map(|s| s.handle(m)
            .unwrap_or_else(|e| vec!(m.error(&e.0, &CString::new(e.1).unwrap()))))) }
    }
}

/// The factory is used to create object paths, interfaces, methods etc.
pub struct Factory<M>(PhantomData<M>);

impl<'a> Factory<MethodFn<'a>> {
    
    /// Creates a new factory for single-thread use.
    pub fn new_fn() -> Self { Factory(PhantomData) }

    /// Creates a new method for single-thread use.
    pub fn method<H: 'a, T>(&self, t: T, handler: H) -> Method<MethodFn<'a>>
        where H: Fn(&Message, &ObjectPath<MethodFn<'a>>, &Interface<MethodFn<'a>>) -> MethodResult, T: Into<Member> {
        Method { name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
            cb: MethodFn(Box::new(handler))
        }
    }
}

impl Factory<MethodSync> {
    
    /// Creates a new factory for multi-thread use.
    /// Trees created will be able to Send and Sync, i e,
    /// it can handle several messages in parallel.
    pub fn new_sync() -> Self { Factory(PhantomData) }

    /// Creates a new method for multi-thread use.
    /// This puts bounds on the callback to enable it to be called from several threads
    /// in parallel.
    pub fn method<H, T>(&self, t: T, handler: H) -> Method<MethodSync>
    where H: Fn(&Message, &ObjectPath<MethodSync>, &Interface<MethodSync>) -> MethodResult + Send + Sync + 'static, T: Into<Member> {
        Method { name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
            cb: MethodSync(Box::new(handler))
        }
    }
}

impl<M> Factory<M> {

    pub fn tree(&self) -> Tree<M> { Tree { paths: BTreeMap::new() }}

    pub fn object_path<T: Into<Path>>(&self, t: T) -> ObjectPath<M> {
        ObjectPath { name: Arc::new(t.into()), ifaces: BTreeMap::new() }
    }

    pub fn interface<T: Into<IfaceName>>(&self, t: T) -> Interface<M> {
        Interface { name: Arc::new(t.into()), methods: BTreeMap::new(), signals: BTreeMap::new(),
            properties: BTreeMap::new(), anns: BTreeMap::new()
        }
    }

    pub fn signal<T: Into<Member>>(&self, t: T) -> Signal {
        Signal { name: Arc::new(t.into()), arguments: vec!(), owner: Mutex::new(None), anns: BTreeMap::new() }
    }

    pub fn property<T: Into<String>, I: Into<MessageItem>>(&self, t: T, i: I) -> Property {
        Property { name: Arc::new(t.into()), emits: EmitsChangedSignal::True,
            value: Mutex::new(i.into()), owner: Mutex::new(None), anns: BTreeMap::new() }
    }
}

impl<M: MCall> Factory<M> {
    /// Creates a new method with bounds enough to be used in all trees.
    pub fn method_sync<H, T>(&self, t: T, handler: H) -> Method<M>
    where H: Fn(&Message, &ObjectPath<M>, &Interface<M>) -> MethodResult + Send + Sync + 'static, T: Into<Member> {
        Method { name: Arc::new(t.into()), i_args: vec!(), o_args: vec!(), anns: BTreeMap::new(), owner: None,
            cb: M::box_method(handler)
        }
    }
}

#[test]
fn factory_test() {
    let f = Factory::new_fn();
    f.interface("com.example.hello").deprecated();
    let b = 5i32;
    f.method("GetSomething", move |m,_,_| Ok(vec!({ let mut z = m.method_return(); z.append_items(&[b.into()]); z})));
    let _ = f.tree().add(f.object_path("/funghi").add(f.interface("a.b.c").deprecated()));
}


#[test]
fn test_introspection() {
    let f = Factory::new_sync();
    let t = f.object_path("/echo").introspectable()
        .add(f.interface("com.example.echo")
            .add_m(f.method("Echo", |_,_,_| unimplemented!()).in_arg(("request", "s")).out_arg(("reply", "s")))
            .add_p(f.property("EchoCount", 7i32))
            .add_s(f.signal("Echoed").arg(("data", "s")))
    );
/*    
    let msg = Message::new_method_call("com.example.echoserver", "/echo", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
    let reply = t.handle(&msg);

    let v: Vec<MessageItem> = reply.get(0).unwrap().get_items(); 
    let actual_result: &str = v.get(0).unwrap().inner().unwrap();
*/
    let actual_result = t.introspect();
    println!("\n=== Introspection XML start ===\n{}\n=== Introspection XML end ===", actual_result);

    let expected_result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/echo">
  <interface name="com.example.echo">
    <method name="Echo">
      <arg name="request" type="s" direction="in"/>
      <arg name="reply" type="s" direction="out"/>
    </method>
    <property name="EchoCount" type="i" access="read"/>
    <signal name="Echoed">
      <arg name="data" type="s"/>
    </signal>
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
      <arg name="props" type="a{sv}" direction="out"/>
    </method>
    <method name="Set">
      <arg name="interface_name" type="s" direction="in"/>
      <arg name="property_name" type="s" direction="in"/>
      <arg name="value" type="v" direction="in"/>
    </method>
  </interface>
</node>"##;
 
    assert_eq!(expected_result, actual_result);   
}

