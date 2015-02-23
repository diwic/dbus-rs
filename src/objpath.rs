use super::{Connection, Message, MessageItem, Error, TypeSig};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::cell::{Cell, RefCell};
use std::borrow::IntoCow;

pub struct Argument<'a> {
    name: &'a str,
    sig: TypeSig<'a>,
}

impl<'a> Argument<'a> {
    pub fn new<T: IntoCow<'a, str>>(name: &'a str, sig: T) -> Argument<'a> {
        Argument { name: name, sig: sig.into_cow() }
    }
}

pub struct Signal<'a> {
    name: String,
    args: Vec<Argument<'a>>,
}

impl<'a> Signal<'a> {
    pub fn new<N: ToString>(name: N, args: Vec<Argument<'a>>) -> Signal<'a> {
        Signal { name: name.to_string(), args: args }
    }
}


pub type MethodResult = Result<Vec<MessageItem>, (&'static str, String)>;
pub type PropertyGetResult = Result<MessageItem, (&'static str, String)>;
pub type PropertySetResult = Result<(), (&'static str, String)>;

pub type MethodHandler<'a> = Box<Fn(&mut Message) -> MethodResult + 'a>;

struct IMethod<'a> {
    in_args: Vec<Argument<'a>>,
    out_args: Vec<Argument<'a>>,
    cb: Rc<MethodHandler<'a>>,
}

pub struct Method<'a> {
    name: String,
    i: IMethod<'a>
}

impl<'a> Method<'a> {
    pub fn new<N: ToString>(name: N, in_args: Vec<Argument<'a>>,
            out_args: Vec<Argument<'a>>, cb: MethodHandler<'a>) -> Method<'a> {
        Method { name: name.to_string(), i: IMethod {
            in_args: in_args, out_args: out_args, cb: Rc::new(cb) }
        }
    }
}

pub trait PropertyHandler {
    fn get(&self) -> PropertyGetResult;
    fn set(&self, &MessageItem) -> PropertySetResult;
}

pub trait PropertyGetHandler {
    fn get(&self) -> PropertyGetResult;
}

pub trait PropertySetHandler {
    fn set(&self, &MessageItem) -> PropertySetResult;
}

pub enum PropertyAccess<'a> {
    RO(Box<PropertyGetHandler+'a>),
    RW(Box<PropertyHandler+'a>),
    WO(Box<PropertySetHandler+'a>),
}

struct IProperty<'a> {
    sig: TypeSig<'a>,
    access: PropertyAccess<'a>,
}

pub struct Property<'a> {
    name: String,
    i: IProperty<'a>
}

impl<'a> Property<'a> {
    fn new<N: ToString>(name: N, sig: TypeSig<'a>, a: PropertyAccess<'a>) -> Property<'a> {
        Property { name: name.to_string(), i: IProperty { sig: sig, access: a } }
    }
    pub fn new_ro<N: ToString>(name: N, sig: TypeSig<'a>, h: Box<PropertyGetHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::RO(h))
    }
    pub fn new_rw<N: ToString>(name: N, sig: TypeSig<'a>, h: Box<PropertyHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::RW(h))
    }
    pub fn new_wo<N: ToString>(name: N, sig: TypeSig<'a>, h: Box<PropertySetHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::WO(h))
    }
}

pub struct Interface<'a> {
    methods: BTreeMap<String, IMethod<'a>>,
    properties: BTreeMap<String, IProperty<'a>>,
    signals: BTreeMap<String, Vec<Argument<'a>>>,
}

impl<'a> Interface<'a> {
    pub fn new(m: Vec<Method<'a>>, p: Vec<Property<'a>>, s: Vec<Signal<'a>>) -> Interface<'a> {
        Interface {
           methods: m.into_iter().map(|m| (m.name, m.i)).collect(),
           properties: p.into_iter().map(|p| (p.name, p.i)).collect(),
           signals: s.into_iter().map(|s| (s.name, s.args)).collect(),
        }
    }
}

struct IObjectPath<'a> {
    conn: &'a Connection,
    path: String,
    registered: Cell<bool>,
    interfaces: RefCell<BTreeMap<String, Interface<'a>>>,
}

