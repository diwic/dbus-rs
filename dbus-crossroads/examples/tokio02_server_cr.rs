use dbus_tokio::connection;
use futures::future;
use tokio::time::delay_for;
use dbus::crossroads::{Crossroads, Path, AsyncMsgCtx};
use std::time::Duration;

// In crossroads, every interface must be registered with a unique type.
// We'll make a simple type here.
// It's fine to have an empty struct if you have no data to associate with it.
struct DBusTest { called_count: u32 }

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Connect to the D-Bus session bus (this is blocking, unfortunately).
    let (resource, c) = connection::new_session_sync()?;

    // The resource is a task that should be spawned onto a tokio compatible
    // reactor ASAP. If the resource ever finishes, you lost connection to D-Bus.
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    // Let's request a name on the bus, so that clients can find us.
    c.request_name("com.example.dbustest", false, true, false).await?;

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
        .method("Hello", ("name",), ("reply",), |mut ctx: AsyncMsgCtx, test: &mut DBusTest, (name,): (String,)| {
            // And here's what happens when the method is called.
            println!("Incoming hello call from {}!", name);
            test.called_count += 1;
            let s = format!("Hello {}! This API has been used {} times.", name, test.called_count);
            async move {
                // Let's wait half a second just to show off how async we are.
               delay_for(Duration::from_millis(500)).await;
                // The ctx parameter can be used to conveniently send extra messages.
                let signal_msg = ctx.make_signal("HelloHappened", (name,));
                ctx.send_msg(signal_msg);
                // And the return value is a tuple of the output arguments.
                Ok((s,))
            }
        });

    // Let's add the "/hello" path...
    let mut p = Path::new("/hello");
    // ...which implements the com.example.dbustest interface...
    p.insert(DBusTest { called_count: 0});
    // ...to the crossroads instance.
    cr.insert(p);

    // We add the Crossroads instance to the connection so that incoming method calls will be handled.
    cr.start(&*c);

    // Run forever.
    future::pending::<()>().await;
    unreachable!()
}
