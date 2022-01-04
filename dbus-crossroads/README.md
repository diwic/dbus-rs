[![crates.io](https://img.shields.io/crates/v/dbus-crossroads.svg)](https://crates.io/crates/dbus-crossroads)
[![API documentation](https://docs.rs/dbus-crossroads/badge.svg)](https://docs.rs/dbus-crossroads)
[![license](https://img.shields.io/crates/l/dbus-crossroads.svg)](https://crates.io/crates/dbus-crossroads)

dbus-crossroads is a library that helps you implement interfaces on object paths, i e
"server side" method handler code.

It is not as mature as `dbus::tree` yet, but the API should be fairly stable and easier
to use than `dbus::tree`. Go ahead and use it, and report any issues you find!

To get started, you can jump into the commented examples,
one for [sync](https://github.com/diwic/dbus-rs/blob/master/dbus-crossroads/examples/server_cr.rs)
one for [async (dbus-tokio)](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_server_cr.rs),
and one [slightly more advanced](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_adv_server_cr.rs),
or familiarize yourself using [the API reference](https://docs.rs/dbus-crossroads).

Design decisions (compared to `dbus::tree`):

 * First class support for both sync and async methods
 * Detect method arguments automatically through generics
 * ...but less generics in structs you use, which means less cluttered API
 * The tree/crossroads instance is `Send` (but not `Sync`).
 * You can now modify the instance from within a method handler
 * It is objects (paths) that can contain custom data, and they can contain different data for different objects
 * Interface descriptions are kept in a registry, which means less reference counting
