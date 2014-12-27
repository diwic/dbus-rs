use super::{Connection, Message, MessageItem, Error};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::cell::{Cell, RefCell};

pub struct Argument<'a> {
    name: &'a str,
    sig: &'a str,
}

pub type MethodResult<'a> = Result<Vec<MessageItem>, (&'a str, String)>;

pub trait MethodHandler<'a> {
    fn handle(&self, &mut Message) -> MethodResult;
}

pub struct Method<'a> {
    in_args: Vec<Argument<'a>>,
    out_args: Vec<Argument<'a>>,
    cb: Box<MethodHandler<'a>+'a>,
}

pub struct Interface<'a> {
    methods: BTreeMap<String, Method<'a>>,
//  TODO: properties and signals
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
{}  </interface>
"##,
                ia, ik, iv.methods.iter().fold("".to_string(), |ma, (mk, mv)| {
                format!(r##"{}    <method name="{}">
{}{}    </method>
"##, ma, mk,
                    mv.in_args.iter().fold("".to_string(), |aa, az| {
                       format!(r##"{}     <arg name="{}" type="{}" direction="in"/>
"##, aa, az.name, az.sig)
                    }), mv.out_args.iter().fold("".to_string(), |aa, az| {
                       format!(r##"{}     <arg name="{}" type="{}" direction="out"/>
"##, aa, az.name, az.sig)
                }))
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

impl<'a> ObjectPath<'a> {
    pub fn new(conn: &'a Connection, path: &str, introspectable: bool) -> ObjectPath<'a> {
        let i = IObjectPath {
            conn: conn,
            path: path.to_string(),
            registered: Cell::new(false),
            interfaces: RefCell::new(BTreeMap::new()),
        };
        let o = ObjectPath { i: Rc::new(i) };

        if introspectable {
            let mut m = BTreeMap::new();
            m.insert("Introspect".to_string(), Method {
                in_args: vec!(),
                out_args: vec!(Argument { name: "xml_data", sig: "s" }),
                cb: box Introspecter { objpath: o.i.downgrade() },
            });
            o.i.interfaces.borrow_mut().insert("org.freedesktop.DBus.Introspectable".to_string(), Interface { methods: m });
        };
        o
    }

    // Note: This function can not be called from inside a MethodHandler callback.
    pub fn insert_interface(&mut self, name: String, i: Interface<'a>) {
        self.i.interfaces.borrow_mut().insert(name, i);
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
        let reply = 
            if let Some(i) = self.i.interfaces.borrow().get(&iface.unwrap()) {
                if let Some(Some(m)) = method.map(|m| i.methods.get(&m)) {
                    match m.cb.handle(msg) {
                        Ok(r) => {
                            let mut z = Message::new_method_return(msg).unwrap();
                            z.append_items(r.as_slice());
                            z
                        }
                        Err((aa,bb)) => Message::new_error(msg, aa, bb.as_slice()).unwrap(),
                    }
                }            
                else { Message::new_error(msg, "org.freedesktop.DBus.Error.UnknownMethod", "Unkown method").unwrap() }
            }
            else { Message::new_error(msg, "org.freedesktop.DBus.Error.UnknownInterface", "Unkown interface").unwrap() }
        ;
        Some(self.i.conn.send(reply))
    }
}

#[cfg(test)]
impl<'a> MethodHandler<'a> for int {
    fn handle(&self, _: &mut Message) -> MethodResult {
        Err(("dummy", "dummy".to_string()))
    }
}

#[test]
fn test_introspect() {
    let mut im = BTreeMap::new();
    im.insert("Echo".to_string(), Method {
        in_args: vec!( Argument { name: "request", sig: "s"} ),  
        out_args: vec!( Argument { name: "reply", sig: "s"} ),
        cb: box 3i,
    });

    let c = Connection::get_private(super::BusType::Session).unwrap();
    let mut o = ObjectPath::new(&c, "/echo", true);
    o.insert_interface("com.example.echo".to_string(), Interface { methods: im });
    let mut msg = Message::new_method_call("com.example.echoserver", "/echo", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();

    println!("Introspect result: {}", o.i.introspect(&mut msg));

    let result = r##"[Str(<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/echo">
  <interface name="com.example.echo">
    <method name="Echo">
     <arg name="request" type="s" direction="in"/>
     <arg name="reply" type="s" direction="out"/>
    </method>
  </interface>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
     <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
</node>)]"##;

    assert_eq!(result, format!("{}", o.i.introspect(&mut msg).unwrap()));

}