pub struct ObjectPath<'a> {
    // We need a weak ref for the introspector, hence this extra boxing
    i: Rc<IObjectPath<'a>>,
}

#[unsafe_destructor]
impl<'a> Drop for IObjectPath<'a> {
    fn drop(&mut self) {
        let _ = self.set_registered(false);
    }
}

fn introspect_args(args: &Vec<Argument>, indent: &str, dir: &str) -> String {
    args.iter().fold("".to_string(), |aa, az| {
        format!("{}{}<arg name=\"{}\" type=\"{}\"{}/>\n", aa, indent, az.name, az.sig, dir)
    })
}

fn introspect_map<T, C: Fn(&T) -> (String, String)>
    (h: &BTreeMap<String, T>, name: &str, indent: &str, func: C) -> String {

    h.iter().fold("".to_string(), |a, (k, v)| {
        let (params, contents) = func(v);
        format!("{}{}<{} name=\"{}\"{}{}>\n",
            a, indent, name, k, params, if contents.len() > 0 {
                format!(">\n{}{}</{}", contents, indent, name)
            }
            else { format!("/") }
        )
    })
}

impl<'a> IObjectPath<'a> {

    fn set_registered(&self, register: bool) -> Result<(), Error> {
        if register == self.registered.get() { return Ok(()) };
        if register {
            try!(self.conn.register_object_path(&*self.path));
        } else {
            self.conn.unregister_object_path(&*self.path);
        }
        self.registered.set(register);
        Ok(())
    }

    fn introspect(&self, _: &mut Message) -> MethodResult {
        let ifacestr = introspect_map(&*self.interfaces.borrow(), "interface", "  ", |iv|
            (format!(""), format!("{}{}{}",
                introspect_map(&iv.methods, "method", "    ", |m| (format!(""), format!("{}{}",
                    introspect_args(&m.in_args, "      ", " direction=\"in\""),
                    introspect_args(&m.out_args, "      ", " direction=\"out\"")
                ))),
                introspect_map(&iv.properties, "property", "    ", |p| (
                    format!(" type=\"{}\" access=\"{}\"", p.sig, match p.access {
                        PropertyAccess::RO(_) => "read",
                        PropertyAccess::RW(_) => "readwrite",
                        PropertyAccess::WO(_) => "write",
                    }), format!("")
                )),
                introspect_map(&iv.signals, "signal", "    ", |s| (format!(""),
                    introspect_args(s, "      ", "")
                ))
            ))
        );
        let childstr = self.conn.list_registered_object_paths(&*self.path).iter().fold("".to_string(), |na, n|
            format!(r##"{}  <node name="{}"/>
"##, na, n)
        );
        let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, self.path, ifacestr, childstr);

        Ok(vec!(MessageItem::Str(nodestr)))
    }

    fn property_get(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));
        let prop_name = try!(parse_msg_str(items.get(1)));

        let is = self.interfaces.borrow();
        let i = if let Some(s) = is.get(iface_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", iface_name)))
        };
        let p = if let Some(s) = i.properties.get(prop_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownProperty", format!("Unknown property {}", prop_name)))
        };
        let v = try!(match p.access {
            PropertyAccess::RO(ref cb) => cb.get(),
            PropertyAccess::RW(ref cb) => cb.get(),
            PropertyAccess::WO(_) => {
                return Err(("org.freedesktop.DBus.Error.Failed", format!("Property {} is write only", prop_name)))
            }
        });
        Ok(vec!(MessageItem::Variant(Box::new(v))))
    }

    fn property_getall(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));

        let is = self.interfaces.borrow();
        let i = if let Some(s) = is.get(iface_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", iface_name)))
        };
        let mut result = Vec::new();
        result.push(try!(MessageItem::from_dict(i.properties.iter().filter_map(|(pname, pv)| {
            let v = match pv.access {
                PropertyAccess::RO(ref cb) => cb.get(),
                PropertyAccess::RW(ref cb) => cb.get(),
                PropertyAccess::WO(_) => { return None }
            };
            Some(v.map(|vv| (pname.clone(),vv)))
        }))));
        Ok(result)
    }

    fn property_set(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));
        let prop_name = try!(parse_msg_str(items.get(1)));
        let value = try!(parse_msg_variant(items.get(2)));

        let is = self.interfaces.borrow();
        let i = if let Some(s) = is.get(iface_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", iface_name)))
        };
        let p = if let Some(s) = i.properties.get(prop_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownProperty", format!("Unknown property {}", prop_name)))
        };
        try!(match p.access {
            PropertyAccess::WO(ref cb) => cb.set(value),
            PropertyAccess::RW(ref cb) => cb.set(value),
            PropertyAccess::RO(_) => {
                return Err(("org.freedesktop.DBus.Error.PropertyReadOnly", format!("Property {} is read only", prop_name)))
            }
        });
        Ok(vec!())
    }
}

