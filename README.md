A DBus binding for rust.

Current state: WIP.

Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

    let mut c = Connection::get_private(BusType::Session).unwrap();
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let mut r = c.send_with_reply_and_block(m, 2000).unwrap();
    let reply = r.get_items();
    println!("{}", reply);

Server
------

This example listens to method calls on the /hello path on the session bus.
(In this example, the caller must know the unique_name of the server.)

    let mut c = Connection::get_private(BusType::Session).unwrap();
    c.register_object_path("/hello").unwrap();
    for n in c.iter(1000) {
        match n {
            ConnectionItem::Msg(mut m) => /* Handle incoming message m */,
            _ => {},
        }
    }

License
=======

Apache / MIT dual licensed.
