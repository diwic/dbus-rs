use std::marker::PhantomData;
use dbus::arg::AppendAll;
use dbus::channel::Sender;
use std::sync::Arc;
use crate::{MethodErr, utils::Dbg};

/// Context is the struct that accompanies you through your method call handler,
/// providing helpful information about the message sent from the client, as well as
/// some methods to send extra messages (typically signals) in return.
#[derive(Debug)]
pub struct Context {
    path: dbus::Path<'static>,
    interface: Option<dbus::strings::Interface<'static>>,
    method: dbus::strings::Member<'static>,
    message: dbus::Message,

    has_error: bool,
    reply: Option<dbus::Message>,
    send_extra: Vec<dbus::Message>,
    send_on_drop: Option<Dbg<Arc<dyn Sender + Send + Sync>>>,
}

impl Context {
    /// Creates a new Context.
    ///
    /// Usually you're not creating your own context, as the crossroads instance is creating one for you.
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
            send_on_drop: None,
            send_extra: vec!(),
            has_error: false,
        })
    }

    /// Convenience method that sets an error reply if the closure returns an error.
    pub fn check<R, F: FnOnce(&mut Context) -> Result<R, MethodErr>>(&mut self, f: F) -> Result<R, ()> {
        f(self).map_err(|e| { self.reply_err(e); })
    }

    /// If the reply is not already set, creates a new method return message and calls the closure
    /// so that the closure can fill in the arguments.
    pub fn do_reply<F: FnOnce(&mut dbus::Message)>(&mut self, f: F) {
        if self.message.get_no_reply() { return; }
        if self.reply.is_some() { return; }
        let mut msg = self.message.method_return();
        f(&mut msg);
        self.reply = Some(msg);
    }

    /// Replies ok to the incoming message, if the reply is not already set.
    /// This is what you'll normally have last in your async method.
    ///
    /// Returns PhantomData just to aid the type system.
    pub (crate) fn reply_ok<OA: AppendAll>(&mut self, oa: OA) -> PhantomData<OA> {
        self.do_reply(|msg| { msg.append_all(oa); });
        PhantomData
    }

    /// Replies to the incoming message, if the reply is not already set.
    /// This is what you'll normally have last in your async method.
    ///
    /// Returns PhantomData just to aid the type system.
    pub fn reply<OA: AppendAll>(&mut self, result: Result<OA, MethodErr>) -> PhantomData<OA> {
        match result {
            Ok(oa) => { self.reply_ok(oa); },
            Err(e) => { self.reply_err(e); },
        };
        PhantomData
    }

    /// Replies to the incoming message with an error.
    pub (crate) fn reply_err(&mut self, err: MethodErr) {
        self.has_error = true;
        if !self.message.get_no_reply() {
            self.reply = Some(err.to_message(&self.message))
        };
    }

    /// Low-level function to set a reply
    ///
    /// You should probably prefer do_reply, or reply_ok / reply_err for async methods.
    pub fn set_reply(&mut self, msg: Option<dbus::Message>, check_no_reply: bool, check_set: bool) {
        if check_no_reply && self.message.get_no_reply() { return; }
        if check_set && self.reply.is_some() { return; }
        self.reply = msg;
    }

    /// Low-level function to flush set messages
    ///
    /// This is called internally, you should probably not use it.
    pub fn flush_messages<S: dbus::channel::Sender + ?Sized>(&mut self, conn: &S) -> Result<(), ()> {
        if let Some(msg) = self.reply.take() {
            conn.send(msg)?;
        }
        for msg in self.send_extra.drain(..) {
            conn.send(msg)?;
        }
        Ok(())
    }

    /// Makes a new signal with the current interface and path
    pub fn make_signal<'b, A, N>(&self, name: N, args: A) -> dbus::Message
    where A: dbus::arg::AppendAll, N: Into<dbus::strings::Member<'b>> {
        let mut msg = dbus::Message::signal(&self.path, self.interface.as_ref().unwrap(), &name.into());
        msg.append_all(args);
        msg
    }

    /// Adds an extra message to send together with the message reply, e g, a custom signal.
    pub fn push_msg(&mut self, msg: dbus::Message) { self.send_extra.push(msg); }

    /// The current object path.
    pub fn path(&self) -> &dbus::Path<'static> { &self.path }

    /// The current interface name.
    ///
    /// The D-Bus specfication allows for the interface to be unspecified, hence this returns an
    /// option. This is very rarely used in practice.
    pub fn interface(&self) -> Option<&dbus::strings::Interface<'static>> { self.interface.as_ref() }

    /// The current method name.
    pub fn method(&self) -> &dbus::strings::Member<'static> { &self.method }

    /// The message that caused this method to be called.
    pub fn message(&self) -> &dbus::Message { &self.message }

    /// True if a reply (error or method return) has been set.
    pub fn has_reply(&self) -> bool { self.reply.is_some() }

    /// Returns true is "reply_err" has been called, or "check" ever returned an error
    pub fn has_error(&self) -> bool { self.has_error }

    pub (crate) fn set_send_on_drop(&mut self, value: Arc<dyn Sender + Send + Sync>) {
        self.send_on_drop = Some(Dbg(value));
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Some(sender) = self.send_on_drop.take() {
            let _ = self.flush_messages(&*sender.0);
        }
    }
}
