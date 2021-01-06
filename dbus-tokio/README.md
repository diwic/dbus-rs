Tokio integration for D-Bus
===========================

This crate integrates the [dbus](https://docs.rs/dbus) crate with the [tokio](https://tokio.rs) async framework.

See the [examples](https://github.com/diwic/dbus-rs/tree/master/dbus-tokio/examples) for how to get started.

For server side functionality, you can use the [dbus-crossroads](https://docs.rs/dbus-crossroads)
crate to have async method and property handlers.

Invitation
----------

You are hereby invited to participate in the development of dbus-tokio and the other dbus crates:

 * If you have discovered what you believe is a bug, [file an issue](https://github.com/diwic/dbus-rs/issues).
 * If you have questions or comments that the documentation cannot answer in an easy way, [start a discussion](https://github.com/diwic/dbus-rs/discussions).
 * If you have smaller improvements to code, documentation, examples etc, go ahead and [submit a pull request](https://github.com/diwic/dbus-rs/pulls).
   Larger pieces of work are better off discussed first.

The code is Apache 2.0 / MIT dual licensed. Any code submitted in Pull Requests, discussions or issues are assumed to have this license,
unless explicitly stated otherwise.

Requirements
============

Same as for the D-Bus crate: [Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/).
If you run Ubuntu (any maintained version should be okay), this means having the `libdbus-1-dev` and `pkg-config` packages installed while building,
and the `libdbus-1-3` package installed while running.
