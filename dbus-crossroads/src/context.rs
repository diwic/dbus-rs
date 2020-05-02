use std::sync::Arc;
use crate::{MethodErr, IfaceDesc};

pub struct Context {
    path: dbus::Path<'static>,
    interface: Option<dbus::strings::Interface<'static>>,
    method: dbus::strings::Member<'static>,
    message: Option<dbus::Message>,

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
            message: Some(msg),
            reply: None,
            iface_desc: None,
        })
    }

    pub fn check<R, F: FnOnce(&mut Context) -> Result<R, MethodErr>>(&mut self, f: F) -> Result<R, ()> {
        f(self).map_err(|e| {
            if let Some(msg) = &self.message {
                if !msg.get_no_reply() {
                    self.reply = Some(e.to_message(&msg))
                }
            };
        })
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
}
