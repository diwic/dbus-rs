#![deny(unsafe_code)]

/// This is a low-level crate meant for use by the dbus crate.

pub mod machineid;

pub mod address;

pub mod authentication;

#[allow(unsafe_code)]
mod sys;
