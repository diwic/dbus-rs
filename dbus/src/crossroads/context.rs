use crate::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use crate::{Message, MessageType};

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
