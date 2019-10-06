use crate::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use crate::{Message, MessageType};
use super::info::IfaceInfo;
use super::crossroads::Crossroads;
use super::path::Path;
use super::handlers::Handlers;
use std::ffi::CStr;

#[derive(Debug)]
pub struct MsgCtx<'a> {
    pub message: &'a Message,
    pub member: MemberName<'a>,
    pub iface: IfaceName<'a>,
    pub path: PathName<'a>,

    send_extra: Vec<Message>,
}

impl<'a> MsgCtx<'a> {
    pub fn new(msg: &'a Message) -> Option<Self> {
        if msg.msg_type() != MessageType::MethodCall { return None };
        let path = msg.path()?;
        let iface = msg.interface()?;
        let member = msg.member()?;
        Some(MsgCtx { message: msg, member, iface, path, send_extra: vec!() })
    }

    pub fn send_msg(&mut self, msg: Message) { self.send_extra.push(msg); }

}

#[derive(Debug, Clone)]
pub struct RefCtx<'a, H: Handlers> {
    pub crossroads: &'a Crossroads<H>,
    pub path: &'a Path<H>,
    pub (super) iface: &'a H::Iface,
    pub (super) iinfo: &'a IfaceInfo<'static, H>,
}

impl<'a, H: Handlers> RefCtx<'a, H> {

    pub (super) fn new<'b>(cr: &'a Crossroads<H>, ctx: &'b MsgCtx) -> Option<Self> {
        let entry = cr.reg.get(ctx.iface.as_cstr())?;
        let path = cr.paths.get(ctx.path.as_cstr())?;
        let iface = path.get_from_typeid(entry.typeid)?;
        Some(RefCtx { crossroads: cr, path, iface, iinfo: &entry.info })
    }

    pub (super) fn with_iface(&self, ifacename: &CStr) -> Option<Self> {
        let entry = self.crossroads.reg.get(ifacename)?;
        let iface = self.path.get_from_typeid(entry.typeid)?;
        Some(RefCtx { crossroads: self.crossroads, path: self.path, iface, iinfo: &entry.info })
    }

}
