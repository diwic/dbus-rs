dbus-futures
============

Async DBus connection for Futures 0.3.

Experimental / WIP / alpha.

Current event loops supported:

 * [thin_main_loop](https://crates.io/crates/thin_main_loop) (compile with features = "tml_io")

Help wanted with:

 * [Tokio](https://tokio.rs) (compile with features = "tokio_io")

Examples
--------

Method call:

```rust
use thin_main_loop as tml;
use dbus_futures as dbusf;

// First set up an executor, the connection task, and spawn it on the executor
let conn = dbusf::Connection::new_session::<dbusf::thin_main_loop::Io>().unwrap();
let c = conn.handle();
let mut exec = tml::future::Executor::new().unwrap();
exec.spawn(conn);

// Get the property "interfaces" on DBus, by using auto-generated code by dbus-codegen
use dbusf::stdintf::org_freedesktop::DBus;
let r = c.with_dbus_path().get_interfaces().into_future().then(|reply| {
   // This function is executed asynchronously, when the reply is returned
   let reply = reply.unwrap();
   println!("Supported interfaces: {:?}", reply);

   // And after that, we're done, so quit the program.
   tml::terminate();
   futures::future::ready(())
});

// Now spawn and run our method call on the executor.
exec.spawn(r);
exec.run();
```

Monitor signals:

```rust
use thin_main_loop as tml;
use dbus_futures as dbusf;

// First set up an executor, the connection task, and spawn it on the executor
let conn = dbusf::Connection::new_session::<dbusf::thin_main_loop::Io>().unwrap();
let c = conn.handle();
let mut exec = tml::future::Executor::new().unwrap();
exec.spawn(conn);

use dbusf::stdintf::org_freedesktop::DBusNameOwnerChanged;
let s = c.add_signal_stream::<DBusNameOwnerChanged>(None, None).into_stream().for_each(move |s| {
    let s = s.unwrap();
    if s.new_owner == "" {
        println!("{} is now available", s.name);
    } else {
        println!("{} is now owned by {}", s.name, s.new_owner);
    }
    futures::future::ready(())
});

// Now spawn and run our signal receiver on the executor.
exec.spawn(s);
exec.run();
```

