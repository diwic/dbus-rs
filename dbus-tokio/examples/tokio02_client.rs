use dbus_tokio::connection;
use dbus::nonblock;
use std::time::Duration;
use dbus::message::MatchRule;
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

    // Create interval - a Stream that will fire an event periodically
    let interval = tokio::time::interval(Duration::from_secs(2));

    // Create a future calling D-Bus method each time the interval generates a tick
    let conn2 = conn.clone();
    let calls = interval.for_each(move |_| {
        let conn = conn2.clone();
        async {
            println!("Calling Hello...");
            let proxy = nonblock::Proxy::new("com.example.dbustest", "/hello", Duration::from_secs(2), conn);
            // TODO: Handle timeouts and errors here
            let (x,): (String,) = proxy.method_call("com.example.dbustest", "Hello", ("Tokio async/await",)).await.unwrap();
            println!("{}", x);
        }
    });

    // To receive D-Bus signals we need to add a match that defines which signals should be forwarded
    // to our application.
    let mr = MatchRule::new_signal("com.example.dbustest", "HelloHappened");
    let incoming_signal = conn.add_match(mr).await?.cb(|_, (source,): (String,)| {
        println!("Hello from {} happened on the bus!", source);
        true
    });

    // This will never return, because the interval stream never ends.
    calls.await;

    /*
    // ..or use the match as a stream if you prefer
    let (incoming_signal, stream) = conn.add_match(mr).await?.stream();
    let stream = stream.for_each(|(_, (source,)): (_, (String,))| {
        println!("Hello from {} happened on the bus!", source);
        async {}
    });
    futures::join!(stream, calls)
    */

    // Needed here to ensure the "incoming_signal" object is not dropped too early
    conn.remove_match(incoming_signal.token()).await?;

    unreachable!()
}
