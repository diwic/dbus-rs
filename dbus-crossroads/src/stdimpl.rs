use crate::utils::Dbg;
use std::sync::Mutex;
use std::sync::Arc;
use dbus::channel::Sender;
use std::collections::HashMap;
use crate::{IfaceToken, Crossroads, Context, MethodErr};
use dbus::arg::{Variant, RefArg, Arg, Append, PropMap};
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


pub (crate) fn make_emits_message<V: dbus::arg::Arg + dbus::arg::Append>(prop_name: &str, emits_changed: &str, ctx: &Context, v: &V) -> Option<dbus::Message> {
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
/// PropContext is a struct that provides helpful information inside a get/set property handler.
///
/// Like Context, but for get/set property handlers.
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
                self.context.as_mut().map(|ctx| ctx.reply(reply.map(|a| (Variant(a),))));
            }
        }
        PhantomData
    }

    /// Set a reply to a "set property" message (use in async context only)
    ///
    /// This can be used when the property does not send a "EmitsChanged" signal.
    pub fn reply_noemit(&mut self, reply: Result<(), MethodErr>) {
        debug_assert!(self.emits_changed.is_some());
        self.context.as_mut().map(|ctx| ctx.reply(reply));
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

    fn call_all_props<F: FnOnce(&mut PropAllCtx) + Send + 'static>(self, cr: &mut Crossroads, f: F) -> Option<Self> {
        let token = self.iface_token;
        let pactx = Arc::new(Mutex::new(PropAllCtx {
            remaining: 0,
            answers: HashMap::new(),
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
        // dbg!(&temp);
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
    answers: PropMap,
    donefn: Option<Dbg<Box<dyn FnOnce(&mut Self) + Send + 'static>>>,
    propctx: Option<PropContext>,
}

impl PropAllCtx {
    fn check_finished(&mut self) -> bool {
        if self.remaining > 0 { return false; }
        if let Some(donefn) = self.donefn.take() {
            (donefn.0)(self);
        }
        true
    }

    fn add_reply(&mut self, prop_name: String, prop_value: Option<Box<dyn RefArg + Send>>) {
        if let Some(v) = prop_value {
            self.answers.insert(prop_name, Variant(v));
        }
        self.remaining -= 1;
        self.check_finished();
    }
}

fn get(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name): (String, String)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropContext::new(cr, ctx.path().clone(), interface_name, property_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.context = Some(ctx);
    propctx.call_prop(cr, false).map(|propctx| { propctx.context.unwrap() })
}

fn getall(mut ctx: Context, cr: &mut Crossroads, (interface_name,): (String,)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropContext::new(cr, ctx.path().clone(), interface_name, "".into())}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.context = Some(ctx);
    propctx.call_all_props(cr, move |pactx| {
        let pctx = pactx.propctx.as_mut().unwrap();
        let answers = &pactx.answers;
        pctx.context.as_mut().unwrap().do_reply(|msg| {
            msg.append_all((answers,));
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
        b.method_with_cr_custom::<_, (PropMap,), _, _>("GetAll", ("interface_name",), ("properties",), getall);
        b.method_with_cr_custom::<_, (), _, _>("Set", ("interface_name", "property_name", "value"), (), set);
        b.signal::<(String, PropMap, Vec<String>), _>("PropertiesChanged",
            ("interface_name", "changed_properties", "invalidated_properties"));
    })
}

#[derive(Debug, Default)]
struct IfaceContext {
    remaining: usize,
    ifaces: IfacePropMap,
    donefn: Option<Dbg<Box<dyn FnOnce(&mut IfaceContext) + Send + 'static>>>,
}

type IfacePropMap = HashMap<String, PropMap>;
type PathPropMap = HashMap<dbus::Path<'static>, IfacePropMap>;

fn get_all_for_path<F: FnOnce(&mut IfaceContext) + Send + 'static>(path: &dbus::Path<'static>, cr: &mut Crossroads, f: F) {
    let ictx: Arc<Mutex<IfaceContext>> = Default::default();
    let (reg, ifaces) = cr.registry_and_ifaces(&path);
    let all: Vec<_> = ifaces.into_iter().filter_map(|&token| {
        if !reg.has_props(token) { return None };
        let iface_name = reg.get_intf_name(token)?;
        Some(PropContext {
            context: None,
            emits_changed: None,
            get_all: None,
            iface_token: token,
            interface: iface_name.clone(),
            path: path.clone(),
            name: "".into(),
        })
    }).collect();

    if all.len() == 0 {
        f(&mut *ictx.lock().unwrap());
        return;
    }

    let mut ic = ictx.lock().unwrap();
    ic.remaining = all.len();
    ic.donefn = Some(Dbg(Box::new(f)));
    drop(ic);
    for pctx in all.into_iter() {
        let iclone = ictx.clone();
        pctx.call_all_props(cr, move |pactx| {
            let mut ic = iclone.lock().unwrap();
            let answers = std::mem::replace(&mut pactx.answers, HashMap::new());
            // dbg!(&pactx);
            ic.ifaces.insert(pactx.propctx.as_ref().unwrap().interface.to_string(), answers);
            ic.remaining -= 1;
            if ic.remaining == 0 {
                let donefn = ic.donefn.take().unwrap().0;
                (donefn)(&mut *ic)
            }
        });
    }
}
//
fn get_managed_objects(mut ctx: Context, cr: &mut Crossroads, _: ()) -> Option<Context> {
    // HashMap<dbus::Path<'static>, IfacePropMap>
    let parent = ctx.path();
    let children: Vec<dbus::Path<'static>> =
        cr.get_children(ctx.path()).into_iter().map(|child_path| {
            let mut x = String::from(&**parent);
            if !x.ends_with('/') {
                x.push_str("/");
            }
            x.push_str(child_path);
            dbus::Path::from(x).into_static()
        }).collect();

    if children.len() == 0 {
        ctx.do_reply(|msg| {
            let x: PathPropMap = Default::default();
            msg.append_all((x,));
        });
        return Some(ctx);
    }

    #[derive(Debug)]
    struct Temp {
        remaining: usize,
        temp_map: PathPropMap,
        ctx: Option<Context>,
    }
    let r = Arc::new(Mutex::new(Temp {
        remaining: children.len(),
        temp_map: HashMap::new(),
        ctx: Some(ctx),
    }));
    let returned_ctx: Arc<Mutex<Option<Context>>> = Default::default();
    for subpath in children {
        let rclone = r.clone();
        let rctx = returned_ctx.clone();
        let subpath_clone = subpath.clone();
        get_all_for_path(&subpath, cr, move |ictx| {
            let mut rr = rclone.lock().unwrap();
            let ifaces = std::mem::replace(&mut ictx.ifaces, HashMap::new());
            rr.temp_map.insert(subpath_clone, ifaces);
            rr.remaining -= 1;
            // dbg!(&rr);
            if rr.remaining > 0 { return; }
            let mut ctx = rr.ctx.take().unwrap();
            ctx.do_reply(|msg| {
                msg.append_all((&rr.temp_map,));
            });
            *rctx.lock().unwrap() = Some(ctx);
        });
    }
    let mut lock = returned_ctx.lock().unwrap();
    lock.take()
}

pub fn object_manager(cr: &mut Crossroads) -> IfaceToken<()> {
    cr.register("org.freedesktop.DBus.ObjectManager", |b| {
        b.method_with_cr_custom::<(), (PathPropMap,), _, _>
            ("GetManagedObjects", (), ("objpath_interfaces_and_properties",), get_managed_objects);
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
        let n = name.clone();
        let s = sender.clone();
        get_all_for_path(&name, cr, move |ictx| {
            let x = dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesAdded {
                object: n,
                interfaces: std::mem::replace(&mut ictx.ifaces, HashMap::new()),
            };
            let _ = s.send(dbus::message::SignalArgs::to_emit_message(&x, &parent));
        });
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
