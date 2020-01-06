use std::{fmt, cell, ops};
use std::any::Any;
use dbus::{arg, Message};
use dbus::arg::{ReadAll, AppendAll, Append, IterAppend, Get, Arg};
use dbus::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use super::crossroads::Crossroads;
use super::info::{MethodInfo, PropInfo};
use super::path::Path;
use super::MethodErr;
use super::context::{MsgCtx, RefCtx, AsyncMsgCtx, AsyncMsgReply};
use std::future::Future;

pub struct DebugMethod<H: Handlers>(pub H::Method);
impl<H: Handlers> fmt::Debug for DebugMethod<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

pub struct DebugProp<H: Handlers>(pub Option<H::GetProp>, pub Option<H::SetProp>);
impl<H: Handlers> fmt::Debug for DebugProp<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
}

pub trait Handlers: Sized {
    type Method;
    type GetProp;
    type SetProp;
    type Iface: ops::Deref + 'static;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static;

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message>;
    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr>;
}

// The "default" - Method handlers that are "Send" but not "Sync"
impl Handlers for () {
    type Method = SendMethod;
    type GetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr> + Send + 'static>;
    type SetProp = Box<dyn FnMut(&mut Path<Self>, Box<dyn arg::RefArg>, &mut MsgCtx) -> Result<Option<Box<dyn arg::RefArg>>, MethodErr> + Send + 'static>;
    type Iface = Box<dyn Any + Send>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        SendMethod(SendMethods::AllRef(Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message().iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message(), r))
        })))
    }

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message> {
        match m.0 {
            SendMethods::MutPath(_) => unreachable!(),
            SendMethods::MutIface(_) => unreachable!(),
            SendMethods::MutCr(_) => unreachable!(),
            SendMethods::AsyncMutPath(_) => unreachable!(),
            SendMethods::AllRef(ref f) => {
                f(ctx, refctx)
            },
        }
    }

    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr> {
        let mut try_ref = false;
        let r = {
            let entry = cr.reg.get_mut(ctx.interface().as_cstr())
                .ok_or_else(|| { MethodErr::no_interface(ctx.interface()) })?;
            let minfo = entry.info.methods.iter_mut().find(|x| x.name() == ctx.member())
                .ok_or_else(|| { MethodErr::no_method(ctx.member()) })?;
            match minfo.handler_mut().0 {
                SendMethods::MutPath(ref mut f) => {
                    let mut data = cr.paths.get_mut(ctx.path().as_cstr()).ok_or_else(|| { MethodErr::no_path(ctx.path()) })?;
                    f(&mut data, ctx)
                }
                SendMethods::MutIface(ref mut f) => {
                    let data = cr.paths.get_mut(ctx.path().as_cstr()).ok_or_else(|| { MethodErr::no_path(ctx.path()) })?;
                    let iface = data.get_from_typeid_mut(entry.typeid).ok_or_else(|| { MethodErr::no_interface(ctx.interface()) })?;
                    let iface = &mut **iface;
                    f(ctx, iface)
                },
                SendMethods::AllRef(_) => { try_ref = true; None }
                SendMethods::MutCr(ref f) => { f.box_clone()(cr, ctx) },
                SendMethods::AsyncMutPath(_) => todo!(),
            }
        };
        if try_ref { cr.dispatch_ref(ctx) } else { Ok(r) }
    }
}

/// Parallel (Send + Sync) tree - Par
#[derive(Debug, Clone, Copy, Default)]
pub struct Par;

impl Handlers for Par {
    type Method = Box<dyn Fn(&mut MsgCtx, &RefCtx<Self>) -> Option<Message> + Send + Sync + 'static>;
    type GetProp = Box<dyn Fn(&mut arg::IterAppend, &mut MsgCtx, &RefCtx<Par>)
        -> Result<(), MethodErr> + Send + Sync + 'static>;
    type SetProp = Box<dyn Fn(Box<dyn arg::RefArg>, &mut MsgCtx, &RefCtx<Par>)
        -> Result<bool, MethodErr> + Send + Sync + 'static>;
    type Iface = Box<dyn Any + 'static + Send + Sync>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message().iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message(), r))
        })
    }

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message> {
        m(ctx, refctx)
    }

    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>,MethodErr> {
        cr.dispatch_ref(ctx)
    }
}

/// Specifier for mutable and non-Sendable instances of Crossroads. This allows for non-Send method handlers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Local;

