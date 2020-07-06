/*
   This example is a WIP demo of the "Crossroads" module, successor of the "Tree" module.

   This example creates a D-Bus server with the following functionality:
   It registers the "com.example.dbustest" name, creates a "/hello" object path,
   which has an "com.example.dbustest" interface.

   The interface has a "Hello" method (which takes no arguments and returns a string),
   and a "HelloHappened" signal (with a string argument) which is sent every time
   someone calls the "Hello" method.
*/
use dbus::blocking::Connection;
use dbus_crossroads::{Crossroads, Context};
use std::error::Error;

// This is our "Hello" object that we are going to store inside the crossroads instance.
struct Hello { called_count: u32 }


fn main() -> Result<(), Box<dyn Error>> {
    // Let's start by starting up a connection to the session bus and request a name.
    let c = Connection::new_session()?;
    c.request_name("com.example.dbustest", false, true, false)?;

    // Create a new crossroads instance.
    // The instance is configured so that introspection and properties interfaces
    // are added by default on object path additions.
    let mut cr = Crossroads::new();

    // Let's build a new interface, which can be used for "Hello" objects.
    let iface_token = cr.register("com.example.dbustest", |b| {
        // This row is just for introspection: It advertises that we can send a
        // HelloHappened signal. We use the single-tuple to say that we have one single argument,
        // named "sender" of type "String".
        b.signal::<(String,), _>("HelloHappened", ("sender",));
        // Let's add a method to the interface. We have the method name, followed by
        // names of input and output arguments (used for introspection). The closure then controls
        // the types of these arguments. The last argument to the closure is a tuple of the input arguments.
        b.method("Hello", ("name",), ("reply",), |ctx: &mut Context, hello: &mut Hello, (name,): (String,)| {
            // And here's what happens when the method is called.
            println!("Incoming hello call from {}!", name);
            hello.called_count += 1;
            let s = format!("Hello {}! This API has been used {} times.", name, hello.called_count);
            // The ctx parameter can be used to conveniently send extra messages.
            let signal_msg = ctx.make_signal("HelloHappened", (name,));
            ctx.push_msg(signal_msg);
            // And the return value is a tuple of the output arguments.
            Ok((s,))
        });
    });

    // Let's add the "/hello" path, which implements the com.example.dbustest interface,
    // to the crossroads instance.
    cr.insert("/hello", &[iface_token], Hello { called_count: 0});

    // Serve clients forever.
    cr.serve(&c)?;
    unreachable!()
}
