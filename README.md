A D-Bus binding for Rust.
========================

Current state: Slowly maturing. Most stuff you need should be working:
 * Connect to system or session bus
 * Messages send/receive (method calls, method returns, signals, errors)
 * Message get/append arguments (through either generics, trait objects or enums), all types (including Unix Fd). See [argument guide](examples/argument_guide.md).
 * Build server side trees, with introspection and method dispatch (boxed closures)
 * Properties, on both client and server sides (set/get/getall methods, signals on server side)
 * Optional async API (for poll-based mainloops, e g mio)

[API Documentation is here](http://docs.rs/dbus/). If you have further questions or comments, [filing an issue](https://github.com/diwic/dbus-rs/issues) with your question is fine.

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
let c = Connection::get_private(BusType::Session).unwrap();
let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
let r = c.send_with_reply_and_block(m, 2000).unwrap();
let arr: Array<&str, _>  = r.get1().unwrap();
for name in arr { println!("{}", name); }
```

You can try a similar example by running:

    cargo run --example client


Server
------

This example grabs the com.example.dbustest bus name, registers the /hello path and adds a method which returns a string.
It then listens for incoming D-Bus events and handles them accordingly.

```rust
let c = Connection::get_private(BusType::Session).unwrap();
c.register_name("com.example.dbustest", NameFlag::ReplaceExisting as u32).unwrap();
let f = Factory::new_fn::<()>();
let tree = f.tree(()).add(f.object_path("/hello", ()).introspectable().add(
    f.interface("com.example.dbustest", ()).add_m(
        f.method("Hello", (), |m| {
            let s = format!("Hello {}!", m.msg.sender().unwrap());
            Ok(vec!(m.msg.method_return().append1(s)))
        }).outarg::<&str,_>("reply")
    )
));
tree.set_registered(&c, true).unwrap();
for _ in tree.run(&c, c.iter(1000)) {}
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


Tokio integration
=================

In the async/dbus-tokio directory, you'll find a crate for integrating D-Bus with [Tokio](http://tokio.rs).

Code generation
===============

In the codegen directory, you'll find a code generation utility that's still in alpha, but I'd appreciate testing and feedback. Given a destination and a path, the utility will generate Rust traits, and their implementations, for all interfaces found on that path.


License
=======

Apache 2.0 / MIT dual licensed.
