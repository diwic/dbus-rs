[![crates.io](https://img.shields.io/crates/v/dbus-crossroads.svg)](https://crates.io/crates/dbus-crossroads)
[![API documentation](https://docs.rs/dbus-crossroads/badge.svg)](https://docs.rs/dbus-crossroads)
[![license](https://img.shields.io/crates/l/dbus-crossroads.svg)](https://crates.io/crates/dbus-crossroads)

dbus-crossroads is a library that helps you implement interfaces on object paths, i e
"server side" method handler code.

You can jump into the commented examples, one for [sync](https://github.com/diwic/dbus-rs/blob/master/dbus-crossroads/examples/server_cr.rs)
and one for [async (dbus-tokio)](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio02_server_cr.rs),
or familiarize yourself using the API reference.

When complete, it will supersede the dbus::tree module. Known missing pieces:

 * Asynchronous "get property" handlers (asynchronous methods and set property handlers are implemented)
 * ObjectManager support

Design decisions (compared to dbus::tree):

 * First class support for both sync and async methods
 * Detect method arguments automatically through generics
 * ...but less generics in structs you use, which means less cluttered API
 * The tree/crossroads instance is `Send` (but not `Sync`).
 * You can now modify the instance from within a method handler
 * Different object paths can contain different types of data
 * Interface descriptions are kept in a registry, which means less reference counting
