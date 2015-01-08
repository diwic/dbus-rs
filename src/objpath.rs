use super::{Connection, Message, MessageItem, Error};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::cell::{Cell, RefCell};

pub struct Argument<'a> {
    name: &'a str,
    sig: &'a str,
}

pub type MethodResult<'a> = Result<Vec<MessageItem>, (&'a str, String)>;
pub type PropertyGetResult = Result<MessageItem, (&'static str, String)>;
pub type PropertySetResult = Result<(), (&'static str, String)>;

pub trait MethodHandler<'a> {
    fn handle(&self, &mut Message) -> MethodResult;
}

struct IMethod<'a> {
    in_args: Vec<Argument<'a>>,
    out_args: Vec<Argument<'a>>,
    cb: Rc<Box<MethodHandler<'a>+'a>>,
}

pub struct Method<'a> {
    name: String,
    i: IMethod<'a>
}

impl<'a> Method<'a> {
    pub fn new<N: ToString>(name: N, in_args: Vec<Argument<'a>>,
            out_args: Vec<Argument<'a>>, cb: Box<MethodHandler<'a>+'a>) -> Method<'a> {
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
    sig: &'a str,
    access: PropertyAccess<'a>,
}

pub struct Property<'a> {
    name: String,
    i: IProperty<'a>
}

impl<'a> Property<'a> {
    fn new<N: ToString>(name: N, sig: &'a str, a: PropertyAccess<'a>) -> Property<'a> {
        Property { name: name.to_string(), i: IProperty { sig: sig, access: a } }
    }
    pub fn new_ro<N: ToString>(name: N, sig: &'a str, h: Box<PropertyGetHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::RO(h))
    }
    pub fn new_rw<N: ToString>(name: N, sig: &'a str, h: Box<PropertyHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::RW(h))
    }
    pub fn new_wo<N: ToString>(name: N, sig: &'a str, h: Box<PropertySetHandler+'a>) -> Property<'a> {
        Property::new(name, sig, PropertyAccess::WO(h))
    }
}

pub struct Interface<'a> {
    methods: BTreeMap<String, IMethod<'a>>,
    properties: BTreeMap<String, IProperty<'a>>,
//  TODO: signals
}

impl<'a> Interface<'a> {
    pub fn new(m: Vec<Method<'a>>, p: Vec<Property<'a>>) -> Interface<'a> {
        Interface {
           methods: m.into_iter().map(|m| (m.name, m.i)).collect(),
           properties: p.into_iter().map(|p| (p.name, p.i)).collect() }
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

    fn introspect<'b>(&self, _: &mut Message) -> MethodResult<'b> {
        let ifacestr = self.interfaces.borrow().iter().fold("".to_string(), |ia, (ik, iv)| {
            format!(r##"{}  <interface name="{}">
{}{}  </interface>
"##,
                ia, ik, iv.methods.iter().fold("".to_string(), |ma, (mk, mv)| {
                format!(r##"{}    <method name="{}">
{}{}    </method>
"##, ma, mk,
                    mv.in_args.iter().fold("".to_string(), |aa, az| {
                       format!(r##"{}      <arg name="{}" type="{}" direction="in"/>
"##, aa, az.name, az.sig)
                    }), mv.out_args.iter().fold("".to_string(), |aa, az| {
                       format!(r##"{}      <arg name="{}" type="{}" direction="out"/>
"##, aa, az.name, az.sig)
                }))
            }), iv.properties.iter().fold("".to_string(), |pa, (pk, pv)| {
                       format!(r##"{}    <property name="{}" type="{}" access="{}"/>
"##, pa, pk, pv.sig, match pv.access {
                PropertyAccess::RO(_) => "read",
                PropertyAccess::RW(_) => "readwrite",
                PropertyAccess::WO(_) => "write",
            })
            }))
        });

        let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}</node>"##, self.path, ifacestr);

        Ok(vec!(MessageItem::Str(nodestr)))
    }
}

struct Introspecter<'a> {
    objpath: Weak<IObjectPath<'a>>,
}

impl<'a> MethodHandler<'a> for Introspecter<'a> {
    fn handle(&self, m: &mut Message) -> MethodResult {
        self.objpath.upgrade().unwrap().introspect(m)
    }
}

fn parse_msg_str(a: Option<&MessageItem>) -> Result<&str,(&'static str, String)> {
    let name = if let Some(s) = a { s } else {
        return Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {}", a)))
    };
    if let &MessageItem::Str(ref s) = name {
        Ok(s.as_slice())
    } else { Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {}", a))) }
}

fn parse_msg_variant(a: Option<&MessageItem>) -> Result<&MessageItem,(&'static str, String)> {
    let name = if let Some(s) = a { s } else {
        return Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {}", a)))
    };
    if let &MessageItem::Variant(ref s) = name {
        Ok(&**s)
    } else { Err(("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {}", a))) }
}

impl PropertyGetHandler for MessageItem {
    fn get(&self) -> PropertyGetResult {
        Ok(self.clone())
    }
}

struct PropertyGet<'a> {
    objpath: Weak<IObjectPath<'a>>,
}

impl<'a> MethodHandler<'a> for PropertyGet<'a> {
    fn handle(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));
        let prop_name = try!(parse_msg_str(items.get(1)));

        let istmp = self.objpath.upgrade().unwrap();
        let is = istmp.interfaces.borrow();
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
        Ok(vec!(MessageItem::Variant(box v)))
    }
}