impl Handlers for Local {
    type Method = LocalMethod;
    type GetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr> + 'static>;
    type SetProp = Box<dyn FnMut(&mut Path<Self>, Box<dyn arg::RefArg>, &mut MsgCtx) -> Result<Option<Box<dyn arg::RefArg>>, MethodErr> + 'static>;
    type Iface = Box<dyn Any>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        LocalMethod(LocalMethods::AllRef(Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message().iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message(), r))
        })))
    }

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message> {
        match m.0 {
            LocalMethods::MutPath(_) => unreachable!(),
            LocalMethods::MutIface(_) => unreachable!(),
            LocalMethods::MutCr(_) => unreachable!(),
            LocalMethods::AsyncMutPath(_) => unreachable!(),
            LocalMethods::AllRef(ref f) => {
                f(ctx, refctx)
            },
        }
    }

    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr> {
        let mut try_ref = false;
        let r = {
            let entry = cr.reg.get_mut(ctx.interface().as_cstr()).ok_or_else(|| { MethodErr::no_interface(ctx.interface()) })?;
            let minfo = entry.info.methods.iter_mut().find(|x| x.name() == ctx.member())
                .ok_or_else(|| { MethodErr::no_method(ctx.member()) })?;
            match minfo.handler_mut().0 {
                LocalMethods::MutPath(ref mut f) => {
                    let mut data = cr.paths.get_mut(ctx.path().as_cstr()).ok_or_else(|| { MethodErr::no_path(ctx.path()) })?;
                    f(&mut data, ctx)
                }
                LocalMethods::MutIface(ref mut f) => {
                    let data = cr.paths.get_mut(ctx.path().as_cstr()).ok_or_else(|| { MethodErr::no_path(ctx.path()) })?;
                    let iface = data.get_from_typeid_mut(entry.typeid).ok_or_else(|| { MethodErr::no_interface(ctx.interface()) })?;
                    let iface = &mut **iface;
                    f(ctx, iface)
                },
                LocalMethods::AllRef(_) => { try_ref = true; None }
                LocalMethods::MutCr(ref f) => { f.box_clone()(cr, ctx) },
                LocalMethods::AsyncMutPath(_) => todo!(),
            }
        };
        if try_ref { cr.dispatch_ref(ctx) } else { Ok(r) }
    }
}

trait MutCrCb<H: Handlers>: FnOnce(&mut Crossroads<H>, &mut MsgCtx) -> Option<Message> {
    fn box_clone(&self) -> Box<dyn MutCrCb<H>>;
}

impl<H: Handlers, F> MutCrCb<H> for F
where F: FnOnce(&mut Crossroads<H>, &mut MsgCtx) -> Option<Message> + Clone + 'static {
    fn box_clone(&self) -> Box<dyn MutCrCb<H>> {
        Box::new(self.clone())
    }
}

macro_rules! local_and_send_impl {
     ($h: ty, $method: ident, $methods: ident $(, $ss:tt)*) =>  {

pub struct $method($methods);

enum $methods {
    AsyncMutPath(Box<dyn FnMut(AsyncMsgCtx, &mut Path<$h>) -> Box<dyn Future<Output=AsyncMsgReply> $(+ $ss)* + 'static> $(+ $ss)* + 'static>),
    MutPath(Box<dyn FnMut(&mut Path<$h>, &mut MsgCtx) -> Option<Message> $(+ $ss)* + 'static>),
    MutIface(Box<dyn FnMut(&mut MsgCtx, &mut (dyn Any $(+ $ss)*)) -> Option<Message> $(+ $ss)* + 'static>),
    AllRef(Box<dyn Fn(&mut MsgCtx, &RefCtx<$h>) -> Option<Message> $(+ $ss)* + 'static>),
    MutCr(Box<dyn MutCrCb<$h> $(+ $ss)* + 'static>),
}

// AllRef handlers

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, u128)> for F
where F: Fn(&mut MsgCtx, &RefCtx<$h>) -> Option<Message> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        $method($methods::AllRef(Box::new(self)))
    }
}

impl<I: 'static $(+ $ss)*, F> MakeHandler<<$h as Handlers>::Method, ((), I), ($h, f32)> for F
where F: Fn(&mut MsgCtx, &RefCtx<$h>, &I) -> Result<Message, MethodErr> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: &mut MsgCtx, refctx: &RefCtx<$h>| {
            let iface: &I = refctx.path.get().unwrap();
            Some(self(ctx, refctx, iface).unwrap_or_else(|e| e.to_message(ctx.message())))
        })
    }
}

