use crate::utils::Dbg;
use std::sync::Mutex;
use std::sync::Arc;
use dbus::channel::Sender;
use std::collections::HashMap;
use crate::{IfaceToken, Crossroads, Context, MethodErr};
use dbus::arg::{Variant, RefArg, Arg, Append};
use std::marker::PhantomData;
use crate::ifacedesc::EMITS_CHANGED;

fn introspect(cr: &Crossroads, path: &dbus::Path<'static>) -> String {
    let mut children = cr.get_children(path);
    let mut childstr = String::new();
    children.sort_unstable();
    for c in children {
        childstr += &format!("  <node name=\"{}\"/>\n", c);
    }
    let (reg, ifaces) = cr.registry_and_ifaces(path);
    let ifacestr = reg.introspect(ifaces);

    let nodestr = format!(
r##"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node name="{}">
{}{}</node>"##, path, ifacestr, childstr);
    nodestr
}

pub fn introspectable(cr: &mut Crossroads) -> IfaceToken<()> {
    cr.register("org.freedesktop.DBus.Introspectable", |b| {
        b.method_with_cr("Introspect", (), ("xml_data",), |ctx, cr, _: ()| {
            Ok((introspect(cr, ctx.path()),))
        });
    })
}


fn make_emits_message<V: dbus::arg::Arg + dbus::arg::Append>(prop_name: &str, emits_changed: &str, ctx: &Context, v: &V) -> Option<dbus::Message> {
    let arr = [prop_name];
    let (d, i) = match emits_changed {
        "false" => return None,
        "invalidates" => (None, &arr[..]),
        "true" => (Some((arr[0], Variant(v))), &[][..]),
        _ => panic!("Invalid value of EmitsChangedSignal: {:?}", emits_changed)
    };

    use dbus::message::SignalArgs;
    use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PPC;
    let s: &str = ctx.message().read1().unwrap();
    Some(dbus::Message::signal(ctx.path(), &PPC::INTERFACE.into(), &PPC::NAME.into())
        .append3(s, dbus::arg::Dict::new(d), i))
}


#[derive(Debug)]
pub struct PropContext {
    path: dbus::Path<'static>,
    interface: dbus::strings::Interface<'static>,
    name: String,
    context: Option<Context>,

    iface_token: usize,
    emits_changed: Option<&'static str>,
    get_all: Option<Arc<Mutex<PropAllCtx>>>,
}

impl PropContext {
    /// The current object path.
    pub fn path(&self) -> &dbus::Path<'static> { &self.path }

    /// The current interface name.
    pub fn interface(&self) -> &dbus::strings::Interface<'static> { &self.interface }

    /// The current property name.
    pub fn name(&self) -> &str { &self.name }

    /// The message, if any, that caused this method to be called.
    pub fn message(&self) -> Option<&dbus::Message> { self.context.as_ref().map(|ctx| ctx.message()) }

    /// Set a reply to message (use in async context only)
    ///
    /// Returns PhantomData just to aid the type system
    pub fn reply<A: Arg + RefArg + Send + Append + 'static>(&mut self, reply: Result<A, MethodErr>) -> PhantomData<A> {
        if let Some(ec) = &self.emits_changed {
            let mut emit_msg = None;
            if let Ok(v) = &reply {
                if let Some(ctx) = &self.context {
                    emit_msg = make_emits_message(&self.name, ec, &ctx, v);
                }
            }
            self.reply_noemit(reply.map(|_| ()));
            emit_msg.map(|emit_msg| self.context.as_mut().map(|ctx| { ctx.push_msg(emit_msg) }));
        } else {
            if let Some(ga) = &self.get_all {
                ga.lock().unwrap().add_reply(self.name.clone(), reply.ok().map(|a| Box::new(a) as Box<(dyn RefArg + Send)>));
            } else {
                self.context.as_mut().map(|ctx| ctx.reply_result(reply.map(|a| (Variant(a),))));
            }
        }
        PhantomData
    }

    /// Set a reply to a "set property" message (use in async context only)
    ///
    /// This can be used when the property does not send a "EmitsChanged" signal.
    pub fn reply_noemit(&mut self, reply: Result<(), MethodErr>) {
        debug_assert!(self.emits_changed.is_some());
        self.context.as_mut().map(|ctx| ctx.reply_result(reply));
    }

    pub (crate) fn set_send_on_drop(&mut self, value: Arc<dyn Sender + Send + Sync>) {
        self.context.as_mut().map(|ctx| ctx.set_send_on_drop(value));
    }

    fn new(cr: &Crossroads, path: dbus::Path<'static>, interface: String, name: String) -> Result<Self, MethodErr> {
        let interface = dbus::strings::Interface::new(interface).map_err(|s| MethodErr::no_interface(&s))?;
        let iface_token = cr.find_iface_token(&path, Some(&interface))?;
        Ok(PropContext {
            path,
            iface_token,
            interface,
            name,
            get_all: None,
            context: None,
            emits_changed: None
        })
    }

    fn call_prop(mut self, cr: &mut Crossroads, is_set: bool) -> Option<Self> {
        let token = self.iface_token;
        let name = self.name.clone();
        let mut cb = match self.check(|_| {
            cr.registry().take_prop(token, &name, is_set)
        }) {
            Ok(cb) => cb,
            Err(_) => return Some(self)
        };
        let octx = cb(self, cr);
        cr.registry().give_prop(token, &name, cb, is_set);
        octx
    }

    fn call_all_props<F: FnOnce(&mut Self, PropMap) + Send + 'static>(self, cr: &mut Crossroads, f: F) -> Option<Self> {
        let token = self.iface_token;
        let pactx = Arc::new(Mutex::new(PropAllCtx {
            remaining: 0,
            answers: vec!(),
            donefn: Some(Dbg(Box::new(f))),
            propctx: Some(self),
        }));
        let mut pb = pactx.lock().unwrap();
        let pctxs: Vec<_> = cr.registry().prop_names_readable(token).map(|prop_name| {
            pb.remaining += 1;
            let parent = pb.propctx.as_ref().unwrap();
            PropContext {
                path: parent.path.clone(),
                iface_token: parent.iface_token,
                interface: parent.interface().clone(),
                name: prop_name.into(),
                get_all: Some(pactx.clone()),
                context: None,
                emits_changed: None
            }
        }).collect();
        drop(pb);
        for pctx in pctxs {
            pctx.call_prop(cr, false);
        }
        let mut temp = pactx.lock().unwrap();
        if temp.check_finished() { Some(temp.propctx.take().unwrap()) } else { None }
    }

    /// Convenience method that sets an error reply if the closure returns an error.
    pub fn check<R, F: FnOnce(Option<&mut Context>) -> Result<R, MethodErr>>(&mut self, f: F) -> Result<R, ()> {
        match &mut self.context {
            Some(ctx) => ctx.check(|ctx| f(Some(ctx))),
            None => match f(None) {
                Ok(r) => Ok(r),
                Err(_) => { todo!() },
            },
        }
    }
}

