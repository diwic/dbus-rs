use super::utils::{ArcMap, Iter, IterE, Annotations, Introspect};
use super::{Factory, MethodType, MethodInfo, MethodResult, MethodErr, DataType, Property, Method, Signal, methodtype};
use std::sync::{Arc, Mutex};
use dbus::{Message, MessageType, Error, arg, message, channel};
use dbus::strings::{Member, Path, Signature, Interface as IfaceName};
use dbus::ffidisp::{ConnectionItem, MsgHandler, Connection, MsgHandlerType, MsgHandlerResult};
use std::fmt;
use super::leaves::prop_append_dict;
use dbus::channel::Channel;
use std::time::Duration;

fn introspect_map<I: fmt::Display, T: Introspect>
    (h: &ArcMap<I, T>, indent: &str) -> String {

    h.iter().fold("".into(), |a, (k, v)| {
        let (name, params, contents) = (v.xml_name(), v.xml_params(), v.xml_contents());
        format!("{}{}<{} name=\"{}\"{}{}>\n",
            a, indent, name, &*k, params, if !contents.is_empty() {
                format!(">\n{}{}</{}", contents, indent, name)
            }
            else { "/".to_string() }
        )
    })
}

#[derive(Debug)]
/// Represents a D-Bus interface.
pub struct Interface<M: MethodType<D>, D: DataType> {
    name: Arc<IfaceName<'static>>,
    methods: ArcMap<Member<'static>, Method<M, D>>,
    signals: ArcMap<Member<'static>, Signal<D>>,
    properties: ArcMap<String, Property<M, D>>,
    anns: Annotations,
    data: D::Interface,
}

impl<M: MethodType<D>, D: DataType> Interface<M, D> {
    /// Builder function that adds a method to the interface.
    pub fn add_m<I: Into<Arc<Method<M, D>>>>(mut self, m: I) -> Self {
        let m = m.into();
        self.methods.insert(m.get_name().clone(), m);
        self
    }

    /// Builder function that adds a signal to the interface.
    pub fn add_s<I: Into<Arc<Signal<D>>>>(mut self, s: I) -> Self {
        let m = s.into();
        self.signals.insert(m.get_name().clone(), m);
        self
    }

    /// Builder function that adds a property to the interface.
    pub fn add_p<I: Into<Arc<Property<M, D>>>>(mut self, p: I) -> Self {
        let m = p.into();
        self.properties.insert(m.get_name().to_owned(), m);
        self
    }

    /// Builder function that adds an annotation to this interface.
    pub fn annotate<N: Into<String>, V: Into<String>>(mut self, name: N, value: V) -> Self {
        self.anns.insert(name, value); self
    }

    /// Builder function that adds an annotation that this entity is deprecated.
    pub fn deprecated(self) -> Self { self.annotate("org.freedesktop.DBus.Deprecated", "true") }

    /// Get interface name
    pub fn get_name(&self) -> &IfaceName<'static> { &self.name }

    /// Get associated data
    pub fn get_data(&self) -> &D::Interface { &self.data }

    /// Iterates over methods implemented by this interface.
    pub fn iter_m<'a>(&'a self) -> Iter<'a, Method<M, D>> { IterE::Member(self.methods.values()).into() }

    /// Iterates over signals implemented by this interface.
    pub fn iter_s<'a>(&'a self) -> Iter<'a, Signal<D>> { IterE::Member(self.signals.values()).into() }

    /// Iterates over properties implemented by this interface.
    pub fn iter_p<'a>(&'a self) -> Iter<'a, Property<M, D>> { IterE::String(self.properties.values()).into() }
}

impl<M: MethodType<D>, D: DataType> Introspect for Interface<M, D> {
    fn xml_name(&self) -> &'static str { "interface" }
    fn xml_params(&self) -> String { String::new() }
    fn xml_contents(&self) -> String {
        format!("{}{}{}{}",
            introspect_map(&self.methods, "    "),
            introspect_map(&self.properties, "    "),
            introspect_map(&self.signals, "    "),
            self.anns.introspect("    "))
    }
}


