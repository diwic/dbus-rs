extern crate dbus;
extern crate xml;

mod generate;

pub use crate::generate::{generate, GenOpts, ServerAccess, ConnectionType};

