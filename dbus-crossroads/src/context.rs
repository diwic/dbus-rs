use dbus::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use dbus::{Message, MessageType};
use super::info::IfaceInfo;
use super::crossroads::Crossroads;
use super::path::Path;
use super::handlers::Handlers;
use super::MethodErr;
use super::stdimpl::DBusSignals;
use std::ffi::CStr;
use dbus::arg::{AppendAll, IterAppend};

#[derive(Debug)]
enum MsgCow<'a> {
    Borrowed(&'a Message),
    Owned(Message),
}

#[derive(Debug)]
pub struct MsgCtx<'a> {
    msg: Option<MsgCow<'a>>,
    member: MemberName<'a>,
    iface: Option<IfaceName<'a>>,
    path: PathName<'a>,
    pub (super) signals: DBusSignals,
    pub (super) reply: Option<Message>,
    pub (super) send_extra: Vec<Message>,
}

impl MsgCtx<'static> {
    pub fn new(msg: Message) -> Option<Self> {
        if msg.msg_type() != MessageType::MethodCall { return None };
        let path = msg.path()?.into_static();
        let iface = msg.interface().map(|x| x.into_static());
        let member = msg.member()?.into_static();
        Some(MsgCtx { msg: Some(MsgCow::Owned(msg)), member, iface, path,
            send_extra: vec!(),
            signals: Default::default(),
            reply: None
        })
    }

    pub fn new_without_message(path: PathName<'static>, iface: Option<IfaceName<'static>>, member: MemberName<'static>) -> Self {
        MsgCtx { msg: None, member, iface, path, send_extra: vec!(), signals: Default::default(), reply: None }
    }
}


impl<'a> MsgCtx<'a> {

    pub fn message(&self) -> &Message {
        match self.msg {
            Some(MsgCow::Owned(ref r)) => r,
            Some(MsgCow::Borrowed(r)) => r,
            None => panic!("No message"),
        }
    }

    pub fn member(&self) -> &MemberName<'a> { &self.member }
    pub fn path(&self) -> &PathName<'a> { &self.path }
    // FIXME
    pub fn interface(&self) -> &IfaceName<'a> { self.iface.as_ref().unwrap() }

    pub fn send_msg(&mut self, msg: Message) { self.send_extra.push(msg); }

    pub fn make_signal<'b, A: AppendAll, N: Into<MemberName<'b>>>(&self, name: N, args: A) -> Message {
        let mut msg = Message::signal(&self.path, self.iface.as_ref().unwrap(), &name.into());
        args.append(&mut IterAppend::new(&mut msg));
        msg
    }

    pub fn dbus_signals_mut(&mut self) -> &mut DBusSignals { &mut self.signals }

    pub (crate) fn do_reply<F: FnOnce(&mut Message, &mut Self)>(&mut self, f: F) {
        if self.message().get_no_reply() { return; }
        let mut m = self.message().method_return();
        f(&mut m, self);
        self.reply = Some(m);
    }
}

pub struct AsyncMsgCtx {

}

pub struct AsyncMsgReply {

}

impl AsyncMsgCtx {
    pub fn send_msg(&mut self, msg: Message) { unimplemented!() }

    pub fn make_signal<'b, A: AppendAll, N: Into<MemberName<'b>>>(&self, name: N, args: A) -> Message {
        unimplemented!()
    }

    pub fn message(&self) -> &Message { unimplemented!() }

    pub fn iface(&self) -> Option<IfaceName<'static>> { unimplemented!() }

    pub fn custom_reply(self, r: Option<Message>) -> AsyncMsgReply { unimplemented!() }

    pub fn reply<A: AppendAll>(self, a: A) -> AsyncMsgReply { unimplemented!() }

    pub fn err<I: Into<MethodErr>>(self, m: I) -> AsyncMsgReply { unimplemented!() }

    pub (crate) fn iface_typeid(&self) -> Option<std::any::TypeId> { todo!() }
}

#[derive(Debug, Clone)]
pub struct RefCtx<'a, H: Handlers> {
    pub crossroads: &'a Crossroads<H>,
    pub path: &'a Path<H>,
    pub (super) iface: &'a H::Iface,
    pub (super) iinfo: &'a IfaceInfo<'static, H>,
}

impl<'a, H: Handlers> RefCtx<'a, H> {

    pub (super) fn new<'b>(cr: &'a Crossroads<H>, ctx: &'b MsgCtx) -> Result<Self, MethodErr> {
        let path = cr.paths.get(ctx.path.as_cstr()).ok_or_else(|| { MethodErr::no_path(&ctx.path) })?;
        let entry = cr.reg.get(ctx.iface.as_ref().unwrap().as_cstr()).ok_or_else(|| { MethodErr::no_interface(&ctx.iface.as_ref().unwrap()) })?;
        let iface = path.get_from_typeid(entry.typeid).ok_or_else(|| { MethodErr::no_interface(&ctx.iface.as_ref().unwrap()) })?;
        Ok(RefCtx { crossroads: cr, path, iface, iinfo: &entry.info })
    }

    pub (super) fn with_iface(&self, ifacename: &CStr) -> Option<Self> {
        let entry = self.crossroads.reg.get(ifacename)?;
        let iface = self.path.get_from_typeid(entry.typeid)?;
        Some(RefCtx { crossroads: self.crossroads, path: self.path, iface, iinfo: &entry.info })
    }

}