struct PropertyGetAll<'a> {
    objpath: Weak<IObjectPath<'a>>,
}

impl<'a> MethodHandler<'a> for PropertyGetAll<'a> {
    fn handle(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));

        let istmp = self.objpath.upgrade().unwrap();
        let is = istmp.interfaces.borrow();
        let i = if let Some(s) = is.get(iface_name) { s } else {
            return Err(("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", iface_name)))
        };
        let mut result = Vec::new();
        for (pname, pv) in i.properties.iter() {
            let v = try!(match pv.access {
                PropertyAccess::RO(ref cb) => cb.get(),
                PropertyAccess::RW(ref cb) => cb.get(),
                PropertyAccess::WO(_) => { continue }
            });
            result.push(MessageItem::DictEntry(box MessageItem::Str(pname.clone()), box v));
        }
        Ok(result)
    }
}

struct PropertySet<'a> {
    objpath: Weak<IObjectPath<'a>>,
}

impl<'a> MethodHandler<'a> for PropertySet<'a> {
    fn handle(&self, msg: &mut Message) -> MethodResult {
        let items = msg.get_items();
        let iface_name = try!(parse_msg_str(items.get(0)));
        let prop_name = try!(parse_msg_str(items.get(1)));
        let value = try!(parse_msg_variant(items.get(2)));

        let istmp = self.objpath.upgrade().unwrap();
        let is = istmp.interfaces.borrow();
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
            let i = Interface::new(vec!(
                Method::new("Introspect", vec!(), vec!(Argument { name: "xml_data", sig: "s" }),
                    box Introspecter { objpath: o.i.downgrade() })), vec!());
            o.insert_interface("org.freedesktop.DBus.Introspectable", i);
        }
        o
    }

    fn add_property_handler(&mut self) {
        if self.i.interfaces.borrow().contains_key("org.freedesktop.DBus.Properties") { return };
        let i = Interface::new(vec!(
            Method::new("Get",
                vec!(Argument { name: "interface_name", sig: "s" }, Argument { name: "property_name", sig: "s" }),
                vec!(Argument { name: "value", sig: "v" }),
                box PropertyGet { objpath: self.i.downgrade() }),
            Method::new("GetAll",
                vec!(Argument { name: "interface_name", sig: "s" }),
                vec!(Argument { name: "props", sig: "a{sv}" }),
                box PropertyGetAll { objpath: self.i.downgrade() }),
            Method::new("Set",
                vec!(Argument { name: "interface_name", sig: "s" }, Argument { name: "property_name", sig: "s" },
                    Argument { name: "value", sig: "v" }),
                vec!(),
                box PropertySet { objpath: self.i.downgrade() })),
            vec!());
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

        let reply = match method.handle(msg) {
            Ok(r) => {
                let mut z = Message::new_method_return(msg).unwrap();
                z.append_items(r.as_slice());
                z
            },
            Err((aa,bb)) => Message::new_error(msg, aa, bb.as_slice()).unwrap(),
        };

        Some(self.i.conn.send(reply))
    }
}

#[cfg(test)]
impl<'a> MethodHandler<'a> for int {
    fn handle(&self, _: &mut Message) -> MethodResult {
        Err(("dummy", "dummy".to_string()))
    }
}

#[cfg(test)]
fn make_objpath<'a>(c: &'a Connection) -> ObjectPath<'a> {
    let mut o = ObjectPath::new(c, "/echo", true);
    o.insert_interface("com.example.echo", Interface::new(
        vec!(Method::new("Echo",
            vec!( Argument { name: "request", sig: "s"} ),
            vec!( Argument { name: "reply", sig: "s"} ), box 3i)),
        vec!(Property::new_ro("EchoCount", "i", box MessageItem::Int32(7)))));
    o
}

#[test]
fn test_objpath() {
    let c = Connection::get_private(super::BusType::Session).unwrap();
    let mut o = make_objpath(&c);
    o.set_registered(true).unwrap();
    let busname = format!("com.example.objpath.test{}", ::std::rand::random::<u32>());
    assert_eq!(c.register_name(busname.as_slice(), super::NameFlag::ReplaceExisting as u32).unwrap(), super::RequestNameReply::PrimaryOwner);

    let thread = ::std::thread::Thread::spawn(move || {
        let c = Connection::get_private(super::BusType::Session).unwrap();
        let pr = super::Props::new(&c, &*busname, "/echo", "com.example.echo", 5000);
        assert_eq!(pr.get("EchoCount").unwrap(), super::MessageItem::Int32(7));
    });

    for n in c.iter(1000) {
        println!("objpath msg {}", n);
        if let super::ConnectionItem::MethodCall(mut m) = n {
            if let Some(msg) = o.handle_message(&mut m) {
                msg.unwrap();
                break;
            }
        }
    }

    thread.join().ok().expect("failed to join thread");
}

#[test]
fn test_introspect() {
    let c = Connection::get_private(super::BusType::Session).unwrap();
    let o = make_objpath(&c);
    let mut msg = Message::new_method_call("com.example.echoserver", "/echo", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();

    println!("Introspect result: {}", o.i.introspect(&mut msg));

    let result = r##"[Str(<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/echo">
  <interface name="com.example.echo">
    <method name="Echo">
      <arg name="request" type="s" direction="in"/>
      <arg name="reply" type="s" direction="out"/>
    </method>
    <property name="EchoCount" type="i" access="read"/>
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
</node>)]"##;

    assert_eq!(result, format!("{}", o.i.introspect(&mut msg).unwrap()));

}

