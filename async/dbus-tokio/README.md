Tokio integration for D-Bus
===========================

This stuff is currently alpha, i e, just up and working. And the design might not be perfect yet, so breaking changes are to be expected.
But unless you try it out and report some feedback on what's great and what could work better, not much will happen at all, so get started!

Client
------

```rust
let conn = Rc::new(Connection::get_private(BusType::Session).unwrap());
let mut core = Core::new().unwrap();
let aconn = AConnection::new(conn.clone(), core.handle()).unwrap();

let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
let reply = core.run(aconn.method_call(m).unwrap()).unwrap();
let z: Vec<&str> = reply.get1().unwrap();
println!("got reply: {:?}", z);
```

Server
------

See the tokio_server for an example of how to add asynchronous methods to a tree and run them.
