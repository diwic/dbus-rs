// First, some boilerplate code to make sure we don't break the stable channel.

#[cfg(not(feature = "nightly"))]
fn main() {
    println!("Enable the nightly feature to run this example, like this:");
    println!("cargo +nightly run --features=nightly --example tokio02_client");
}

#[cfg(feature = "nightly")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio02::runtime::Runtime::new().unwrap();
    rt.block_on(nightly::main())
}

#[cfg(feature = "nightly")]
mod nightly {

// Okay, now we can start coding.

use dbus_tokio::connection;
use dbus::nonblock;
use tokio02 as tokio;
use std::time::Duration;
use tokio02::prelude::*;

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
    let interval = tokio::timer::Interval::new_interval(Duration::from_secs(2));

    // Create a future calling D-Bus method each time the interval generates a tick
    let calls = interval.for_each(move |_| {
        let c = conn.clone();
        async {
            println!("Calling Hello...");
            let proxy = nonblock::Proxy::new("com.example.dbustest", "/hello", c);
            // TODO: Handle timeouts and errors here
            let (x,): (String,) = proxy.method_call("com.example.dbustest", "Hello", ("Tokio async/await",)).await.unwrap();
            println!("{}", x);
        }
    });

    calls.await;

    Ok(())
}

}


