use std::collections::{BTreeMap, HashMap};
use std::any::{TypeId, Any};
use std::ffi::{CString, CStr};
use std::fmt;
use std::ops::Deref;
use crate::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use crate::{Message, MessageType, channel};
use crate::message::MatchRule;
use super::info::{IfaceInfo, MethodInfo, PropInfo, IfaceInfoBuilder};
use super::handlers::{self, Handlers, Par};
use super::stdimpl::{DBusProperties, DBusIntrospectable};
use super::path::{Path, PathData};
use super::context::{MsgCtx, RefCtx};
use super::MethodErr;

pub (super) struct RegEntry<H: Handlers> {
    pub typeid: TypeId,
    pub info: IfaceInfo<'static, H>,
    pub path_insert: Option<Box<dyn Fn(&mut Path<H>, &Crossroads<H>) + Send + Sync>>
}

impl<H: Handlers> RegEntry<H> {
    pub fn new<I: 'static>(name: IfaceName<'static>) -> Self {
        RegEntry {
            typeid: TypeId::of::<I>(),
            info: IfaceInfo::new_empty(name),
            path_insert: None
        }
    }
}


impl<H: Handlers> fmt::Debug for RegEntry<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "RegEntry") }
}

#[derive(Debug)]
pub struct Crossroads<H: Handlers> {
    pub (super) reg: BTreeMap<CString, RegEntry<H>>,
    pub (super) paths: BTreeMap<CString, Path<H>>,
}

impl<H: Handlers> Crossroads<H> {
    pub fn insert(&mut self, mut path: Path<H>) {
        for x in self.reg.values() {
            if let Some(ref cb) = x.path_insert { cb(&mut path, self) }
        }
        let c = path.name().clone().into_cstring();
        self.paths.insert(c, path);
    }

    /// Path accessor
    pub fn get<N: Into<PathName<'static>>>(&self, name: N) -> Option<&Path<H>> {
        self.paths.get(name.into().as_cstr())
    }

    /// Allows for direct manipulation of a path, bypassing signal generation
    pub fn get_mut<N: Into<PathName<'static>>>(&mut self, name: N) -> Option<&mut Path<H>> {
        self.paths.get_mut(name.into().as_cstr())
    }

    pub fn register<'a, I: 'static, N: Into<IfaceName<'static>>>(&'a mut self, name: N) -> IfaceInfoBuilder<'a, I, H> {
        IfaceInfoBuilder::new(Some(self), name.into())
    }

/*
    fn reg_lookup(&self, ctx: &MsgCtx) -> Option<(RefCtx<H>, &MethodInfo<'static, H>)> {
        let refctx = RefCtx::new(self, ctx)?;
        let minfo = refctx.iinfo.methods.iter().find(|x| x.name() == &ctx.member)?;
        Some((refctx, minfo))
    }
    */

/*
    pub (super) fn reg_prop_lookup<'a>(&'a self, data: &'a Path<H>, iname: &CStr, propname: &CStr) ->
    Option<(RefCtx<'a, H>, &PropInfo<'static, H>)> {
        let refctx = RefCtx::new(self, ctx)?;
        let pinfo = refctx.iinfo.props.iter().find(|x| x.name.as_cstr() == propname)?;
        Some((refctx, pinfo))
    }
*/
    pub (super) fn prop_lookup_mut<'a>(&'a mut self, path: &CStr, iname: &CStr, propname: &CStr) ->
    Option<(&'a mut PropInfo<'static, H>, &'a mut Path<H>)> {
        let entry = self.reg.get_mut(iname)?;
        let propinfo = entry.info.props.iter_mut().find(|x| x.name.as_cstr() == propname)?;
        let path = self.paths.get_mut(path)?;
        Some((propinfo, path))
    }

    fn new_noprops(reg_default: bool) -> Self where DBusIntrospectable: PathData<H::Iface> {
        let mut cr = Crossroads {
            reg: BTreeMap::new(),
            paths: BTreeMap::new(),
        };
        if reg_default {
            DBusIntrospectable::register(&mut cr);
            cr.insert(Path::new("/"));
        }
        cr
    }

    pub (super) fn dispatch_ref(&self, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr> {
        let refctx = RefCtx::new(self, ctx)?;
        let entry = self.reg.get(ctx.iface.as_cstr()).ok_or_else(|| { MethodErr::no_interface(&ctx.iface) })?;
        let minfo = entry.info.methods.iter().find(|x| x.name() == &ctx.member)
            .ok_or_else(|| { MethodErr::no_interface(&ctx.member) })?;
        Ok(H::call_method_ref(&minfo.handler(), ctx, &refctx))
    }

    fn post_dispatch<C: channel::Sender>(&self, ctx: MsgCtx, reply: Result<Option<Message>, MethodErr>, c: &C) {
        let r = reply.unwrap_or_else(|e| Some(e.to_message(&ctx.message)));
        if let Some(reply) = r {
            let _ = c.send(reply);
        }
        for retmsg in ctx.send_extra.into_iter() { let _ = c.send(retmsg); }
    }

    /// Handles an incoming message. Returns false if the message was broken somehow
    pub fn dispatch<C: channel::Sender>(&mut self, msg: &Message, c: &C) -> Result<(), ()> {
        let mut ctx = MsgCtx::new(msg).ok_or(())?;
        let r = H::call_method_mut(self, &mut ctx);
        self.post_dispatch(ctx, r, c);
        Ok(())
    }
}