fn parse_msg_str(a: Option<&MessageItem>) -> Result<&str,(&'static str, String)> {
    let name = if let Some(s) = a { s } else {
        return Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a)))
    };
    if let &MessageItem::Str(ref s) = name {
        Ok(&**s)
    } else { Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a))) }
}

fn parse_msg_variant(a: Option<&MessageItem>) -> Result<&MessageItem,(&'static str, String)> {
    let name = if let Some(s) = a { s } else {
        return Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a)))
    };
    if let &MessageItem::Variant(ref s) = name {
        Ok(&**s)
    } else { Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a))) }
}

impl PropertyGetHandler for MessageItem {
    fn get(&self) -> PropertyGetResult {
        Ok(self.clone())
    }
}

impl<'a> ObjectPath<'a> {
    pub fn new(conn: &'a Connection, path: &str, introspectable: bool) -> ObjectPath<'a> {
        let i = IObjectPath {
            conn: conn,
            path: path.to_string(),
            registered: Cell::new(false),
            interfaces: RefCell::new(BTreeMap::new()),
        };
        let mut o = ObjectPath { i: Rc::new(i) };

        if introspectable {
            let o_weak = o.i.downgrade();
            let i = Interface::new(vec!(
                Method::new("Introspect", vec!(), vec!(Argument::new("xml_data", "s")),
                    Box::new(move |m| { o_weak.upgrade().unwrap().introspect(m) }))), vec!(), vec!());
            o.insert_interface("org.freedesktop.DBus.Introspectable", i);
        }
        o
    }

    fn add_property_handler(&mut self) {
        if self.i.interfaces.borrow().contains_key("org.freedesktop.DBus.Properties") { return };
        let (weak1, weak2, weak3) = (self.i.downgrade(), self.i.downgrade(), self.i.downgrade());
        let i = Interface::new(vec!(
            Method::new("Get",
                vec!(Argument::new("interface_name", "s"), Argument::new("property_name", "s")),
                vec!(Argument::new("value", "v")),
                Box::new(move |m| weak1.upgrade().unwrap().property_get(m))),
            Method::new("GetAll",
                vec!(Argument::new("interface_name", "s")),
                vec!(Argument::new("props", "a{sv}")),
                Box::new(move |m| weak2.upgrade().unwrap().property_getall(m))),
            Method::new("Set",
                vec!(Argument::new("interface_name", "s"), Argument::new("property_name", "s"),
                    Argument::new("value", "v")),
                vec!(),
                Box::new(move |m| weak3.upgrade().unwrap().property_set(m)))),
            vec!(), vec!());
        self.insert_interface("org.freedesktop.DBus.Properties", i);
    }

    pub fn insert_interface<N: ToString>(&mut self, name: N, i: Interface<'a>) {
        if !i.properties.is_empty() {
            self.add_property_handler();
        }
        self.i.interfaces.borrow_mut().insert(name.to_string(), i);
    }

    pub fn is_registered(&self) -> bool {
        self.i.registered.get()
    }

    pub fn set_registered(&mut self, register: bool) -> Result<(), Error> {
        self.i.set_registered(register)
    }

    /* Return value:
       None => not handled,
       Some(Err(())) => message reply send failed,
       Some(Ok()) => message reply send ok */

    pub fn handle_message(&mut self, msg: &mut Message) -> Option<Result<(), ()>> {
        let (_, path, iface, method) = msg.headers();
        if path.is_none() || path.unwrap() != self.i.path { return None; }
        if iface.is_none() { return None; }

        let method = {
            // This is because we don't want to hold the refcell lock when we call the
            // callback - maximum flexibility for clients.
            if let Some(i) = self.i.interfaces.borrow().get(&iface.unwrap()) {
                if let Some(Some(m)) = method.map(|m| i.methods.get(&m)) {
                    m.cb.clone()
                } else {
                    return Some(self.i.conn.send(Message::new_error(
                        msg, "org.freedesktop.DBus.Error.UnknownMethod", "Unknown method").unwrap()));
                }
            } else {
                return Some(self.i.conn.send(Message::new_error(msg,
                    "org.freedesktop.DBus.Error.UnknownInterface", "Unknown interface").unwrap()));
            }
        };

        let reply = match (*method)(msg) {
            Ok(r) => {
                let mut z = Message::new_method_return(msg).unwrap();
                z.append_items(&*r);
                z
            },
            Err((aa,bb)) => Message::new_error(msg, aa, &*bb).unwrap(),
        };

        Some(self.i.conn.send(reply))
    }
}

#[cfg(test)]
fn make_objpath<'a>(c: &'a Connection) -> ObjectPath<'a> {
    let mut o = ObjectPath::new(c, "/echo", true);
    o.insert_interface("com.example.echo", Interface::new(
        vec!(Method::new("Echo",
            vec!(Argument::new("request", "s")),
            vec!(Argument::new("reply", "s")), Box::new(|_| { Err(("dummy", "dummy".to_string())) } ))),
        vec!(Property::new_ro("EchoCount", MessageItem::Int32(7).type_sig(), Box::new(MessageItem::Int32(7)))),
        vec!(Signal::new("Echoed", vec!(Argument::new("data", "s"))))));
    o
}

