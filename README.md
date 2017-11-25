A D-Bus binding for Rust.
========================

Current state of the [dbus](http://crates.io/crates/dbus/) crate: Slowly maturing. Most stuff you need should be working:

 * Connect to system or session bus
 * Messages send/receive (method calls, method returns, signals, errors)
 * Message get/append arguments (through either generics, trait objects or enums), all types (including Unix Fd). See [argument guide](dbus/examples/argument_guide.md).
 * Build server side trees, with introspection and method dispatch (boxed closures)
 * Properties, on both client and server sides (set/get/getall methods, signals)
 * Optional async API (for poll-based mainloops, e g mio)

[API Documentation is here](http://docs.rs/dbus/). If you have further questions or comments, [filing an issue](https://github.com/diwic/dbus-rs/issues) with your question is fine.

Additional crates
-----------------

 * [libdbus-sys](http://crates.io/crates/libdbus-sys/) contains the raw FFI bindings to libdbus.
 * [dbus-codegen](http://crates.io/crates/dbus-codegen/) installs a binary tool which generates Rust code from D-Bus XML introspection data.
 * [dbus-tokio](http://crates.io/crates/dbus-tokio/) integrates D-Bus with [Tokio](http://tokio.rs).

All these crates are less tested and less mature than the main "dbus" crate.

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
let c = Connection::get_private(BusType::Session)?;
let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames")?;
let r = c.send_with_reply_and_block(m, 2000)?;
let arr: Array<&str, _>  = r.get1()?;
for name in arr { println!("{}", name); }
```

You can try a similar example by running:

    cargo run --example client


Server
------

This example grabs the com.example.dbustest bus name, registers the /hello path and adds a method which returns a string.
It then listens for incoming D-Bus events and handles them accordingly.

```rust
let c = Connection::get_private(BusType::Session)?;
c.register_name("com.example.dbustest", NameFlag::ReplaceExisting as u32)?;
let f = Factory::new_fn::<()>();
let tree = f.tree(()).add(f.object_path("/hello", ()).introspectable().add(
    f.interface("com.example.dbustest", ()).add_m(
        f.method("Hello", (), |m| {
            let n: &str = m.msg.read1()?;
            let s = format!("Hello {}!", n);
            Ok(vec!(m.msg.method_return().append1(s)))
        }).inarg::<&str,_>("name")
          .outarg::<&str,_>("reply")
    )
));
tree.set_registered(&c, true)?;
c.add_handler(tree);
loop { c.incoming(1000).next(); }
```

You can try a similar example (which has more comments) by running:

    cargo run --example server

Or a more advanced server example:

    cargo run --example adv_server

Properties
----------

There are two examples of getting properties in the examples directory, one
which uses the newer `arg` style and one that uses the older `MessageItem` style. See:

    cargo run --example properties
    cargo run --example properties_msgitem

For an extended example, which also uses non-panicking error handling, see

    examples/rtkit.rs


Requirements
============

[Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/). If you run Ubuntu, this translates to Ubuntu 14.04 or later, having the `libdbus-1-dev` package installed while building, and the `libdbus-1-3` package installed while running.

However, if you enable the feature `no-string-validation`, you might be able to build and run with older versions of the D-Bus library. This feature skips an extra check that a specific string (e g a Path, ErrorName etc) conforms to the D-Bus specification, which might also make things a tiny bit faster. But - if you do so, and then actually send invalid strings to the D-Bus library, you might get a panic instead of a proper error.


License
=======

Apache 2.0 / MIT dual licensed.
