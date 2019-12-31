#![warn(missing_docs)]
//! Tokio integration for dbus
//!
//! This crate contains code for interfacing `dbus` with `tokio`, enabling async/non-blocking operation.
//!
//! This crate contains only the connection components, most of the async code is in the `dbus::nonblock`
//! module, where you can find additional information.
//!
//! There are some examples in the examples directory to help you get started.

pub mod connection;