#[test]
fn test_objpath() {
    let c = Connection::get_private(super::BusType::Session).unwrap();
    let mut o = make_objpath(&c);
    o.set_registered(true).unwrap();
    let busname = format!("com.example.objpath.test.test_objpath");
    assert_eq!(c.register_name(&*busname, super::NameFlag::ReplaceExisting as u32).unwrap(), super::RequestNameReply::PrimaryOwner);

    let thread = ::std::thread::Thread::scoped(move || {
        let c = Connection::get_private(super::BusType::Session).unwrap();
        let pr = super::Props::new(&c, &*busname, "/echo", "com.example.echo", 5000);
        assert_eq!(pr.get("EchoCount").unwrap(), super::MessageItem::Int32(7));
        let m = pr.get_all().unwrap();
        assert_eq!(m.get("EchoCount").unwrap(), &super::MessageItem::Int32(7));
    });

    let mut i = 0;
    for n in c.iter(1000) {
        println!("objpath msg {:?}", n);
        if let super::ConnectionItem::MethodCall(mut m) = n {
            if let Some(msg) = o.handle_message(&mut m) {
                msg.unwrap();
                i += 1;
                if i >= 2 { break };
            }
        }
    }

    thread.join();
}

#[test]
fn test_introspect() {
    let c = Connection::get_private(super::BusType::Session).unwrap();
    let mut o = make_objpath(&c);
    o.set_registered(true).unwrap();
    let mut o2 = ObjectPath::new(&c, "/echo/subpath", true);
    o2.set_registered(true).unwrap();
    let mut msg = Message::new_method_call("com.example.echoserver", "/echo", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
    println!("Introspect result: {}", parse_msg_str(o.i.introspect(&mut msg).unwrap().get(0)).unwrap());

    let result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
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
  <node name="subpath"/>
</node>"##;

    assert_eq!(result, parse_msg_str(o.i.introspect(&mut msg).unwrap().get(0)).unwrap());

}