#[derive(Debug)]
struct PropAllCtx {
    remaining: usize,
    answers: Vec<(String, Box<dyn RefArg + Send>)>,
    donefn: Option<Dbg<Box<dyn FnOnce(&mut PropContext, PropMap) + Send + 'static>>>,
    propctx: Option<PropContext>,
}

impl PropAllCtx {
    fn check_finished(&mut self) -> bool {
        if self.remaining > 0 { return false; }
        if let Some(donefn) = self.donefn.take() {
            let mut h = HashMap::new();
            for (k,v) in self.answers.drain(..) {
                // Rebuild to change RefArg + Send => RefArg
                h.insert(k, Variant(v as Box<dyn RefArg>));
            }
            let mut pctx = self.propctx.as_mut().unwrap();
            (donefn.0)(&mut pctx, h);
        }
        true
    }

    fn add_reply(&mut self, prop_name: String, prop_value: Option<Box<dyn RefArg + Send>>) {
        if let Some(v) = prop_value {
            self.answers.push((prop_name, v));
        }
        self.remaining -= 1;
        self.check_finished();
    }
}

/*
#[derive(Debug)]
pub (crate) struct PropCtx {
    iface_token: usize,
    emits_changed: Option<&'static str>,

    prop_names: Vec<String>,
    prop_name: Option<String>,
    get_msg: Option<dbus::Message>,
}

impl PropCtx {
    fn new(cr: &Crossroads, path: &dbus::Path<'static>, iface_name: String) -> Result<PropCtx, MethodErr> {
        let name = dbus::strings::Interface::new(iface_name).map_err(|s| MethodErr::no_interface(&s))?;
        let iface_token = cr.find_iface_token(path, Some(&name))?;
        Ok(PropCtx {
            iface_token,
            prop_names: vec!(),
            prop_name: None,
            get_msg: None,
            emits_changed: None
        })
    }
/*
    fn call_prop(mut self, mut ctx: Context, cr: &mut Crossroads, prop_name: String, is_set: bool) -> Option<(Context, Self)> {
        let token = self.iface_token;
        let pname = prop_name.clone();
        self.prop_name = Some(prop_name);
        let mut cb = match ctx.check(|_| {
            cr.registry().take_prop(token, &pname, is_set)
        }) {
            Ok(cb) => cb,
            Err(_) => return Some((ctx, self))
        };
        ctx.give_prop_ctx(self);
        let octx = cb(ctx, cr);
        cr.registry().give_prop(token, &pname, cb, is_set);
        octx.map(|mut ctx| {
            let prop_ctx = ctx.take_prop_ctx();
            (ctx, prop_ctx)
        })
    }
*/
/*
    fn run_getall(mut self, mut ctx: Context, cr: &mut Crossroads) -> Option<Context> {
        loop {
            if let Some(next_name) = self.prop_names.pop() {
                if let Some(temp_msg) = self.get_msg.as_mut() {
                    temp_msg.append_all((&next_name,));
                }
                self.prop_name = Some(next_name.clone());
                let x = self.call_prop(ctx, cr, next_name, false)?;
                ctx = x.0;
                self = x.1;
                if ctx.has_reply() { return Some(ctx) }
            } else {
                ctx.do_reply(|mut msg| {
                    // This is quite silly, but I found no other way around the combination of
                    // Async + Send + RefArg being !Send than to first append it to one message
                    // and then read it just to append it to another...
                    let mut a1 = IterAppend::new(&mut msg);
                    a1.append_dict(&<String as Arg>::signature(), &<Variant<()> as Arg>::signature(), |a2| {
                        if let Some(temp_msg) = self.get_msg.take() {
                            let mut i = temp_msg.iter_init();
                            while let Ok(k) = i.read::<&str>() {
                                let v = i.get_refarg().unwrap();
                                a2.append_dict_entry(|a3| {
                                    a3.append(k);
                                    v.append(a3);
                                });
                                i.next();
                            }
                        }
                    });
                });
                return Some(ctx)
            }
        }
    }
*/
    pub (crate) fn add_get_result<V: dbus::arg::RefArg + Send>(&mut self, v: V) {
        if let Some(mut get_msg) = self.get_msg.as_mut() {
            let mut m = IterAppend::new(&mut get_msg);
            Variant(v).append(&mut m);
        }
    }

    pub (crate) fn make_emits_message<V: dbus::arg::Arg + dbus::arg::Append>(&self, ctx: &Context, v: V) -> Option<dbus::Message> {
        let arr = [self.prop_name.as_ref().unwrap()];
        let (d, i) = match self.emits_changed.as_ref().map(|x| &**x) {
            Some("false") => return None,
            Some("invalidates") => (None, &arr[..]),
            None | Some("true") => (Some((arr[0], Variant(&v))), &[][..]),
            _ => panic!("Invalid value of EmitsChangedSignal: {:?}", self.emits_changed)
        };

        use dbus::message::SignalArgs;
        use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PPC;
        let s: &str = ctx.message().read1().unwrap();
        Some(dbus::Message::signal(ctx.path(), &PPC::INTERFACE.into(), &PPC::NAME.into())
            .append3(s, dbus::arg::Dict::new(d), i))
    }
}
*/
type Props = HashMap<String, Variant<Box<dyn RefArg>>>;

