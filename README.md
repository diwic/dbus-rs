A DBus binding for rust.

Current state: WIP, but these things should be up and working:
 * Connect to system or session bus
 * Method calls send/receive
 * Signals send/receive
 * Properties, on both client and server sides (set/get/getall methods, but no signals)
 * Server side introspection
 * Server side method dispatch (boxed closures) and property get/set dispatch (trait objects)

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

This example grabs the com.example.test bus name, registers the /hello path and adds a method which returns a string.
It then listens for incoming D-Bus events and handles them accordingly.

    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.test", NameFlag::ReplaceExisting as u32).unwrap();

    let mut o = ObjectPath::new(&c, "/hello", true);
    o.insert_interface("com.example.test", Interface::new(
        vec!(Method::new("Hello", vec!(),
            vec!(Argument::new("reply", "s")),
            Box::new(|msg| Ok(vec!(MessageItem::Str(format!("Hello {}!", msg.sender().unwrap())))))
        )),
        vec!(), vec!()
    ));
    o.set_registered(true).unwrap();

    for n in c.iter(1000) {
        match n {
            ConnectionItem::MethodCall(mut m) => {
                o.handle_message(&mut m);
            },
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

For an extended example, which also uses non-panicing error handling, see

    examples/rtkit.rs


License
=======

Apache / MIT dual licensed.
