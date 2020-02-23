D-Bus bindings for Rust
=======================

[![crates.io](https://img.shields.io/crates/v/dbus.svg)](https://crates.io/crates/dbus)
[![API documentation](https://docs.rs/dbus/badge.svg)](https://docs.rs/dbus)
[![license](https://img.shields.io/crates/l/dbus.svg)](https://crates.io/crates/dbus)

The main dbus crate is fairly mature and the features you need should be all there. Breaking changes can still happen, but not often.
If you're currently using 0.6.x of dbus and want to upgrade to later versions, you can read [changes in dbus-rs 0.7](dbus/changes-in-0.7.md).

 * Use `blocking::Connection` to connect to the session or system bus.
 * Use `Message` to send and receive messages. Get and append arguments of all types, see the [argument guide](dbus/examples/argument_guide.md) for details.
 * Build method dispatching servers using the `tree` module. Standard D-Bus interfaces (introspection, properties, object manager) are supported.

If you have questions or comments that the documentation cannot answer in an easy way, [filing an issue](https://github.com/diwic/dbus-rs/issues) with your question is fine. Pull requests that improve the code, documentation, etc, are welcome!

Additional crates
-----------------

 * [dbus-tokio](http://crates.io/crates/dbus-tokio/) integrates D-Bus with [Tokio](http://tokio.rs). [![API documentation](https://docs.rs/dbus-tokio/badge.svg)](https://docs.rs/dbus-tokio)
 * [dbus-codegen](http://crates.io/crates/dbus-codegen/) installs a binary tool which generates Rust code from D-Bus XML introspection data. The [readme](https://github.com/diwic/dbus-rs/tree/master/dbus-codegen) contains an introduction to how to use it.
 * [libdbus-sys](http://crates.io/crates/libdbus-sys/) contains the raw FFI bindings to libdbus.

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
use dbus::blocking::Connection;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // First open up a connection to the session bus.
    let conn = Connection::new_session()?;

    // Second, create a wrapper struct around the connection that makes it easy
    // to send method calls to a specific destination and path.
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/", Duration::from_millis(5000));

    // Now make the method call. The ListNames method call takes zero input parameters and 
    // one output parameter which is an array of strings.
    // Therefore the input is a zero tuple "()", and the output is a single tuple "(names,)".
    let (names,): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ())?;

    // Let's print all the names to stdout.
    for name in names { println!("{}", name); }

    Ok(())
}
```

Examples of client code in the examples directory:

 * [client.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/client.rs) (same as the above)
 * [properties.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/properties.rs)
 * [match_signal.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/match_signal.rs)
 * [rtkit.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/rtkit.rs)

Server
------

This example grabs the `com.example.dbustest` bus name, registers the `/hello` path and adds a method which returns a string.
It then listens for incoming D-Bus events and handles them accordingly.

```rust
let c = Connection::new_session()?;
c.request_name("com.example.dbustest", false, true, false)?;
let f = Factory::new_fn::<()>();
let tree = f.tree(())
    .add(f.object_path("/hello", ()).introspectable()
        .add(f.interface("com.example.dbustest", ())
            .add_m(f.method("Hello", (), |m| {
                let n: &str = m.msg.read1()?;
                let s = format!("Hello {}!", n);
                Ok(vec!(m.msg.method_return().append1(s)))
            }).inarg::<&str,_>("name")
              .outarg::<&str,_>("reply")
        )
    ).add(f.object_path("/", ()).introspectable());
tree.start_receive(&c);
loop { c.process(Duration::from_millis(1000))?; }
```

You can try a similar example (which has more comments) by running:

    cargo run --example server

Or a more advanced server example:

    cargo run --example adv_server

More examples
-------------

More examples are available in the [examples](https://github.com/diwic/dbus-rs/tree/master/dbus/examples) directory.

Features
========

The `futures` feature makes `dbus` depend on the `futures` crate. This enables the `nonblock` module (used by the `dbus-tokio` crate).

The `no-string-validation` feature skips an extra check that a specific string (e g a `Path`, `ErrorName` etc) conforms to the D-Bus specification, which might also make things a tiny bit faster. But - if you do so, and then actually send invalid strings to the D-Bus library, you might get a panic instead of a proper error.

Requirements
============

[Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/). If you run Ubuntu (any maintained version should be okay), this means having the `libdbus-1-dev` and `pkg-config` packages installed while building, and the `libdbus-1-3` package installed while running.

Cross compiling libdbus might be tricky because it binds to a C library, there are some notes [here](https://github.com/diwic/dbus-rs/blob/master/libdbus-sys/cross_compile.md).

License
=======

Apache 2.0 / MIT dual licensed. Any PR you make is assumed to have this license.