pub fn new_interface<M: MethodType<D>, D: DataType>(t: IfaceName<'static>, d: D::Interface) -> Interface<M, D> {
    Interface { name: Arc::new(t), methods: ArcMap::new(), signals: ArcMap::new(),
        properties: ArcMap::new(), anns: Annotations::new(), data: d
    }
}


#[derive(Debug)]
/// Cache of built-in interfaces, in order to save memory when many object paths implement the same interface(s).
pub struct IfaceCache<M: MethodType<D>, D: DataType>(Mutex<ArcMap<IfaceName<'static>, Interface<M, D>>>);

impl<M: MethodType<D>, D: DataType> IfaceCache<M, D>
where D::Interface: Default {
    pub fn get<S: Into<IfaceName<'static>> + Clone, F>(&self, s: S, f: F) -> Arc<Interface<M, D>>
        where F: FnOnce(Interface<M, D>) -> Interface<M, D> {
        let s2 = s.clone().into();
        let mut m = self.0.lock().unwrap();
        m.entry(s2).or_insert_with(|| {
            let i = new_interface(s.into(), Default::default());
            Arc::new(f(i))
        }).clone()
    }
}

impl<M: MethodType<D>, D: DataType> IfaceCache<M, D> {
    pub fn get_factory<S: Into<IfaceName<'static>> + Clone, F>(&self, s: S, f: F) -> Arc<Interface<M, D>>
        where F: FnOnce() -> Interface<M, D> {
        let s2 = s.clone().into();
        let mut m = self.0.lock().unwrap();
        m.entry(s2).or_insert_with(|| {
            Arc::new(f())
        }).clone()
    }


    pub fn new() -> Arc<Self> { Arc::new(IfaceCache(Mutex::new(ArcMap::new()))) }
}

#[derive(Debug)]
/// A D-Bus Object Path.
pub struct ObjectPath<M: MethodType<D>, D: DataType> {
    name: Arc<Path<'static>>,
    default_iface: Option<IfaceName<'static>>,
    ifaces: ArcMap<Arc<IfaceName<'static>>, Interface<M, D>>,
    ifacecache: Arc<IfaceCache<M, D>>,
    data: D::ObjectPath,
}

impl<M: MethodType<D>, D: DataType> ObjectPath<M, D> {

    /// Get property name
    pub fn get_name(&self) -> &Path<'static> { &self.name }

    /// Get associated data
    pub fn get_data(&self) -> &D::ObjectPath { &self.data }

    /// Iterates over interfaces implemented by this object path.
    pub fn iter<'a>(&'a self) -> Iter<'a, Interface<M, D>> { IterE::Iface(self.ifaces.values()).into() }

    pub(super) fn introspect(&self, tree: &Tree<M, D>) -> String {
        let ifacestr = introspect_map(&self.ifaces, "  ");
        let olen = if &**self.name == "/" { 1 } else { self.name.len()+1 };
        let childstr = tree.children(self, true).iter().fold("".to_string(), |na, n|
            format!("{}  <node name=\"{}\"/>\n", na, &n.name[olen..])
        );

        let nodestr = format!(r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, self.name, ifacestr, childstr);
        nodestr
    }

    fn get_iface<'a>(&'a self, iface_name: &'a str) -> Result<&Arc<Interface<M, D>>, MethodErr> {
        let j = IfaceName::from_slice(iface_name).map_err(|e| MethodErr::invalid_arg(&e))?;
        self.ifaces.get(&j).ok_or_else(|| MethodErr::no_interface(&j))
    }

    fn prop_get(&self, m: &MethodInfo<M, D>) -> MethodResult {
        let (iname, prop_name): (&str, &str) = m.msg.read2()?;
        let iface = self.get_iface(iname)?;
        let prop: &Property<M, D> = iface.properties.get(&String::from(prop_name))
            .ok_or_else(|| MethodErr::no_property(&prop_name))?;
        prop.can_get()?;
        let mut mret = m.msg.method_return();
        {
            let mut iter = arg::IterAppend::new(&mut mret);
            let pinfo = m.to_prop_info(iface, prop);
            prop.get_as_variant(&mut iter, &pinfo)?;
        }
        Ok(vec!(mret))
    }

    fn prop_get_all(&self, m: &MethodInfo<M, D>) -> MethodResult {
        let iface = self.get_iface(m.msg.read1()?)?;
        let mut mret = m.msg.method_return();
        prop_append_dict(&mut arg::IterAppend::new(&mut mret),
            iface.properties.values().map(|v| &**v), m)?;
        Ok(vec!(mret))
    }


    fn prop_set(&self, m: &MethodInfo<M, D>) -> MethodResult {
        let (iname, prop_name): (&str, &str) = m.msg.read2()?;
        let iface = self.get_iface(iname)?;
        let prop: &Property<M, D> = iface.properties.get(&String::from(prop_name))
            .ok_or_else(|| MethodErr::no_property(&prop_name))?;

        let mut iter = arg::Iter::new(m.msg);
        iter.next(); iter.next();
        let mut iter2 = iter;
        prop.can_set(Some(iter))?;

        let pinfo = m.to_prop_info(iface, prop);
        let mut r: Vec<Message> = prop.set_as_variant(&mut iter2, &pinfo)?.into_iter().collect();
        r.push(m.msg.method_return());
        Ok(r)

    }

    fn get_managed_objects(&self, m: &MethodInfo<M, D>) -> MethodResult {
        use dbus::arg::{Dict, Variant};
        let paths = m.tree.children(&self, false);
        let mut result = Ok(());
        let mut r = m.msg.method_return();
        {
            let mut i = arg::IterAppend::new(&mut r);
            i.append_dict(&Signature::make::<Path>(), &Signature::make::<Dict<&str,Dict<&str,Variant<()>,()>,()>>(), |ii| {
                for p in paths {
                    ii.append_dict_entry(|pi| {
                        pi.append(&*p.name);
                        pi.append_dict(&Signature::make::<&str>(), &Signature::make::<Dict<&str,Variant<()>,()>>(), |pii| {
                            for ifaces in p.ifaces.values() {
                                let m2 = MethodInfo { msg: m.msg, path: p, iface: ifaces, tree: m.tree, method: m.method };
                                pii.append_dict_entry(|ppii| {
                                    ppii.append(&**ifaces.name);
                                    result = prop_append_dict(ppii, ifaces.properties.values().map(|v| &**v), &m2);
                                });
                                if result.is_err() { break; }
                            }
                        });
                    });
                    if result.is_err() { break; }
                }
            });
        }
        result?;
        Ok(vec!(r))
    }

    fn handle(&self, m: &Message, t: &Tree<M, D>) -> MethodResult {
        let iname = m.interface().or_else(|| { self.default_iface.clone() });
        let i = iname.and_then(|i| self.ifaces.get(&i)).ok_or_else(|| MethodErr::no_interface(&""))?;
        let me = m.member().and_then(|me| i.methods.get(&me)).ok_or_else(|| MethodErr::no_method(&""))?;
        let minfo = MethodInfo { msg: m, tree: t, path: self, iface: i, method: me };
        me.call(&minfo)
    }

}

