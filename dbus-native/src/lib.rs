#![deny(unsafe_code)]


/// This is a low-level crate meant for use by the dbus crate.
///
/// No stability guarantees for this crate.

pub mod machineid;

pub mod address;

pub mod authentication;

pub mod message;

pub mod types;

#[allow(unsafe_code)]
mod sys;
