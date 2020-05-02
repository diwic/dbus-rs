use crate::{Context, MethodErr, IfaceToken};
use crate::ifacedesc::Registry;
use std::collections::{BTreeMap, HashSet};
use std::any::Any;

#[derive(Debug)]
struct Object {
    ifaces: HashSet<IfaceToken>,
    data: Box<dyn Any + Send + 'static>
}

#[derive(Default, Debug)]
pub struct Crossroads {
    map: BTreeMap<dbus::Path<'static>, Object>,
    registry: Registry,
}

impl Crossroads {
    pub fn new() -> Crossroads { Default::default() }

    pub fn insert<D, I>(&mut self, name: dbus::Path<'static>, ifaces: I, data: D)
    where D: Any + Send + 'static, I: IntoIterator<Item = IfaceToken>
    {
        let ifaces = ifaces.into_iter();
        let ifaces = std::iter::FromIterator::from_iter(ifaces);
        self.map.insert(name, Object { ifaces, data: Box::new(data)});
    }

    pub fn handle_message<S: dbus::channel::Sender>(&mut self, message: dbus::Message, conn: &S) -> Result<(), ()> {
        let mut ctx = Context::new(message).ok_or(())?;
        let (itoken, mut cb) = ctx.check(|ctx| {
            let obj = self.map.get_mut(ctx.path()).ok_or_else(|| MethodErr::no_path(ctx.path()))?;
            let itoken = self.registry.find_token(ctx.interface(), &obj.ifaces)?;
            let cb = self.registry.take_method(itoken, ctx.method())?;
            Ok((itoken, cb))
        })?;

        let methodname = ctx.method().clone();
        let ctx = cb(ctx, self);
        self.registry.give_method(itoken, &methodname, cb);
        if let Some(mut ctx) = ctx { ctx.flush_messages(conn) } else { Ok(()) }
    }
}

#[test]
fn test_send() {
    fn is_send<T: Send>(_: &T) {}
    let c = Crossroads::new();
    dbg!(&c);
    is_send(&c);
}