impl<M: MethodType<D>, D: DataType> ObjectPath<M, D>
where <D as DataType>::Interface: Default,
      <D as DataType>::Method: Default,
      <D as DataType>::Signal: Default
{
    /// Adds introspection support for this object path.
    pub fn introspectable(self) -> Self {
        let z = self.ifacecache.get_factory("org.freedesktop.DBus.Introspectable", || {
            let f = Factory::from(self.ifacecache.clone());
            methodtype::org_freedesktop_dbus_introspectable_server(&f, Default::default())
        });
        self.add(z)
    }

    /// Builder function that adds a interface to the object path.
    pub fn add<I: Into<Arc<Interface<M, D>>>>(mut self, s: I) -> Self {
        let m = s.into();
        if !m.properties.is_empty() { self.add_property_handler(); }
        self.ifaces.insert(m.name.clone(), m);
        self
    }

    /// Builder function that sets what interface should be dispatched on an incoming
    /// method call without interface.
    pub fn default_interface(mut self, i: IfaceName<'static>) -> Self {
        self.default_iface = Some(i);
        self
    }

    /// Adds ObjectManager support for this object path.
    ///
    /// It is not possible to add/remove interfaces while the object path belongs to a tree,
    /// hence no InterfacesAdded / InterfacesRemoved signals are sent.
    pub fn object_manager(mut self) -> Self {
        use dbus::arg::{Variant, Dict};
        let ifname = IfaceName::from("org.freedesktop.DBus.ObjectManager");
        if self.ifaces.contains_key(&ifname) { return self };
        let z = self.ifacecache.get(ifname, |i| {
            i.add_m(super::leaves::new_method("GetManagedObjects".into(), Default::default(),
                M::make_method(|m| m.path.get_managed_objects(m)))
                .outarg::<Dict<Path,Dict<&str,Dict<&str,Variant<()>,()>,()>,()>,_>("objpath_interfaces_and_properties"))
        });
        self.ifaces.insert(z.name.clone(), z);
        self
    }

    fn add_property_handler(&mut self) {
        use dbus::arg::{Variant, Dict};
        let ifname = IfaceName::from("org.freedesktop.DBus.Properties");
        if self.ifaces.contains_key(&ifname) { return };
        let z = self.ifacecache.get(ifname, |i| {
            i.add_m(super::leaves::new_method("Get".into(), Default::default(),
                M::make_method(|m| m.path.prop_get(m)))
                .inarg::<&str,_>("interface_name")
                .inarg::<&str,_>("property_name")
                .outarg::<Variant<()>,_>("value"))
            .add_m(super::leaves::new_method("GetAll".into(), Default::default(),
                M::make_method(|m| m.path.prop_get_all(m)))
                .inarg::<&str,_>("interface_name")
                .outarg::<Dict<&str, Variant<()>, ()>,_>("props"))
            .add_m(super::leaves::new_method("Set".into(), Default::default(),
                M::make_method(|m| m.path.prop_set(m)))
                .inarg::<&str,_>("interface_name")
                .inarg::<&str,_>("property_name")
                .inarg::<Variant<bool>,_>("value"))
            .add_s(super::leaves::new_signal("PropertiesChanged".into(), Default::default())
                .sarg::<&str, _>("interface_name")
                .sarg::<Dict<&str, Variant<()>, ()>, _>("changed_properties")
                .sarg::<Vec<&str>, _>("invalidated_properties"))
        });
        self.ifaces.insert(z.name.clone(), z);
    }
}

