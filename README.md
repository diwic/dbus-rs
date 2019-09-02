D-Bus bindings for Rust
=======================

[![crates.io](https://img.shields.io/crates/v/dbus.svg)](https://crates.io/crates/dbus)
[![API documentation](https://docs.rs/dbus/badge.svg)](https://docs.rs/dbus)
[![license](https://img.shields.io/crates/l/dbus.svg)](https://crates.io/crates/dbus)

The main dbus crate is fairly mature and the features you need should be all there. Breaking changes can still happen, but not often. If you're currently using 0.6.x of dbus and want to upgrade to 0.7.x, you can read [changes in dbus-rs 0.7](dbus/changes-in-0.7.md).

 * Use `Connection` to connect to the system or session bus.
 * Use `Message` to send and receive messages. Get and append arguments of all types (including Unix Fd), see the [argument guide](dbus/examples/argument_guide.md) for details.
 * Build method dispatching servers using the `tree` module. Standard D-Bus interfaces (introspection, properties, object manager) are supported.

If you have questions or comments that the documentation cannot answer in an easy way, [filing an issue](https://github.com/diwic/dbus-rs/issues) with your question is fine. Pull requests that improve the code, documentation, etc, are welcome!

Additional crates
-----------------

 * [dbus-codegen](http://crates.io/crates/dbus-codegen/) installs a binary tool which generates Rust code from D-Bus XML introspection data. The [readme](https://github.com/diwic/dbus-rs/tree/master/dbus-codegen) contains an introduction to how to use it.
 * [libdbus-sys](http://crates.io/crates/libdbus-sys/) contains the raw FFI bindings to libdbus.
 * [dbus-tokio](http://crates.io/crates/dbus-tokio/) integrates D-Bus with [Tokio](http://tokio.rs). It will be deprecated or rewritten from scratch when Tokio has caught up with `std::future` and async/await. [![API documentation](https://docs.rs/dbus-tokio/badge.svg)](https://docs.rs/dbus-tokio)


Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
let conn = Connection::new_session()?;
let obj = conn.with_path("org.freedesktop.DBus", "/", 5000);
let (names,): (Vec<String>,) = obj.method_call("org.freedesktop.DBus", "ListNames", ())?;
for name in names { println!("{}", name); }
```

You can try a similar example (which has more comments) by running:

    cargo run --example client


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

More examples are available in the [examples](https://github.com/diwic/dbus-rs/tree/master/dbus/examples) directory.

Requirements
============

[Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/). If you run Ubuntu, this translates to Ubuntu 14.04 or later, having the `libdbus-1-dev` and `pkg-config` packages installed while building, and the `libdbus-1-3` package installed while running.

However, if you enable the feature `no-string-validation`, you might be able to build and run with older versions of the D-Bus library. This feature skips an extra check that a specific string (e g a Path, ErrorName etc) conforms to the D-Bus specification, which might also make things a tiny bit faster. But - if you do so, and then actually send invalid strings to the D-Bus library, you might get a panic instead of a proper error.

Cross compiling libdbus might be tricky because it binds to a C library, there are some notes [here](https://github.com/diwic/dbus-rs/blob/master/libdbus-sys/cross_compile.md).

License
=======

Apache 2.0 / MIT dual licensed. Any PR you make is assumed to have this license.
