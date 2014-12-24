A DBus binding for rust.

Current state: WIP, but basic things should be up and working.

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

    let c = Connection::get_private(BusType::Session).unwrap();
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let mut r = c.send_with_reply_and_block(m, 2000).unwrap();
    let reply = r.get_items();
    println!("{}", reply);

You can try a similar example by running:

    cargo run --example client


Server
------

This example grabs the com.example.test bus name and listens to method calls on the /hello path.

    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.test", NameFlag::ReplaceExisting as u32).unwrap();
    c.register_object_path("/hello").unwrap();
    for n in c.iter(1000) {
        match n {
            ConnectionItem::MethodCall(mut m) => /* Handle incoming method call */,
            _ => {},
        }
    }

You can try a similar example by running:

    cargo run --example server


Properties
----------

This example gets the current version of the Policykit backend.

    let c = Connection::get_private(BusType::System).unwrap();
    let p = Props::new(&c, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority", 10000);
    let v = p.get("BackendVersion").unwrap();

You can try a this example by running:

    cargo run --example properties

License
=======

Apache / MIT dual licensed.
