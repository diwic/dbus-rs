use std::{fmt, cell, ops};
use std::any::Any;
use crate::{arg, Message};
use crate::arg::{ReadAll, AppendAll, Append, IterAppend, Get, Arg};
use crate::strings::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use super::crossroads::Crossroads;
use super::info::{MethodInfo, PropInfo};
use super::path::Path;
use super::MethodErr;
use super::context::{MsgCtx, RefCtx};

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
    type SetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::Iter, &mut MsgCtx) -> Result<bool, MethodErr> + Send + 'static>;
    type Iface = Box<dyn Any + Send>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        SendMethod(SendMethods::AllRef(Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message, r))
        })))
    }

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message> {
        match m.0 {
            SendMethods::MutPath(_) => unreachable!(),
            SendMethods::MutIface(_) => unreachable!(),
            SendMethods::MutCr(_) => unreachable!(),
            SendMethods::AllRef(ref f) => {
                f(ctx, refctx)
            },
        }
    }

    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr> {
        let mut try_ref = false;
        let r = {
            let entry = cr.reg.get_mut(ctx.iface.as_cstr()).ok_or_else(|| { MethodErr::no_interface(&ctx.iface) })?;
            let minfo = entry.info.methods.iter_mut().find(|x| x.name() == &ctx.member)
                .ok_or_else(|| { MethodErr::no_method(&ctx.member) })?;
            match minfo.handler_mut().0 {
                SendMethods::MutPath(ref mut f) => {
                    let mut data = cr.paths.get_mut(ctx.path.as_cstr()).ok_or_else(|| { MethodErr::no_path(&ctx.path) })?;
                    f(&mut data, ctx)
                }
                SendMethods::MutIface(ref mut f) => {
                    let data = cr.paths.get_mut(ctx.path.as_cstr()).ok_or_else(|| { MethodErr::no_path(&ctx.path) })?;
                    let iface = data.get_from_typeid_mut(entry.typeid).ok_or_else(|| { MethodErr::no_interface(&ctx.iface) })?;
                    let iface = &mut **iface;
                    f(iface, ctx)
                },
                SendMethods::AllRef(_) => { try_ref = true; None }
                SendMethods::MutCr(ref f) => { f.box_clone()(cr, ctx) },
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
    type SetProp = Box<dyn Fn(&mut arg::Iter, &mut MsgCtx, &RefCtx<Par>)
        -> Result<bool, MethodErr> + Send + Sync + 'static>;
    type Iface = Box<dyn Any + 'static + Send + Sync>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message, r))
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
    type SetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::Iter, &mut MsgCtx) -> Result<bool, MethodErr> + 'static>;
    type Iface = Box<dyn Any>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        LocalMethod(LocalMethods::AllRef(Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia));
            Some(posthandler(ctx.message, r))
        })))
    }

    fn call_method_ref(m: &Self::Method, ctx: &mut MsgCtx, refctx: &RefCtx<Self>) -> Option<Message> {
        match m.0 {
            LocalMethods::MutPath(_) => unreachable!(),
            LocalMethods::MutIface(_) => unreachable!(),
            LocalMethods::MutCr(_) => unreachable!(),
            LocalMethods::AllRef(ref f) => {
                f(ctx, refctx)
            },
        }
    }

    fn call_method_mut(cr: &mut Crossroads<Self>, ctx: &mut MsgCtx) -> Result<Option<Message>, MethodErr> {
        let mut try_ref = false;
        let r = {
            let entry = cr.reg.get_mut(ctx.iface.as_cstr()).ok_or_else(|| { MethodErr::no_interface(&ctx.iface) })?;
            let minfo = entry.info.methods.iter_mut().find(|x| x.name() == &ctx.member)
                .ok_or_else(|| { MethodErr::no_method(&ctx.member) })?;
            match minfo.handler_mut().0 {
                LocalMethods::MutPath(ref mut f) => {
                    let mut data = cr.paths.get_mut(ctx.path.as_cstr()).ok_or_else(|| { MethodErr::no_path(&ctx.path) })?;
                    f(&mut data, ctx)
                }
                LocalMethods::MutIface(ref mut f) => {
                    let data = cr.paths.get_mut(ctx.path.as_cstr()).ok_or_else(|| { MethodErr::no_path(&ctx.path) })?;
                    let iface = data.get_from_typeid_mut(entry.typeid).ok_or_else(|| { MethodErr::no_interface(&ctx.iface) })?;
                    let iface = &mut **iface;
                    f(iface, ctx)
                },
                LocalMethods::AllRef(_) => { try_ref = true; None }
                LocalMethods::MutCr(ref f) => { f.box_clone()(cr, ctx) },
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

pub struct LocalMethod(LocalMethods);

enum LocalMethods {
    MutPath(Box<dyn FnMut(&mut Path<Local>, &mut MsgCtx) -> Option<Message> + 'static>),
    MutIface(Box<dyn FnMut(&mut (dyn Any), &mut MsgCtx) -> Option<Message> + 'static>),
    AllRef(Box<dyn Fn(&mut MsgCtx, &RefCtx<Local>) -> Option<Message> + 'static>),
    MutCr(Box<dyn MutCrCb<Local> + 'static>),
}

pub struct SendMethod(SendMethods);

enum SendMethods {
    MutPath(Box<dyn FnMut(&mut Path<()>, &mut MsgCtx) -> Option<Message> + Send + 'static>),
    MutIface(Box<dyn FnMut(&mut (dyn Any), &mut MsgCtx) -> Option<Message> + Send + 'static>),
    AllRef(Box<dyn Fn(&mut MsgCtx, &RefCtx<()>) -> Option<Message> + Send + 'static>),
    MutCr(Box<dyn MutCrCb<()> + Send + 'static>),
}

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
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(iface, ctx, refctx, ia));
            Some(posthandler(ctx.message, r))
        })
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<() as Handlers>::Method, ((), IA, OA, I), ((), ())> for F
where F: FnMut(&mut I, &mut MsgCtx, IA) -> Result<OA, MethodErr> + Send + 'static
{
    fn make(mut self) -> <() as Handlers>::Method {
        SendMethod(SendMethods::MutIface(Box::new(move |data, info| {
            let iface: &mut I = data.downcast_mut().unwrap();
            let r = IA::read(&mut info.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(iface, info, ia));
            Some(posthandler(info.message, r))
        })))
    }
}

impl<F: FnOnce(&mut Crossroads<()>, &mut MsgCtx) -> Option<Message> + Send + Clone + 'static> From<F> for SendMethod {
    fn from(f: F) -> Self { SendMethod(SendMethods::MutCr(Box::new(f))) }
}

impl<F: FnOnce(&mut Crossroads<Local>, &mut MsgCtx) -> Option<Message> + Clone + 'static> From<F> for LocalMethod {
    fn from(f: F) -> Self { LocalMethod(LocalMethods::MutCr(Box::new(f))) }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<() as Handlers>::Method, ((), IA, OA, I), (u8, ())> for F
where F: FnMut(&mut Path<()>, &mut MsgCtx, IA) -> Result<OA, MethodErr> + Send + 'static
{
    fn make(mut self) -> <() as Handlers>::Method {
        SendMethod(SendMethods::MutPath(Box::new(move |path, ctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(path, ctx, ia));
            Some(posthandler(ctx.message, r))
        })))
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<() as Handlers>::Method, ((), IA, OA, I), (i8, ())> for F
where F: FnOnce(&mut Crossroads<()>, &mut MsgCtx, IA) -> Result<OA, MethodErr> + Send + Clone + 'static
{
    fn make(self) -> <() as Handlers>::Method {
        SendMethod(SendMethods::MutCr(Box::new(move |cr, ctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(cr, ctx, ia));
            Some(posthandler(ctx.message, r))
        })))
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<Local as Handlers>::Method, ((), IA, OA, I), ((), Local)> for F
where F: FnMut(&mut I, &mut MsgCtx, IA) -> Result<OA, MethodErr> + 'static
{
    fn make(mut self) -> <Local as Handlers>::Method {
        LocalMethod(LocalMethods::MutIface(Box::new(move |data, info| {
            let iface: &mut I = data.downcast_mut().unwrap();
            let r = IA::read(&mut info.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(iface, info, ia));
            Some(posthandler(info.message, r))
        })))
    }
}

impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<Local as Handlers>::Method, ((), IA, OA, I), (u8, ())> for F
where F: FnMut(&mut Path<Local>, &mut MsgCtx, IA) -> Result<OA, MethodErr> + 'static
{
    fn make(mut self) -> <Local as Handlers>::Method {
        LocalMethod(LocalMethods::MutPath(Box::new(move |path, ctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(path, ctx, ia));
            Some(posthandler(ctx.message, r))
        })))
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


impl<F, I: 'static + Send + Sync, T: Arg + for<'b> Get<'b>> MakeHandler<<Par as Handlers>::SetProp, (u64, T, I), ((), Par)> for F
where F: Fn(&I, T, &mut MsgCtx, &RefCtx<Par>) -> Result<bool, MethodErr> + Send + Sync + 'static
{
    fn make(self) -> <Par as Handlers>::SetProp {
        Box::new(move |iter, ctx, refctx| {
            let iface: &I = refctx.path.get().unwrap();
            self(iface, iter.read()?, ctx, refctx)
        })
    }
}