impl Crossroads<()> {
    /// Creates a new instance which is Send but not Sync.
    pub fn new(reg_default: bool) -> Self {
        let mut cr = Self::new_noprops(reg_default);
        if reg_default { DBusProperties::register(&mut cr); }
        cr
    }

    pub fn start<C>(mut self, connection: &C) -> u32
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool + Send>> + channel::Sender
    {
        let mut mr = MatchRule::new();
        mr.msg_type = Some(MessageType::MethodCall);
        connection.start_receive(mr, Box::new(move |msg, c| {
            let _ = self.dispatch(&msg, c);
            true
        }))
    }

}

impl Crossroads<Par> {

    /// Handles an incoming message. Returns err if the message was broken somehow
    pub fn dispatch_par<C: channel::Sender>(&self, msg: &Message, c: &C) -> Result<(), ()> {
        let mut ctx = MsgCtx::new(msg).ok_or(())?;
        let r = self.dispatch_ref(&mut ctx);
        self.post_dispatch(ctx, r, c);
        Ok(())
    }

    /// Creates a new instance which is both Send and Sync.
    pub fn new_par(reg_default: bool) -> Self {
        let mut cr = Self::new_noprops(reg_default);
        if reg_default { DBusProperties::register_par(&mut cr); }
        cr
    }

    pub fn start_par<C, CC, CR>(cr: CR, connection: CC) -> u32
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool + Send + Sync>> + channel::Sender,
        CC: Deref<Target=C>,
        CR: Deref<Target=Self> + Send + Sync + 'static,
    {
        let mut mr = MatchRule::new();
        mr.msg_type = Some(MessageType::MethodCall);
        connection.start_receive(mr, Box::new(move |msg, c| {
            let _ = cr.dispatch_par(&msg, c);
            true
        }))
    }
}

impl Crossroads<handlers::Local> {
    pub fn new_local(reg_default: bool) -> Self {
        let mut cr = Self::new_noprops(reg_default);
        if reg_default { DBusProperties::register_local(&mut cr); }
        cr
    }

    pub fn start_local<C>(mut self, connection: &C) -> u32
    where
        C: channel::MatchingReceiver<F=Box<dyn FnMut(Message, &C) -> bool>> + channel::Sender
    {
        let mut mr = MatchRule::new();
        mr.msg_type = Some(MessageType::MethodCall);
        connection.start_receive(mr, Box::new(move |msg, c| {
            let _ = self.dispatch(&msg, c);
            true
        }))
    }

}


#[cfg(test)]
mod test {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn test_send_sync() {
        fn is_send<T: Send>(_: &T) {}
        fn is_sync<T: Sync>(_: &T) {}
        let c = Crossroads::new_par(true);
        dbg!(&c);
        is_send(&c);
        is_sync(&c);

        let c2 = Crossroads::new(true);
        is_send(&c2);
   }

