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
}

/// Parallel tree - Par
#[derive(Debug, Clone, Copy, Default)]
pub struct Par;

impl Handlers for Par {
    type Method = Box<dyn Fn(&mut MsgCtx, &RefCtx<Par>) -> Option<Message> + Send + Sync + 'static>;
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

}

/// Mutable, non-Send tree
#[derive(Debug, Clone, Copy, Default)]
pub struct Mut;

impl Handlers for Mut {
    type Method = MutMethod;
    type GetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::IterAppend, &mut MsgCtx) -> Result<(), MethodErr> + 'static>;
    type SetProp = Box<dyn FnMut(&mut Path<Self>, &mut arg::Iter, &mut MsgCtx) -> Result<bool, MethodErr> + 'static>;
    type Iface = Box<dyn Any>;

    fn make_method<IA: ReadAll, OA: AppendAll, F>(f: F) -> Self::Method
    where F: Fn(&mut MsgCtx, &RefCtx<Self>, IA) -> Result<OA, MethodErr> + Send + Sync + 'static {
        MutMethod(MutMethods::AllRef(Box::new(move |ctx, refctx| {
            let r = IA::read(&mut ctx.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| f(ctx, refctx, ia)); 
            Some(posthandler(ctx.message, r))
        })))
    }
}


pub struct MutMethod(pub (super) MutMethods);

pub (super) enum MutMethods {
    MutIface(Box<dyn FnMut(&mut (dyn Any), &mut MsgCtx) -> Option<Message> + 'static>),
    AllRef(Box<dyn Fn(&mut MsgCtx, &RefCtx<Mut>) -> Option<Message> + 'static>),
    MutCr(fn(&mut Crossroads<Mut>, &Message) -> Vec<Message>),
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


impl<F, I: 'static, IA: ReadAll, OA: AppendAll> MakeHandler<<Mut as Handlers>::Method, ((), IA, OA, I), ((), Mut)> for F
where F: FnMut(&mut I, &mut MsgCtx, IA) -> Result<OA, MethodErr> + 'static
{
    fn make(mut self) -> <Mut as Handlers>::Method {
        MutMethod(MutMethods::MutIface(Box::new(move |data, info| {
            let iface: &mut I = data.downcast_mut().unwrap();
            let r = IA::read(&mut info.message.iter_init()).map_err(From::from);
            let r = r.and_then(|ia| self(iface, info, ia)); 
            Some(posthandler(info.message, r))
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