pub fn new_objectpath<M: MethodType<D>, D: DataType>(n: Path<'static>, d: D::ObjectPath, cache: Arc<IfaceCache<M, D>>)
    -> ObjectPath<M, D> {
    ObjectPath { name: Arc::new(n), data: d, ifaces: ArcMap::new(), ifacecache: cache, default_iface: None }
}


/// A collection of object paths.
#[derive(Debug, Default)]
pub struct Tree<M: MethodType<D>, D: DataType> {
    paths: ArcMap<Arc<Path<'static>>, ObjectPath<M, D>>,
    data: D::Tree,
}

impl<M: MethodType<D>, D: DataType> Tree<M, D> {
    /// Builder function that adds an object path to this tree.
    pub fn add<I: Into<Arc<ObjectPath<M, D>>>>(mut self, s: I) -> Self {
        self.insert(s);
        self
    }

    /// Get a reference to an object path from the tree.
    pub fn get(&self, p: &Path<'static>) -> Option<&Arc<ObjectPath<M, D>>> {
        self.paths.get(p)
    }

    /// Iterates over object paths in this tree.
    pub fn iter<'a>(&'a self) -> Iter<'a, ObjectPath<M, D>> { IterE::Path(self.paths.values()).into() }

    /// Non-builder function that adds an object path to this tree.
    pub fn insert<I: Into<Arc<ObjectPath<M, D>>>>(&mut self, s: I) {
        let m = s.into();
        self.paths.insert(m.name.clone(), m);
    }


    /// Remove a object path from the Tree. Returns the object path removed, or None if not found.
    pub fn remove(&mut self, p: &Path<'static>) -> Option<Arc<ObjectPath<M, D>>> {
        // There is no real reason p needs to have a static lifetime; but
        // the borrow checker doesn't agree. :-(
        self.paths.remove(p)
    }

    /// Registers or unregisters all object paths in the tree to a ffidisp::Connection.
    pub fn set_registered(&self, c: &Connection, b: bool) -> Result<(), Error> {
        let mut regd_paths = Vec::new();
        for p in self.paths.keys() {
            if b {
                match c.register_object_path(p) {
                    Ok(()) => regd_paths.push(p.clone()),
                    Err(e) => {
                        while let Some(rp) = regd_paths.pop() {
                            c.unregister_object_path(&rp);
                        }
                        return Err(e)
                    }
                }
            } else {
                c.unregister_object_path(p);
            }
        }
        Ok(())
    }

    /// This method takes an `ConnectionItem` iterator (you get it from `Connection::iter()`)
    /// and handles all matching items. Non-matching items (e g signals) are passed through.
    pub fn run<'a, I: Iterator<Item=ConnectionItem>>(&'a self, c: &'a Connection, i: I) -> TreeServer<'a, I, M, D> {
        TreeServer { iter: i, tree: &self, conn: c }
    }

    /// Handles a message.
    ///
    /// Will return None in case the object path was not
    /// found in this tree, or otherwise a list of messages to be sent back.
    pub fn handle(&self, m: &Message) -> Option<Vec<Message>> {
        if m.msg_type() != MessageType::MethodCall { None }
        else { m.path().and_then(|p| self.paths.get(&p).map(|s| s.handle(m, &self)
            .unwrap_or_else(|e| vec!(e.to_message(m))))) }
    }

    /// Tries to handle an incoming message from the provided channel if there is one. If there isn't one,
    /// it will wait up to timeout
    pub fn process_channel(&self, channel: &Channel, timeout: Duration) -> Result<bool, Error> {
        if let Some(msg) = channel.blocking_pop_message(timeout)? {
            if let Some(replies) = self.handle(&msg) {
                for r in replies {
                    let _ = channel.send(r);
                }
            } else if let Some(reply) = dbus::channel::default_reply(&msg) {
                let _ = channel.send(reply);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }


    fn children(&self, o: &ObjectPath<M, D>, direct_only: bool) -> Vec<&ObjectPath<M, D>> {
        let parent: &str = &o.name;
        let plen = if parent == "/" { 1 } else { parent.len()+1 };
        let mut r: Vec<&ObjectPath<M, D>> = self.paths.values().filter_map(|v| {
            let k: &str = &v.name;
            if !k.starts_with(parent) || k.len() <= plen || &k[plen-1..plen] != "/" {None} else {
                Some(&**v)
            }
        }).collect();
        if direct_only {
            r.sort_by_key(|v| &**v.name);
            // println!("DEBUG before: {:?}", r.iter().map(|v| &**v.name).collect::<Vec<_>>());
            let mut prev: Option<&ObjectPath<M, D>> = None;
            r.retain(|v| {
                let a = prev.map(|prev|
                     !(v.name.starts_with(&**prev.name) && v.name.as_bytes().get(prev.name.len()) == Some(&b'/'))
                 ).unwrap_or(true);
                if a { prev = Some(v); }
                a
            });
        }
        r
    }

    /// Get associated data
    pub fn get_data(&self) -> &D::Tree { &self.data }
}

impl<M: MethodType<D> + 'static, D: DataType + 'static> Tree<M, D> {
    /// Connects a SyncConnection with a Tree so that incoming method calls are handled.
    ///
    /// The tree needs to be of type MTSync.
    pub fn start_receive_sync<C>(self, connection: &C)
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool + Send + Sync>> + channel::Sender,
        D::Tree: Send + Sync, D::ObjectPath: Send + Sync, D::Interface: Send + Sync,
        D::Property: Send + Sync, D::Method: Send + Sync, D::Signal: Send + Sync,
        M::Method: Send + Sync, M::GetProp: Send + Sync, M::SetProp: Send + Sync,
    {
        connection.start_receive(message::MatchRule::new_method_call(), Box::new(move |msg, c| {
            if let Some(replies) = self.handle(&msg) {
                for r in replies { let _ = c.send(r); }
            }
            true
        }));
    }

    /// Connects a Connection with a Tree so that incoming method calls are handled.
    ///
    /// The tree needs to be of type MTSync.
    pub fn start_receive_send<C>(self, connection: &C)
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool + Send>> + channel::Sender,
        D::Tree: Send + Sync, D::ObjectPath: Send + Sync, D::Interface: Send + Sync,
        D::Property: Send + Sync, D::Method: Send + Sync, D::Signal: Send + Sync,
        M::Method: Send + Sync, M::GetProp: Send + Sync, M::SetProp: Send + Sync,
    {
        connection.start_receive(message::MatchRule::new_method_call(), Box::new(move |msg, c| {
            if let Some(replies) = self.handle(&msg) {
                for r in replies { let _ = c.send(r); }
            }
            true
        }));
    }


    /// Connects a LocalConnection with a Tree so that incoming method calls are handled.
    pub fn start_receive<C>(self, connection: &C)
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool>> + channel::Sender
    {
        connection.start_receive(message::MatchRule::new_method_call(), Box::new(move |msg, c| {
            if let Some(replies) = self.handle(&msg) {
                for r in replies { let _ = c.send(r); }
            }
            true
        }));
    }

}

pub fn new_tree<M: MethodType<D>, D: DataType>(d: D::Tree) -> Tree<M, D> {
    Tree { paths: ArcMap::new(), data: d }
}

impl<M: MethodType<D>, D: DataType> MsgHandler for Tree<M, D> {
    fn handle_msg(&mut self, msg: &Message) -> Option<MsgHandlerResult> {
        self.handle(msg).map(|v| MsgHandlerResult { handled: true, done: false, reply: v })
    }
    fn handler_type(&self) -> MsgHandlerType { MsgHandlerType::MsgType(MessageType::MethodCall) }
}
/*
impl<M: MethodType<D>, D: DataType> MsgHandler for Arc<Tree<M, D>> {
    fn handle_msg(&mut self, msg: &Message) -> Option<MsgHandlerResult> {
        self.handle(msg).map(|v| MsgHandlerResult { handled: true, done: false, reply: v })
    }
    fn handler_type(&self) -> MsgHandlerType { MsgHandlerType::MsgType(MessageType::MethodCall) }
}
*/
/// An iterator adapter that handles incoming method calls.
///
/// Method calls that match an object path in the tree are handled and consumed by this
/// iterator. Other messages are passed through.
pub struct TreeServer<'a, I, M: MethodType<D> + 'a, D: DataType + 'a> {
    iter: I,
    conn: &'a Connection,
    tree: &'a Tree<M, D>,
}

impl<'a, I: Iterator<Item=ConnectionItem>, M: 'a + MethodType<D>, D: DataType + 'a> Iterator for TreeServer<'a, I, M, D> {
    type Item = ConnectionItem;

    fn next(&mut self) -> Option<ConnectionItem> {
        loop {
            let n = self.iter.next();
            if let Some(ConnectionItem::MethodCall(ref msg)) = n {
                if let Some(v) = self.tree.handle(&msg) {
                    // Probably the wisest is to ignore any send errors here -
                    // maybe the remote has disconnected during our processing.
                    for m in v { let _ = self.conn.send(m); };
                    continue;
                }
            }
            return n;
        }
    }
}


#[test]
fn test_iter() {
    let f = super::Factory::new_fn::<()>();
    let t = f.tree(())
    .add(f.object_path("/echo", ()).introspectable()
        .add(f.interface("com.example.echo", ())
            .add_m(f.method("Echo", (), |_| unimplemented!()).in_arg(("request", "s")).out_arg(("reply", "s")))
            .add_p(f.property::<i32,_>("EchoCount", ()))
            .add_s(f.signal("Echoed", ()).arg(("data", "s")).deprecated()
        )
    )).add(f.object_path("/echo/subpath", ()));

    let paths: Vec<_> = t.iter().collect();
    assert_eq!(paths.len(), 2);
}

#[test]
fn test_set_default_interface() {
    let iface_name: IfaceName<'_> = "com.example.echo".into();
    let f = super::Factory::new_fn::<()>();
    let t = f.object_path("/echo", ()).default_interface(iface_name.clone());
    assert_eq!(t.default_iface, Some(iface_name));
}


#[test]
fn test_introspection() {
    let f = super::Factory::new_fn::<()>();
    let t = f.object_path("/echo", ()).introspectable()
        .add(f.interface("com.example.echo", ())
            .add_m(f.method("Echo", (), |_| unimplemented!()).in_arg(("request", "s")).out_arg(("reply", "s")))
            .add_p(f.property::<i32,_>("EchoCount", ()))
            .add_s(f.signal("Echoed", ()).arg(("data", "s")).deprecated())
    );

    let actual_result = t.introspect(&f.tree(()).add(f.object_path("/echo/subpath2", ())).add(f.object_path("/echo/subpath", ())));
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
      <annotation name="org.freedesktop.DBus.Deprecated" value="true"/>
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
    <signal name="PropertiesChanged">
      <arg name="interface_name" type="s"/>
      <arg name="changed_properties" type="a{sv}"/>
      <arg name="invalidated_properties" type="as"/>
    </signal>
  </interface>
  <node name="subpath"/>
  <node name="subpath2"/>
</node>"##;

    assert_eq!(expected_result, actual_result);
}

#[test]
fn test_introspection_dynamic() {
    let f = super::Factory::new_fn::<()>();
    let tree = f
        .tree(())
        .add(f.object_path("/", ()).introspectable())
        .add(f.object_path("/foo/bar", ()).introspectable())
        .add(f.object_path("/foo/bar/item1", ()).introspectable());

    // For / only node /foo/bar is listed.
    let o = f.object_path("/", ()).introspectable();
    let actual_result = o.introspect(&tree);
    println!("\n=== Introspection XML start ===\n{}\n=== Introspection XML end ===", actual_result);

    let expected_result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/">
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
  <node name="foo/bar"/>
</node>"##;

    assert_eq!(expected_result, actual_result);

    // For /foo/bar node /foo/bar/item1 is listed.
    let o = f.object_path("/foo/bar", ()).introspectable();
    let actual_result = o.introspect(&tree);
    println!("\n=== Introspection XML start ===\n{}\n=== Introspection XML end ===", actual_result);

    let expected_result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/foo/bar">
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
  <node name="item1"/>
</node>"##;

    assert_eq!(expected_result, actual_result);

    // Dynamically add an object /foo/bar/item2.
    let tree = tree.add(f.object_path("/foo/bar/item2", ()).introspectable());

    // Now, for / still only node /foo/bar is listed.
    let o = f.object_path("/", ()).introspectable();
    let actual_result = o.introspect(&tree);
    println!("\n=== Introspection XML start ===\n{}\n=== Introspection XML end ===", actual_result);

    let expected_result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/">
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
  <node name="foo/bar"/>
</node>"##;

    assert_eq!(expected_result, actual_result);

    // And for /foo/bar node /foo/bar/item2 is now listed too.
    let o = f.object_path("/foo/bar", ()).introspectable();
    let actual_result = o.introspect(&tree);
    println!("\n=== Introspection XML start ===\n{}\n=== Introspection XML end ===", actual_result);

    let expected_result = r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="/foo/bar">
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg name="xml_data" type="s" direction="out"/>
    </method>
  </interface>
  <node name="item1"/>
  <node name="item2"/>
</node>"##;

    assert_eq!(expected_result, actual_result);
}
