//! Tokio integration for dbus
//!
//! What's currently working is:
//!
//!  * Client: Make method calls and wait asynchronously for them to be replied to - see `AConnection::method_call`
//!  * Get a stream of incoming messages (so you can listen to signals etc) - see `AConnection::messages`
//!  * Server: Make a tree handle that stream of incoming messages - see `tree::ATreeServer`
//!  * Server: Add asynchronous methods to the tree - in case you cannot reply right away,
//!    you can return a future that will reply when that future resolves - see `tree::AFactory::amethod`
//! 
//! For examples to get you started, see the examples directory and the Readme.

extern crate dbus;
extern crate futures;
extern crate tokio_core;
extern crate mio;

pub mod tree;

mod adriver;

pub use adriver::{AConnection, AMessageStream, AMethodCall};