fn get(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name): (String, String)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropContext::new(cr, ctx.path().clone(), interface_name, property_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.context = Some(ctx);
    propctx.call_prop(cr, false).map(|propctx| { propctx.context.unwrap() })
/*
    if !ctx.message().get_no_reply() {
        propctx.get_msg = Some(ctx.message().method_return());
    }
    propctx.call_prop(ctx, cr, property_name, false).map(|(mut ctx, propctx)| {
        if !ctx.has_reply() { ctx.set_reply(propctx.get_msg, true, true) }
        ctx
    }) */
}

fn getall(mut ctx: Context, cr: &mut Crossroads, (interface_name,): (String,)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropContext::new(cr, ctx.path().clone(), interface_name, "".into())}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.context = Some(ctx);
    propctx.call_all_props(cr, move |propctx, all_props| {
        propctx.context.as_mut().unwrap().do_reply(|msg| {
            msg.append_all((all_props,));
        });
    }).map(|propctx| { propctx.context.unwrap() })
}

fn set(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name, _value): (String, String, Variant<Box<dyn RefArg>>)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropContext::new(cr, ctx.path().clone(), interface_name, property_name) }) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    let ann = cr.registry()
        .find_annotation(propctx.iface_token, EMITS_CHANGED, Some(&propctx.name));
    propctx.emits_changed = match ann {
        Some("const") => Some("const"),
        Some("false") => Some("false"),
        Some("invalidates") => Some("invalidates"),
        _ => Some("true"),
    };
    propctx.context = Some(ctx);
    propctx.call_prop(cr, true).map(|propctx| { propctx.context.unwrap() })
}

