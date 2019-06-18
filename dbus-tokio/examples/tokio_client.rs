/* This example communicates with the server example. It has following functionality:
   It periodically calls com.example.dbustest.Hello method on  "/hello" object on
   com.example.dbustest service.

   In the meantime, this example reacts to HelloHappened signal which is sent by the server.
*/

extern crate dbus;
extern crate dbus_tokio;
extern crate futures;
extern crate tokio;
extern crate tokio_timer;

use dbus::*;

use dbus_tokio::AConnection;
use futures::{Future, Stream};
use std::rc::Rc;
use std::time::Duration;
use tokio::reactor::Handle;
use tokio::runtime::current_thread;
use tokio_timer::{clock, Interval};

fn main() {
    // Let's start by starting up a connection to the session bus. We do not register a name
    // because we do not intend to expose any objects on the bus.
    let c = Rc::new(Connection::get_private(BusType::Session).unwrap());

    // To receive D-Bus signals we need to add match that defines which signals should be forwarded
    // to our application.
    c.add_match("type=signal,sender=com.example.dbustest,member=HelloHappened")
        .unwrap();

    // Create asynchronous connection object
    let f = AConnection::new(c.clone(), Handle::default())
        .map_err(|_| ())
        .and_then(|aconn| {
            let aconn = Rc::new(aconn);

            // Create interval - a Stream that will fire an event periodically
            let interval = Interval::new(clock::now(), Duration::from_secs(2));

            // Handle timer errors. Additionally this erases error type from Stream signature.
            let interval = interval.map_err(|e| panic!("TimerError: {}", e));

            // Create a future calling D-Bus method each time the interval generates a tick
            let clone = Rc::clone(&aconn);
            let calls: Box<dyn Future<Item = (), Error = ()>> =
                Box::new(interval.for_each(move |_| {
                    println!("Calling Hello...");
                    //TODO: try to handle error when calling on "/"
                    let m = Message::new_method_call(
                        "com.example.dbustest",
                        "/hello",
                        "com.example.dbustest",
                        "Hello",
                    )
                    .unwrap()
                    .append1(500u32);
                    let aconn = Rc::clone(&clone);
                    aconn.method_call(m).unwrap().then(|reply| {
                        let m = reply.unwrap();
                        let msg: &str = m.get1().unwrap();
                        println!("Received reply: {}", msg);
                        Ok(())
                    })
                }));

            // Create stream of all incoming D-Bus messages. On top of the messages stream create future,
            // running forever, handling all incoming messages
            let clone = Rc::clone(&aconn);
            let messages = clone.messages().unwrap();
            let signals: Box<dyn Future<Item = (), Error = ()>> =
                Box::new(messages.for_each(|m| {
                    let headers = m.headers();
                    let member = headers.3.unwrap();
                    if member == "HelloHappened" {
                        let arg1: &str = m.get1().unwrap();
                        println!("Hello from {} happened on the bus!", arg1)
                    } else {
                        println!("Unprocessed message: {:?}", m)
                    }
                    Ok(())
                }));

            // Simultaneously run signal handling and method calling
            futures::future::join_all(vec![signals, calls])
        });

    // Actually run the future
    current_thread::block_on_all(f).unwrap();
}