impl<I: 'static $(+ $ss)*, IA: ReadAll, OA: AppendAll, F> MakeHandler<<$h as Handlers>::Method, ((), IA, OA, I), ($h, f64)> for F
where F: Fn(&mut MsgCtx, &RefCtx<$h>, &I, IA) -> Result<OA, MethodErr> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: &mut MsgCtx, refctx: &RefCtx<$h>, i: &I| {
            let ia = IA::read(&mut ctx.message().iter_init())?;
            let r = self(ctx, refctx, i, ia)?;
            let mut m = ctx.message().method_return();
            OA::append(&r, &mut IterAppend::new(&mut m));
            Ok(m)
        })
    }
}


// MutIface handlers

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, i64)> for F
where F: FnMut(&mut MsgCtx, &mut (dyn Any $(+ $ss)*)) -> Option<Message> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        $method($methods::MutIface(Box::new(self)))
    }
}

impl<I: 'static $(+ $ss)*, F> MakeHandler<<$h as Handlers>::Method, ((), I), ($h, i64)> for F
where F: FnMut(&mut MsgCtx, &mut I) -> Result<Message, MethodErr> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: &mut MsgCtx, data: &mut (dyn Any $(+ $ss)*)| {
            let iface: &mut I = data.downcast_mut().unwrap();
            Some(self(ctx, iface).unwrap_or_else(|e| e.to_message(ctx.message())))
        })
    }
}

impl<F, I: 'static $(+ $ss)*, IA: ReadAll, OA: AppendAll> MakeHandler<<$h as Handlers>::Method, ((), IA, OA, I), ($h, i128)> for F
where F: FnMut(&mut MsgCtx, &mut I, IA) -> Result<OA, MethodErr> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: &mut MsgCtx, iface: &mut I| {
            let ia = IA::read(&mut ctx.message().iter_init())?;
            let r = self(ctx, iface, ia)?;
            let mut m = ctx.message().method_return();
            OA::append(&r, &mut IterAppend::new(&mut m));
            Ok(m)
        })
    }
}

// AsyncMutPath

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, (i16, i16))> for F
where F: FnMut(AsyncMsgCtx, &mut Path<$h>) -> Box<dyn Future<Output=AsyncMsgReply> $(+ $ss)* + 'static> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        $method($methods::AsyncMutPath(Box::new(self)))
    }
}

impl<F, R> MakeHandler<<$h as Handlers>::Method, (), ($h, (i16, u16))> for F
where F: FnMut(AsyncMsgCtx, &mut Path<$h>) -> R $(+ $ss)* + 'static,
    R: Future<Output = AsyncMsgReply> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: AsyncMsgCtx, path: &mut Path<$h>| {
            let r = self(ctx, path);
            let r: Box<dyn Future<Output=AsyncMsgReply> $(+ $ss)* + 'static> = Box::new(r);
            r
        })
    }
}


impl<F, R> MakeHandler<<$h as Handlers>::Method, (), ($h, (i16, u8))> for F
where F: FnMut(AsyncMsgCtx, &mut (dyn Any $(+ $ss)*)) -> R $(+ $ss)* + 'static,
    R: Future<Output = AsyncMsgReply> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: AsyncMsgCtx, path: &mut Path<$h>| {
            let iface = path.get_from_typeid_mut(ctx.iface_typeid().unwrap());
            let r: Box<dyn Future<Output=AsyncMsgReply> $(+ $ss)* + 'static> =
                if let Some(iface) = iface {
                    let iface = &mut **iface;
                    let r = self(ctx, iface);
                    Box::new(r)
                } else {
                    Box::new(async {
                        let s = ctx.iface().unwrap_or(dbus::strings::Interface::from(""));
                        ctx.err(MethodErr::no_interface(&s))
                    })
                };
            r
        })
    }
}

impl<I: 'static $(+ $ss)*, F, R> MakeHandler<<$h as Handlers>::Method, ((), I), ($h, (i16, bool))> for F
where F: FnMut(AsyncMsgCtx, &mut I) -> R $(+ $ss)* + 'static,
    R: Future<Output = AsyncMsgReply> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: AsyncMsgCtx, data: &mut (dyn Any $(+ $ss)*)| {
            let iface: &mut I = data.downcast_mut().unwrap();
            self(ctx, iface)
        })
    }
}


