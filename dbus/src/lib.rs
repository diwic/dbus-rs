//! D-Bus bindings for Rust
//!
//! [D-Bus](http://dbus.freedesktop.org/) is a message bus, and is mainly used in Linux
//! for communication between processes. It is present by default on almost every
//! Linux distribution out there, and runs in two instances - one per session, and one
//! system-wide.
//!
//! In addition to the API documentation, which you're currently reading, you might want to
//! look in the examples directory, which contains many examples and an argument guide.
//! README.md also contain a few quick "getting started" examples.
//!
//! In addition to this crate, there are two companion crates, dbus-codegen for generating Rust
//! code from D-Bus introspection data, and dbus-tokio for integrating D-Bus with [Tokio](http://tokio.rs).
//! However, at the time of this writing, these are far less mature than this crate. 

#![warn(missing_docs)]

extern crate libc;

#[allow(missing_docs)]
extern crate libdbus_sys as ffi;

pub use crate::message::{Message, MessageType};

pub mod message;

pub mod ffidisp;

mod error;
pub use error::Error;

pub mod channel;

// Not ready for release yet
#[doc(hidden)]
pub mod crossroads;

pub mod blocking;

pub mod nonblock;

pub mod strings;
pub use crate::strings::{Signature, Path};

pub mod arg;

pub mod tree;

static INITDBUS: std::sync::Once = std::sync::Once::new();

use std::ffi::{CString, CStr};
use std::os::raw::c_char;

fn init_dbus() {
    INITDBUS.call_once(|| {
        if unsafe { ffi::dbus_threads_init_default() } == 0 {
            panic!("Out of memory when trying to initialize D-Bus library!");
        }
    });
}

fn c_str_to_slice(c: & *const c_char) -> Option<&str> {
    if c.is_null() { None }
    else { std::str::from_utf8( unsafe { CStr::from_ptr(*c).to_bytes() }).ok() }
}

fn to_c_str(n: &str) -> CString { CString::new(n.as_bytes()).unwrap() }



