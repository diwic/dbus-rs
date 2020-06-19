
use dbus::arg::Arg;
use dbus::arg::Append;
use std::marker::PhantomData;
use dbus::arg::AppendAll;
use dbus::channel::Sender;
use std::sync::Arc;
use std::fmt;
use crate::{MethodErr};
use crate::stdimpl::PropCtx;

struct Dbg<T>(T);

impl<T> fmt::Debug for Dbg<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

#[derive(Debug)]
pub struct Context {
    path: dbus::Path<'static>,
    interface: Option<dbus::strings::Interface<'static>>,
    method: dbus::strings::Member<'static>,
    message: dbus::Message,

    prop_ctx: Option<PropCtx>,
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
            prop_ctx: None,
            send_on_drop: None,
            send_extra: vec!(),
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
    pub fn reply_ok<OA: AppendAll>(&mut self, oa: OA) -> PhantomData<OA> {
        self.do_reply(|msg| { msg.append_all(oa); });
        PhantomData
    }

    /// Reply to a "get" result.
    /// This is what you'll normally have last in your async get property handler.
    ///
    /// Returns PhantomData just to aid the type system.
    pub (crate) fn reply_get<A: Arg + Append>(&mut self, a: A) -> PhantomData<(A, ())> {
        self.prop_ctx().add_get_result(a);
        PhantomData
    }

    /// Replies to the incoming message with an error.
    pub fn reply_err(&mut self, err: MethodErr) {
        if !self.message.get_no_reply() {
            self.reply = Some(err.to_message(&self.message))
        };
    }

    pub fn set_reply(&mut self, msg: Option<dbus::Message>, check_no_reply: bool, check_set: bool) {
        if check_no_reply && self.message.get_no_reply() { return; }
        if check_set && self.reply.is_some() { return; }
        self.reply = msg;
    }

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

    pub fn path(&self) -> &dbus::Path<'static> { &self.path }
    pub fn interface(&self) -> Option<&dbus::strings::Interface<'static>> { self.interface.as_ref() }
    pub fn method(&self) -> &dbus::strings::Member<'static> { &self.method }
    pub fn message(&self) -> &dbus::Message { &self.message }

    pub fn has_reply(&self) -> bool { self.reply.is_some() }

    pub (crate) fn take_prop_ctx(&mut self) -> PropCtx { self.prop_ctx.take().unwrap() }
    pub (crate) fn give_prop_ctx(&mut self, p: PropCtx) { self.prop_ctx = Some(p); }
    pub (crate) fn prop_ctx(&mut self) -> &mut PropCtx { self.prop_ctx.as_mut().unwrap() }
    pub (crate) fn set_on_drop(&mut self, value: Arc<dyn Sender + Send + Sync>) {
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