impl<I: 'static $(+ $ss)*, IA: ReadAll $(+ $ss)* + 'static, OA: AppendAll, F, R>
   MakeHandler<<$h as Handlers>::Method, ((), IA, OA, I), ($h, (i16, ()))> for F
where F: FnMut(AsyncMsgCtx, &mut I, IA) -> R $(+ $ss)* + 'static,
    R: Future<Output = AsyncMsgReply> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |ctx: AsyncMsgCtx, iface: &mut I| {
            let r = match IA::read(&mut ctx.message().iter_init()) {
                Ok(ia) => Ok(self(ctx, iface, ia)),
                Err(e) => Err(ctx.err(e)),
            };
            async {
                match r {
                    Ok(r) => r.await,
                    Err(e) => e,
                }
            }
        })
    }
}

// MutCr handlers

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, i16)> for F
where F: FnOnce(&mut Crossroads<$h>, &mut MsgCtx) -> Option<Message> $(+ $ss)* + Clone + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        $method($methods::MutCr(Box::new(self)))
    }
}

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, u32)> for F
where F: FnOnce(&mut Crossroads<$h>, &mut MsgCtx) -> Result<Message, MethodErr> $(+ $ss)* + Clone + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |cr: &mut Crossroads<$h>, ctx: &mut MsgCtx| {
            Some(self(cr, ctx).unwrap_or_else(|e| e.to_message(ctx.message())))
        })
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<$h as Handlers>::Method, ((), IA, OA, I), ($h, i32)> for F
where F: FnOnce(&mut Crossroads<$h>, &mut MsgCtx, IA) -> Result<OA, MethodErr> $(+ $ss)* + Clone + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |cr: &mut Crossroads<$h>, ctx: &mut MsgCtx| {
            let ia = IA::read(&mut ctx.message().iter_init())?;
            let r = self(cr, ctx, ia)?;
            let mut m = ctx.message().method_return();
            OA::append(&r, &mut IterAppend::new(&mut m));
            Ok(m)
        })
    }
}

// MutPath handlers

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, u8)> for F
where F: FnMut(&mut Path<$h>, &mut MsgCtx) -> Option<Message> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::Method {
        $method($methods::MutPath(Box::new(self)))
    }
}

impl<F> MakeHandler<<$h as Handlers>::Method, (), ($h, i8)> for F
where F: FnMut(&mut Path<$h>, &mut MsgCtx) -> Result<Message, MethodErr> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |path: &mut Path<$h>, ctx: &mut MsgCtx| {
            Some(self(path, ctx).unwrap_or_else(|e| e.to_message(ctx.message())))
        })
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<$h as Handlers>::Method, ((), IA, OA, I), ($h, u16)> for F
where F: FnMut(&mut Path<$h>, &mut MsgCtx, IA) -> Result<OA, MethodErr> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::Method {
        MakeHandler::make(move |path: &mut Path<$h>, ctx: &mut MsgCtx| -> Result<Message, MethodErr> {
            let ia = IA::read(&mut ctx.message().iter_init())?;
            let r = self(path, ctx, ia)?;
            let mut m = ctx.message().method_return();
            OA::append(&r, &mut IterAppend::new(&mut m));
            Ok(m)
        })
    }
}

// GetProp handlers

impl<F> MakeHandler<<$h as Handlers>::GetProp, i64, ($h, u8)> for F
where F: FnMut(&mut Path<$h>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::GetProp {
        Box::new(self)
    }
}

impl<F, I: 'static $(+ $ss)*, T: Append> MakeHandler<<$h as Handlers>::GetProp, (i64, T, I), ($h, u8)> for F
where F: FnMut(&I, &mut MsgCtx) -> Result<T, MethodErr> $(+ $ss)* + 'static
{
    fn make(mut self) -> <$h as Handlers>::GetProp {
        MakeHandler::make(move |path: &mut Path<$h>, ia: &mut arg::IterAppend, ctx: &mut MsgCtx| {
            let iface: &I = path.get().unwrap();
            self(iface, ctx).map(|r| { ia.append(r); })
        })
    }
}

// SetProp handlers

