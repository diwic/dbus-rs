//! dbus-crossroads is a framework for quickly implementing interfaces on object paths, i e
//! server side.
//!
//! When complete, it will supersede the dbus::tree module.
//!
//! You can jump into the commented examples, one for [sync](https://github.com/diwic/dbus-rs/blob/master/dbus-crossroads/examples/server_cr.rs)
//! and one for [async (dbus-tokio)](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio02_server_cr.rs),
//! or familiarize yourself using this API reference.

mod context;
mod crossroads;
mod ifacedesc;
mod stdimpl;

pub use dbus::tree::MethodErr as MethodErr;

pub use context::Context;
pub use crossroads::{Crossroads, IfaceToken};

pub use ifacedesc::{MethodDesc, SignalDesc, IfaceBuilder, PropBuilder};

#[cfg(test)]
mod test;
