use std::collections::HashMap;
use crate::{IfaceToken, Crossroads, Context, MethodErr};
use dbus::arg::{Variant, RefArg, IterAppend, Arg};

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

#[derive(Debug)]
pub (crate) struct PropCtx {
    iface_token: usize,

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
        })
    }

    fn call_prop(self, mut ctx: Context, cr: &mut Crossroads, prop_name: &str, is_set: bool) -> Option<(Context, Self)> {
        let token = self.iface_token;
        let mut cb = match ctx.check(|ctx| {
            cr.registry().take_prop(token, prop_name, is_set)
        }) {
            Ok(cb) => cb,
            Err(_) => return Some((ctx, self))
        };
        ctx.give_prop_ctx(self);
        let octx = cb(ctx, cr);
        dbg!(&octx);
        cr.registry().give_prop(token, prop_name, cb, is_set);
        octx.map(|mut ctx| {
            let prop_ctx = ctx.take_prop_ctx();
            (ctx, prop_ctx)
        })
    }

    fn run_getall(mut self, mut ctx: Context, cr: &mut Crossroads) -> Option<Context> {
        loop {
            if let Some(next_name) = self.prop_names.pop() {
                if let Some(temp_msg) = self.get_msg.as_mut() {
                    temp_msg.append_all((&next_name,));
                }
                self.prop_name = Some(next_name.clone());
                let x = self.call_prop(ctx, cr, &next_name, false)?;
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
                    dbg!(&msg);
                });
                return Some(ctx)
            }
        }
    }

    pub (crate) fn add_get_result<V: 'static + dbus::arg::Arg + dbus::arg::Append + dbus::arg::RefArg>(&mut self, v: V) {
        if let Some(get_msg) = self.get_msg.as_mut() {
            get_msg.append_all((&Variant(v),));
        }
    }
}

type Props = HashMap<String, Variant<Box<dyn RefArg>>>;

fn get(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name): (String, String)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropCtx::new(cr, ctx.path(), interface_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    if !ctx.message().get_no_reply() {
        propctx.get_msg = Some(ctx.message().method_return());
    }
    propctx.call_prop(ctx, cr, &property_name, false).map(|(mut ctx, propctx)| {
        if !ctx.has_reply() { ctx.set_reply(propctx.get_msg, true, true) }
        ctx
    })
}

fn getall(mut ctx: Context, cr: &mut Crossroads, (interface_name,): (String,)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropCtx::new(cr, ctx.path(), interface_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.prop_names = cr.registry().prop_names_readable(propctx.iface_token);
    if !ctx.message().get_no_reply() {
        propctx.get_msg = Some(ctx.message().method_return());
    }

    propctx.run_getall(ctx, cr)
}

fn set(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name, value): (String, String, Variant<Box<dyn RefArg>>)) -> Option<Context> {
    let propctx = match ctx.check(|ctx| { PropCtx::new(cr, ctx.path(), interface_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    propctx.call_prop(ctx, cr, &property_name, true).map(|(mut ctx, propctx)| {
        ctx.do_reply(|_| {});
        ctx
    })
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
