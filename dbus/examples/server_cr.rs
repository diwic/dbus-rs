/*
   This example is a WIP demo of the "Crossroads" module, successor of the "Tree" module.

   This example creates a D-Bus server with the following functionality:
   It registers the "com.example.dbustest" name, creates a "/hello" object path,
   which has an "com.example.dbustest" interface.

   The interface has a "Hello" method (which takes no arguments and returns a string),
   and a "HelloHappened" signal (with a string argument) which is sent every time
   someone calls the "Hello" method.
*/

extern crate dbus;

use dbus::blocking::Connection;
use dbus::crossroads::{Crossroads, Path, MsgCtx};
use std::error::Error;
use std::time::Duration;

// In crossroads, every interface must be registered with a unique type.
// We'll make a simple type here.
// It's fine to have an empty struct if you have no data to associate with it.
struct DBusTest { called_count: u32 }


fn main() -> Result<(), Box<dyn Error>> {
    // Let's start by starting up a connection to the session bus and request a name.
    let mut c = Connection::new_session()?;
    c.request_name("com.example.dbustest", false, true, false)?;

    // The choice of crossroads tells us what type of method handlers we want.
    // We choose the default one, which is Send but not Sync.
    // We also add introspection and properties interfaces by default on object path additions.
    let mut cr = Crossroads::new(true);

    // Let's register a new interface in Crossroads' interface registry.
    cr.register::<DBusTest, _>("com.example.dbustest")
        // This row is just for introspection: It advertises that we can send a
        // HelloHappened signal. We use the single-tuple to say that we have one single argument,
        // named "sender" of type "String".
        .signal::<(String,), _>("HelloHappened", ("sender",))
        // Let's add a method to the interface. We have the method name, followed by
        // names of input and output arguments (used for introspection). The closure then controls
        // the types of these arguments. The last argument to the closure is a tuple of the input arguments.
        .method("Hello", ("name",), ("reply",), |ctx: &mut MsgCtx, test: &mut DBusTest, (name,): (String,)| {
            // And here's what happens when the method is called.
            println!("Incoming hello call from {}!", name);
            test.called_count += 1;
            let s = format!("Hello {}! This API has been used {} times.", name, test.called_count);
            // The ctx parameter can be used to conveniently send extra messages.
            let signal_msg = ctx.make_signal("HelloHappened", (name,));
            ctx.send_msg(signal_msg);
            // And the return value is a tuple of the output arguments.
            Ok((s,))
        });

    // Let's add the "/hello" path...
    let mut p = Path::new("/hello");
    // ...which implements the com.example.dbustest interface...
    p.insert(DBusTest { called_count: 0});
    // ...to the crossroads instance.
    cr.insert(p);

    // We add the Crossroads instance to the connection so that incoming method calls will be handled.
    cr.start(&c);

    // Serve clients forever.
    loop { c.process(Duration::from_millis(1000))?; }
}
