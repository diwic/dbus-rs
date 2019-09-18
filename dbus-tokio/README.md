Tokio integration for D-Bus
===========================

This is an intermediate version:

The `connection` module uses the `futures-preview` crate, `std::future`, and can bind to both Tokio 0.1 and 0.2.
Run it with the `nightly` feature to have the `connection` module bind to Tokio 0.2, and without this feature
to bind to Tokio 0.1.
The Tokio 0.2 connection is what will be supported in future versions.
At the moment, this is a WIP, and only client side code is up and working.

The other structs (`ADriver`, `AMethodCall`, `tree` module) will bind to Tokio 0.1 and the old `futures` 0.1.
They will be removed when Tokio 0.2 is stable.

See the [examples](https://github.com/diwic/dbus-rs/tree/master/dbus-tokio/examples) for how to get started.

