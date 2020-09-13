//! dbus-crossroads is a framework for quickly implementing interfaces on object paths, i e
//! server side.
//!
//! To get started, you can jump into the commented examples,
//! one for [sync](https://github.com/diwic/dbus-rs/blob/master/dbus-crossroads/examples/server_cr.rs)
//! one for [async (dbus-tokio)](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio02_server_cr.rs),
//! and one [slightly more advanced](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_adv_server_cr.rs),
//! or familiarize yourself using this API reference.

mod context;
mod crossroads;
mod ifacedesc;
mod stdimpl;

pub use dbus::MethodErr as MethodErr;

pub use context::Context;
pub use stdimpl::PropContext;
pub use crossroads::{Crossroads, IfaceToken};

pub use ifacedesc::{MethodDesc, SignalDesc, IfaceBuilder, PropBuilder};

#[cfg(test)]
mod test;

mod utils {
    use std::fmt;
    pub (crate) struct Dbg<T>(pub T);

    impl<T> fmt::Debug for Dbg<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "...") }
    }
}
