A D-Bus binding for rust.
========================

Current state: WIP, but these things should be up and working:
 * Connect to system or session bus
 * Method calls send/receive
 * Signals send/receive
 * Properties, on both client and server sides (set/get/getall methods, but no signals)
 * Server side introspection
 * Server side method dispatch (boxed closures) and property get/set dispatch (trait objects)

[API Documentation](http://diwic.github.io/dbus-rs-docs/dbus/)

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
let c = Connection::get_private(BusType::Session).unwrap();
let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
let r = c.send_with_reply_and_block(m, 2000).unwrap();
let reply = r.get_items();
println!("{}", reply);
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
let f = Factory::new_fn();
let tree = f.tree().add(f.object_path("/hello").introspectable().add(
    f.interface("com.example.dbustest").add_m(
        f.method("Hello", |m,_,_| {
            let s = format!("Hello {}!", m.sender().unwrap());
            Ok(vec!(m.method_return().append(s)))
        }).out_arg(("reply", "s"))
    )
));
tree.set_registered(&c, true).unwrap();
for _ in tree.run(&c, c.iter(1000)) {}
```

You can try a similar example by running:

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