impl<F> MakeHandler<<$h as Handlers>::SetProp, u64, ($h, u8)> for F
where F: FnMut(&mut Path<$h>, Box<dyn arg::RefArg>, &mut MsgCtx) -> Result<Option<Box<dyn arg::RefArg>>, MethodErr> $(+ $ss)* + 'static
{
    fn make(self) -> <$h as Handlers>::SetProp {
        Box::new(self)
    }
}

impl<F, I: 'static $(+ $ss)*, T> MakeHandler<<$h as Handlers>::SetProp, (u64, T, I), ($h, u8)> for F
where
    F: FnMut(&mut I, &mut MsgCtx, &T) -> Result<Option<T>, MethodErr> $(+ $ss)* + 'static,
    T: Arg + for <'s> Get<'s> + arg::RefArg + 'static,

{
    fn make(mut self) -> <$h as Handlers>::SetProp {
        MakeHandler::make(move |path: &mut Path<$h>, val: Box<dyn arg::RefArg>, ctx: &mut MsgCtx| -> Result<Option<Box<dyn arg::RefArg>>, MethodErr> {
            let iface: &mut I = path.get_mut().unwrap();
            let val: &T = arg::cast(&*val).unwrap();
            let res = self(iface, ctx, val)?;
            Ok(res.map(|x| Box::new(x) as Box<dyn arg::RefArg>))
        })
    }
}

    }
}

local_and_send_impl!(Local, LocalMethod, LocalMethods);
local_and_send_impl!((), SendMethod, SendMethods, Send);

/// Internal helper trait
pub trait MakeHandler<T, A, Dummy> {
    /// Internal helper trait
    fn make(self) -> T;
}

fn posthandler<OA: AppendAll>(msg: &Message, r: Result<OA, MethodErr>) -> Message {
    match r {
        Err(e) => e.to_message(msg),
        Ok(r) => {
            let mut m = msg.method_return();
            OA::append(&r, &mut IterAppend::new(&mut m));
            m
        },
    }
}

// Methods

impl<F, I: 'static + Send + Sync, IA: ReadAll, OA: AppendAll> MakeHandler<<Par as Handlers>::Method, ((), IA, OA, I), ((), Par)> for F
where F: Fn(&I, &mut MsgCtx, &RefCtx<Par>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <Par as Handlers>::Method {
        Box::new(move |ctx, refctx| {
            let iface: &I = refctx.path.get().unwrap();
            let r = IA::read(&mut ctx.message().iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(iface, ctx, refctx, ia));
            Some(posthandler(ctx.message(), r))
        })
    }
}

// For introspection

impl<IA: ReadAll, OA: AppendAll, H: Handlers, F, I> MakeHandler<H::Method, ((), IA, OA, I), (bool, H)> for F
where F: Fn(&mut MsgCtx, &RefCtx<H>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <H as Handlers>::Method { H::make_method(self) }
}

// For getprop

impl<F, I: 'static + Send + Sync, T: Append> MakeHandler<<Par as Handlers>::GetProp, (i64, T, I), ((), Par)> for F
where F: Fn(&I, &mut MsgCtx, &RefCtx<Par>) -> Result<T, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <Par as Handlers>::GetProp {
        Box::new(move |a, ctx, refctx| {
            let iface: &I = refctx.path.get().unwrap();
            self(iface, ctx, refctx).map(|r| { a.append(r); })
        })
    }
}

impl<F, I: 'static + Send + Sync, T: Append> MakeHandler<<Par as Handlers>::GetProp, (i64, T, I), ((), (Par, Par))> for F
where F: Fn(&I) -> Result<T, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <Par as Handlers>::GetProp {
        Box::new(move |a, _, refctx| {
            let iface: &I = refctx.path.get().unwrap();
            self(iface).map(|r| { a.append(r); })
        })
    }
}

// For setprop


impl<F, I: 'static + Send + Sync, T: 'static + Arg + for<'b> Get<'b>> MakeHandler<<Par as Handlers>::SetProp, (u64, T, I), ((), Par)> for F
where F: Fn(&I, &T, &mut MsgCtx, &RefCtx<Par>) -> Result<bool, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <Par as Handlers>::SetProp {
        Box::new(move |val, ctx, refctx| {
            let iface: &I = refctx.path.get().unwrap();
            let val: &T = arg::cast(&*val).unwrap();
            self(iface, val, ctx, refctx)
        })
    }
}
