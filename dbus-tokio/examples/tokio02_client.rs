use dbus_tokio::connection;
use dbus::nonblock;
use std::time::Duration;
use dbus::message::MatchRule;
use dbus::channel::MatchingReceiver;
use futures::StreamExt;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Connect to the D-Bus session bus (this is blocking, unfortunately).
    let (resource, conn) = connection::new_session_sync()?;

    // The resource is a task that should be spawned onto a tokio compatible
    // reactor ASAP. If the resource ever finishes, you lost connection to D-Bus.
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    // To receive D-Bus signals we need to add a match that defines which signals should be forwarded
    // to our application.
    //
    // (A more convenient API might be done later)
    let mr = MatchRule::new_signal("com.example.dbustest", "HelloHappened");

    // Call the dbus server to register our interest in the signal.
    conn.add_match_no_cb(&mr.match_str()).await?;

    // This is our own method handler that will be called every time we receive a matching signal.
    conn.start_receive(mr, Box::new(|msg, _| {
        // TODO: Handle error here
        let source: &str = msg.read1().unwrap();
        println!("Hello from {} happened on the bus!", source);
        true
    }));

    // Create interval - a Stream that will fire an event periodically
    let interval = tokio::time::interval(Duration::from_secs(2));

    // Create a future calling D-Bus method each time the interval generates a tick
    let calls = interval.for_each(move |_| {
        let conn = conn.clone();
        async {
            println!("Calling Hello...");
            let proxy = nonblock::Proxy::new("com.example.dbustest", "/hello", Duration::from_secs(2), conn);
            // TODO: Handle timeouts and errors here
            let (x,): (String,) = proxy.method_call("com.example.dbustest", "Hello", ("Tokio async/await",)).await.unwrap();
            println!("{}", x);
        }
    });

    // This will never return, because the interval stream never ends.
    calls.await;

    unreachable!()
}
