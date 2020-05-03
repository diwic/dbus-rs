use std::marker::PhantomData;
use crate::{Context, MethodErr, IfaceBuilder,stdimpl};
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

#[derive(Debug)]
pub struct Crossroads {
    map: BTreeMap<dbus::Path<'static>, Object>,
    registry: Registry,
}

impl Crossroads {
    pub fn new() -> Crossroads {
        let mut cr = Crossroads {
            map: Default::default(),
            registry: Default::default(),
        };
        let t0 = stdimpl::introspectable(&mut cr);
        debug_assert_eq!(t0.0, INTROSPECTABLE);
        cr
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

    pub (crate) fn find_iface_token(&self,
        path: &dbus::Path<'static>,
        interface: Option<&dbus::strings::Interface<'static>>)
    -> Result<usize, MethodErr> {
        let obj = self.map.get(path).ok_or_else(|| MethodErr::no_path(path))?;
        self.registry.find_token(interface, &obj.ifaces)
    }

    pub (crate) fn registry(&mut self) -> &mut Registry { &mut self.registry }

    pub fn handle_message<S: dbus::channel::Sender>(&mut self, message: dbus::Message, conn: &S) -> Result<(), ()> {
        let mut ctx = Context::new(message).ok_or(())?;
        let (itoken, mut cb) = ctx.check(|ctx| {
            let itoken = self.find_iface_token(ctx.path(), ctx.interface())?;
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


#[cfg(test)]
mod test {
    use crate::*;
    use dbus::Message;
    use std::cell::RefCell;

    #[test]
    fn test_send() {
        fn is_send<T: Send>(_: &T) {}
        let c = Crossroads::new();
        dbg!(&c);
        is_send(&c);
    }

    fn dispatch_helper2(cr: &mut Crossroads, mut msg: Message) -> Vec<Message> {
        msg.set_serial(57);
        let r = RefCell::new(vec!());
        cr.handle_message(msg, &r).unwrap();
        r.into_inner()
    }

    fn dispatch_helper(cr: &mut Crossroads, msg: Message) -> Message {
        let mut r = dispatch_helper2(cr, msg);
        assert_eq!(r.len(), 1);
        r[0].as_result().unwrap();
        r.into_iter().next().unwrap()
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

        cr.insert("/".into(), &[iface, cr.introspectable()], Score(7, 0));

        let msg = Message::call_with_args("com.example.dbusrs.crossroads.score", "/",
            "com.example.dbusrs.crossroads.score", "UpdateScore", (5u16,));
        let r = dispatch_helper(&mut cr, msg);
        let (new_score, call_times): (u16, u32) = r.read2().unwrap();
        assert_eq!(new_score, 12);
        assert_eq!(call_times, 1);
    }

}
