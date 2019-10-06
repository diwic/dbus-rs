//! Will eventually superseed the "tree" module. It's unstable and experimental for now.
#![allow(unused_imports, dead_code, missing_docs, unused_variables)]

mod info;
mod handlers;
mod crossroads;
mod stdimpl;
mod path;
mod context;

pub use crate::tree::MethodErr as MethodErr;

// pub use self::info::{IfaceInfo, MethodInfo, PropInfo};

pub use self::crossroads::{Crossroads};

pub use self::path::{Path, PathData};

pub use self::handlers::{Handlers, Par, Local};

pub use self::context::{MsgCtx, RefCtx};
