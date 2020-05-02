use std::sync::Arc;
use crate::{MethodErr, IfaceDesc};

pub struct Context {
    path: dbus::Path<'static>,
    interface: Option<dbus::strings::Interface<'static>>,
    method: dbus::strings::Member<'static>,
    message: dbus::Message,

    pub (crate) iface_desc: Option<Arc<IfaceDesc>>,

    reply: Option<dbus::Message>,
}

impl Context {
    pub fn new(msg: dbus::Message) -> Option<Self> {
        if msg.msg_type() != dbus::MessageType::MethodCall { return None; }
        let p = msg.path()?.into_static();
        let i = msg.interface().map(|i| i.into_static());
        let m = msg.member()?.into_static();
        Some(Context {
            path: p,
            interface: i,
            method: m,
            message: msg,
            reply: None,
            iface_desc: None,
        })
    }

    pub fn check<R, F: FnOnce(&mut Context) -> Result<R, MethodErr>>(&mut self, f: F) -> Result<R, ()> {
        f(self).map_err(|e| {
            if !self.message.get_no_reply() {
                self.reply = Some(e.to_message(&self.message))
            };
        })
    }

    pub fn set_reply<F: FnOnce(&mut dbus::Message)>(&mut self, f: F) {
        if self.message.get_no_reply() { return; }
        if self.reply.is_some() { return; }
        let mut msg = self.message.method_return();
        f(&mut msg);
        self.reply = Some(msg);
    }

    pub fn flush_messages<S: dbus::channel::Sender>(&mut self, conn: &S) -> Result<(), ()> {
        if let Some(msg) = self.reply.take() {
            conn.send(msg)?;
        }
        Ok(())
    }

    pub fn path(&self) -> &dbus::Path<'static> { &self.path }
    pub fn interface(&self) -> Option<&dbus::strings::Interface<'static>> { self.interface.as_ref() }
    pub fn method(&self) -> &dbus::strings::Member<'static> { &self.method }
    pub fn message(&self) -> &dbus::Message { &self.message }
}
