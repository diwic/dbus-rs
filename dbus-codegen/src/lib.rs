extern crate xml;
#[cfg(feature = "dbus")]
extern crate dbus;

mod generate;

pub use crate::generate::{generate, ConnectionType, GenOpts, ServerAccess};