    #[test]
    fn cr_local() {
        fn dispatch_helper<H: Handlers>(cr: &mut Crossroads<H>, mut msg: Message) -> Message {
            crate::message::message_set_serial(&mut msg, 57);
            let r = RefCell::new(vec!());
            cr.dispatch(&msg, &r).unwrap();
            let mut r = r.into_inner();
            assert_eq!(r.len(), 1);
            r[0].as_result().unwrap();
            r.into_iter().next().unwrap()
        }

        let mut cr = Crossroads::new_local(true);

        struct Score(u16);

        let mut call_times = 0u32;
        cr.register::<Score,_>("com.example.dbusrs.crossroads.score")
            .annotate("com.example.dbusrs.whatever", "Funny annotation")
            .method("UpdateScore", ("change",), ("new_score", "call_times"), move |_: &mut MsgCtx, score: &mut Score, (change,): (u16,)| {
                score.0 += change;
                call_times += 1;
                Ok((score.0, call_times))
            }).deprecated();

        let mut pdata = Path::new("/");
        pdata.insert(Score(7u16));
        cr.insert(pdata);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "com.example.dbusrs.crossroads.score", "UpdateScore").unwrap();
        let r = dispatch_helper(&mut cr, msg.append1(5u16));
        let (new_score, call_times): (u16, u32) = r.read2().unwrap();
        assert_eq!(new_score, 12);
        assert_eq!(call_times, 1);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
        let r = dispatch_helper(&mut cr, msg);
        let xml_data: &str = r.read1().unwrap();
        println!("{}", xml_data);
        // assert_eq!(xml_data, "mooh");
    }


    #[test]
    fn cr_par() {
        fn dispatch_helper(cr: &Crossroads<Par>, mut msg: Message) -> Message {
            crate::message::message_set_serial(&mut msg, 57);
            let r = RefCell::new(vec!());
            cr.dispatch_par(&msg, &r).unwrap();
            let mut r = r.into_inner();
            assert_eq!(r.len(), 1);
            r[0].as_result().unwrap();
            r.into_iter().next().unwrap()
        }

        let mut cr = Crossroads::new_par(true);
        use std::sync::Mutex;
        use crate::arg;
        use crate::arg::Variant;

        struct Score(u16, Mutex<u32>);

        cr.register::<Score,_>("com.example.dbusrs.crossroads.score")
            .method("Hello", ("sender",), ("reply",), |score: &Score, _: &mut MsgCtx, _: &RefCtx<_>, (sender,): (String,)| {
                assert_eq!(score.0, 7u16);
                Ok((format!("Hello {}, my score is {}!", sender, score.0),))
            })
            .prop_ro("Score", |score: &Score| {
                assert_eq!(score.0, 7u16);
                Ok(score.0)
            }).emits_changed(super::super::info::EmitsChangedSignal::False)
            .prop_rw("Dummy",
                |score: &Score, _: &mut MsgCtx, _: &RefCtx<_>| { Ok(*score.1.lock().unwrap()) },
                |score: &Score, val: u32, _: &mut MsgCtx, _: &RefCtx<_>| { *score.1.lock().unwrap() = val; Ok(false) })
            .signal::<(u16,),_>("ScoreChanged", ("NewScore",));

        let mut pdata = Path::new("/");
        pdata.insert(Score(7u16, Mutex::new(37u32)));
        cr.insert(pdata);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "com.example.dbusrs.crossroads.score", "Hello").unwrap();
        let msg = msg.append1("example");
        let r = dispatch_helper(&cr, msg);
        let rr: String = r.read1().unwrap();
        assert_eq!(&rr, "Hello example, my score is 7!");

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "org.freedesktop.DBus.Properties", "Get").unwrap();
        let msg = msg.append2("com.example.dbusrs.crossroads.score", "Score");
        let r = dispatch_helper(&cr, msg);
        let z: Variant<u16> = r.read1().unwrap();
        assert_eq!(z.0, 7u16);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "org.freedesktop.DBus.Properties", "Set").unwrap();
        let msg = msg.append3("com.example.dbusrs.crossroads.score", "Dummy", Variant(987u32));
        let r = dispatch_helper(&cr, msg);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "org.freedesktop.DBus.Properties", "GetAll").unwrap();
        let msg = msg.append1("com.example.dbusrs.crossroads.score");
        let r = dispatch_helper(&cr, msg);
        let z: HashMap<String, Variant<Box<dyn arg::RefArg>>> = r.read1().unwrap();
        println!("{:?}", z);
        assert_eq!(z.get("Dummy").unwrap().0.as_i64().unwrap(), 987);

        let msg = Message::new_method_call("com.example.dbusrs.crossroads.score", "/", "org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
        let r = dispatch_helper(&cr, msg);
        let xml_data: &str = r.read1().unwrap();
        println!("{}", xml_data);
    }
}
