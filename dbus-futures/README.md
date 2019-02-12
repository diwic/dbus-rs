dbus-futures
============

Async DBus connection for Futures 0.3.

Experimental / WIP / alpha.

Current event loops supported:

 * thin_main_loop (compile with features = "tml_io")

Example
-------

```rust
use thin_main_loop as tml;

// First set up an executor, the connection task, and spawn it on the executor
let ctr = ConnTxRx::new_session().unwrap();
let c = ctr.handle();
let mut exec = tml::future::Executor::new().unwrap();
exec.spawn(ctr);

// Get the property "interfaces" on DBus, by using auto-generated code by dbus-codegen
use crate::stdintf::org_freedesktop::DBus;
let remote_path = c.with_path("org.freedesktop.DBus", "/org/freedesktop/DBus");
let r = remote_path.get_interfaces().into_future().then(|reply| {
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