pub fn properties(cr: &mut Crossroads) -> IfaceToken<()> {
    cr.register("org.freedesktop.DBus.Properties", |b| {
        b.method_with_cr_custom::<_, (Variant<u8>,), _, _>("Get", ("interface_name", "property_name"), ("value",), get);
        b.method_with_cr_custom::<_, (Props,), _, _>("GetAll", ("interface_name",), ("properties",), getall);
        b.method_with_cr_custom::<_, (), _, _>("Set", ("interface_name", "property_name", "value"), (), set);
        b.signal::<(String, Props, Vec<String>), _>("PropertiesChanged",
            ("interface_name", "changed_properties", "invalidated_properties"));
    })
}

type PropMap = HashMap<String, Variant<Box<dyn RefArg>>>;
type IfacePropMap = HashMap<String, PropMap>;

fn get_all_for_path(path: &dbus::Path<'static>, cr: &mut Crossroads) -> IfacePropMap {
    let mut i = HashMap::new();
    let (reg, ifaces) = cr.registry_and_ifaces(&path);
    for iface in ifaces.into_iter().filter_map(|iface| reg.get_intf_name(*iface)) {
        // TODO
        i.insert(String::from(&**iface), HashMap::new());
    }
    i
}

fn get_managed_objects(cr: &mut Crossroads, path: &dbus::Path<'static>) -> HashMap<dbus::Path<'static>, IfacePropMap> {
    let children: Vec<dbus::Path<'static>> =
        cr.get_children(path).into_iter().map(|path| dbus::Path::from(path).into_static()).collect();
    let mut r = HashMap::new();
    for path in children {
        let i = get_all_for_path(&path, cr);
        r.insert(path, i);
    }
    r
}

pub fn object_manager(cr: &mut Crossroads) -> IfaceToken<()> {
    cr.register("org.freedesktop.DBus.ObjectManager", |b| {
        b.method_with_cr("GetManagedObjects", (), ("objpath_interfaces_and_properties",), |ctx, cr, _: ()| {
            Ok((get_managed_objects(cr, ctx.path()),))
        });
        b.signal::<(dbus::Path<'static>, IfacePropMap), _>("InterfacesAdded",
            ("object_path", "interfaces_and_properties"));
        b.signal::<(dbus::Path<'static>, Vec<String>), _>("InterfacesRemoved",
            ("object_path", "interfaces"));
    })
}

fn object_manager_parents<F: FnMut(dbus::Path<'static>, &mut Crossroads)>(name: &dbus::Path<'static>, cr: &mut Crossroads, mut f: F) {
    for idx in 0..name.len()-1 {
        if name.as_bytes()[idx] != b'/' { continue; }
        let parent = dbus::Path::from(&name[0..(if idx == 0 { idx + 1 } else {idx})]).into_static();
        if !cr.has_interface(&parent, cr.object_manager::<()>()) { continue; }
        f(parent, cr)
    }
}

pub fn object_manager_path_added(sender: Arc<dyn Sender + Send + Sync>, name: &dbus::Path<'static>, cr: &mut Crossroads) {
    object_manager_parents(name, cr, |parent, cr| {
        let x = dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesAdded {
            object: name.clone(),
            interfaces: get_all_for_path(&name, cr),
        };
        let _ = sender.send(dbus::message::SignalArgs::to_emit_message(&x, &parent));
    });
}

pub fn object_manager_path_removed(sender: Arc<dyn Sender + Send + Sync>, name: &dbus::Path<'static>, cr: &mut Crossroads) {
    object_manager_parents(name, cr, |parent, cr| {
        let (reg, ifaces) = cr.registry_and_ifaces(&name);

        let x = dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesRemoved {
            object: name.clone(),
            interfaces: ifaces.into_iter()
                .filter_map(|iface| reg.get_intf_name(*iface))
                .map(|iface| String::from(&**iface))
                .collect(),
        };
        let _ = sender.send(dbus::message::SignalArgs::to_emit_message(&x, &parent));
    });
}
