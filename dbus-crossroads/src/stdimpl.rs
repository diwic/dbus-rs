use std::collections::HashMap;
use crate::{IfaceToken, Crossroads, Context, MethodErr};
use dbus::arg::{Variant, RefArg, IterAppend};

fn introspect(cr: &Crossroads, path: &dbus::Path<'static>) -> String { todo!() }

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
    getall_result: Option<Props>,
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
            getall_result: None,
            get_msg: None,
        })
    }

    fn call_getprop(self, mut ctx: Context, cr: &mut Crossroads, prop_name: &str) -> Option<(Context, Self)> {
        let token = self.iface_token;
        let mut cb = match ctx.check(|ctx| {
            cr.registry().take_getprop(token, prop_name)
        }) {
            Ok(cb) => cb,
            Err(_) => return Some((ctx, self))
        };
        ctx.give_prop_ctx(self);
        let octx = cb(ctx, cr);
        dbg!(&octx);
        cr.registry().give_getprop(token, prop_name, cb);
        octx.map(|mut ctx| {
            let prop_ctx = ctx.take_prop_ctx();
            (ctx, prop_ctx)
        })
    }

    fn run_getall(mut self, mut ctx: Context, cr: &mut Crossroads) -> Option<Context> {
        loop {
            if let Some(next_name) = self.prop_names.pop() {
                self.prop_name = Some(next_name.clone());
                let x = self.call_getprop(ctx, cr, &next_name)?;
                ctx = x.0;
                self = x.1;
                if ctx.has_reply() { return Some(ctx) }
            } else {
                ctx.do_reply(|mut msg| {
                    let p = self.getall_result.unwrap();
                    IterAppend::new(&mut msg).append(p);
                    dbg!(&msg);
                });
                return Some(ctx)
            }
        }
    }

    pub (crate) fn add_get_result<V: 'static + dbus::arg::Arg + dbus::arg::Append + dbus::arg::RefArg>(&mut self, v: V) {
        if let Some(map) = self.getall_result.as_mut() {
            map.insert(self.prop_name.take().unwrap(), Variant(Box::new(v)));
        } else if let Some(get_msg) = self.get_msg.as_mut() {
            IterAppend::new(get_msg).append(&Variant(v));
        }
    }
}

// fn get(ctx: &mut Context, cr: &mut Crossroads, (interface_name: &str, property_name: &str))
type Props = HashMap<String, Variant<Box<dyn RefArg>>>;

fn get(mut ctx: Context, cr: &mut Crossroads, (interface_name, property_name): (String, String)) -> Option<Context> {
    let mut propctx = match ctx.check(|ctx| { PropCtx::new(cr, ctx.path(), interface_name)}) {
        Ok(p) => p,
        Err(_) => return Some(ctx),
    };
    if !ctx.message().get_no_reply() {
        propctx.get_msg = Some(ctx.message().method_return());
    }
    propctx.call_getprop(ctx, cr, &property_name).map(|(mut ctx, propctx)| {
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
    propctx.getall_result = Some(HashMap::new());
    propctx.run_getall(ctx, cr)
}

fn set(ctx: Context, cr: &mut Crossroads, (interface_name, property_name, value): (String, String, Variant<Box<dyn RefArg>>)) -> Option<Context> {
    todo!()
}


pub fn properties(cr: &mut Crossroads) -> IfaceToken<()> {
    cr.register("org.freedesktop.DBus.Properties", |b| {
        b.method_with_cr_async::<_, (Variant<u8>,), _, _>("Get", ("interface_name", "property_name"), ("value",), get);
        b.method_with_cr_async::<_, (Props,), _, _>("GetAll", ("interface_name",), ("props",), getall);
        b.method_with_cr_async::<_, (), _, _>("Set", ("interface_name", "property_name", "value"), (), set);
        b.signal::<(String, Props, Vec<String>), _>("PropertiesChanged",
            ("interface_name", "changed_properties", "invalidated_properties"));
    })
}
