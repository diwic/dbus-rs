use std::marker::PhantomData;
use crate::{Context, MethodErr, IfaceBuilder};
use crate::ifacedesc::Registry;
use std::collections::{BTreeMap, HashSet};
use std::any::Any;

const INTROSPECTABLE: usize = 0;

#[derive(Debug, Copy, Clone, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub struct IfaceToken<T: Send + 'static>(usize, PhantomData<&'static T>);


#[derive(Debug)]
struct Object {
    ifaces: HashSet<usize>,
    data: Box<dyn Any + Send + 'static>
}

#[derive(Default, Debug)]
pub struct Crossroads {
    map: BTreeMap<dbus::Path<'static>, Object>,
    registry: Registry,
}

impl Crossroads {
    pub fn new() -> Crossroads {
        let mut registry: Registry = Default::default();
        registry.push(crate::stdimpl::introspectable());
        Crossroads {
            map: Default::default(),
            registry
        }
    }

    pub fn register<T, N, F>(&mut self, name: N, f: F) -> IfaceToken<T>
    where T: Send + 'static, N: Into<dbus::strings::Interface<'static>>,
    F: FnOnce(&mut IfaceBuilder<T>)
    {
        let iface = IfaceBuilder::build(Some(name.into()), f);
        let x = self.registry.push(iface);
        IfaceToken(x, PhantomData)
    }

    pub fn data_mut<D: Any + Send + 'static>(&mut self, name: &dbus::Path<'static>) -> Option<&mut D> {
        let obj = self.map.get_mut(name)?;
        obj.data.downcast_mut()
    }

    pub fn insert<'z, D, I>(&mut self, name: dbus::Path<'static>, ifaces: I, data: D)
    where D: Any + Send + 'static, I: IntoIterator<Item = &'z IfaceToken<D>>
    {
        let ifaces = ifaces.into_iter().map(|x| x.0);
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
        // No failure paths before method is given back!
        let methodname = ctx.method().clone();
        let ctx = cb(ctx, self);
        self.registry.give_method(itoken, &methodname, cb);
        if let Some(mut ctx) = ctx { ctx.flush_messages(conn) } else { Ok(()) }
    }

    pub fn introspectable<T: Send + 'static>(&self) -> IfaceToken<T> { IfaceToken(INTROSPECTABLE, PhantomData) }
}

#[test]
fn test_send() {
    fn is_send<T: Send>(_: &T) {}
    let c = Crossroads::new();
    dbg!(&c);
    is_send(&c);
}

#[test]
fn score() {
    struct Score(u16, u32);

    let mut cr = Crossroads::new();

    let iface = cr.register("com.example.dbusrs.crossroads.score", |b: &mut IfaceBuilder<Score>| {
        b.method("UpdateScore", ("change",), ("new_score", "call_times"), |_, score, (change,): (u16,)| {
            score.0 += change;
            score.1 += 1;
            Ok((score.0, score.1))
        });
    });

    cr.insert("/".into(), &[iface, cr.introspectable()], Score(7u16, 0));

}
