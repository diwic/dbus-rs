A D-Bus binding for Rust.
========================

Current state: Slowly maturing. Most stuff you need should be working:
 * Connect to system or session bus
 * Messages send/receive (method calls, method returns, signals, errors)
 * Message get/append arguments (through either generics or enums), all types (including Unix Fd).
 * Build server side trees, with introspection and method dispatch (boxed closures)
 * Properties, on both client and server sides (set/get/getall methods, signals on server side)
 * Optional async API (for poll-based mainloops, e g mio)

[API Documentation is here](http://docs.rs/dbus/). If you have further questions or comments, [filing an issue](https://github.com/diwic/dbus-rs/issues) with your question is fine.

Requirements: [Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/). 

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


Properties
----------

This example gets the current version of the Policykit backend.

```rust
let c = Connection::get_private(BusType::System).unwrap();
let p = Props::new(&c, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
    "org.freedesktop.PolicyKit1.Authority", 10000);
let v = p.get("BackendVersion").unwrap();
```

You can try a this example by running:

    cargo run --example properties

For an extended example, which also uses non-panicing error handling, see

    examples/rtkit.rs


License
=======

Apache / MIT dual licensed. 
