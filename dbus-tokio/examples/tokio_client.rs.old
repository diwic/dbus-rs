/* This example communicates with the server example. It has following functionality:
   It periodically calls com.example.dbustest.Hello method on  "/hello" object on
   com.example.dbustest service.

   In the meantime, this example reacts to HelloHappened signal which is sent by the server.
*/

extern crate dbus;
extern crate dbus_tokio;
extern crate tokio;
extern crate tokio_timer;
extern crate futures;

use dbus::{Message, ffidisp::Connection};

use std::rc::Rc;
use tokio::reactor::Handle;
use tokio::runtime::current_thread::Runtime;
use tokio_timer::{clock, Interval};
use std::time::Duration;
use futures::{Stream, Future};
use dbus_tokio::AConnection;

fn main() {
    // Let's start by starting up a connection to the session bus. We do not register a name
    // because we do not intend to expose any objects on the bus.
    let c = Rc::new(Connection::new_session().unwrap());

    // To receive D-Bus signals we need to add match that defines which signals should be forwarded
    // to our application.
    c.add_match("type=signal,sender=com.example.dbustest,member=HelloHappened").unwrap();

    // Create Tokio event loop along with asynchronous connection object
    let mut rt = Runtime::new().unwrap();
    let aconn = AConnection::new(c.clone(), Handle::default(), &mut rt).unwrap();

    // Create interval - a Stream that will fire an event periodically
    let interval = Interval::new(clock::now(), Duration::from_secs(2));

    // Handle timer errors. Additionally this erases error type from Stream signature.
    let interval = interval.map_err(|e| panic!("TimerError: {}", e) );

    // Create a future calling D-Bus method each time the interval generates a tick
    let calls = interval.for_each(|_| {
        println!("Calling Hello...");
        //TODO: try to handle error when calling on "/"
        let m = Message::new_method_call("com.example.dbustest", "/hello", "com.example.dbustest", "Hello")
            .unwrap().append1("Tokio 0.1");
        aconn.method_call(m).unwrap().then(|reply| {
            let m = reply.unwrap();
            let msg: &str = m.get1().unwrap();
            println!("{}", msg);
            Ok(())
        })
    });

    // Create stream of all incoming D-Bus messages. On top of the messages stream create future,
    // running forever, handling all incoming messages
    let messages = aconn.messages().unwrap();
    let signals = messages.for_each(|m| {
        let member = m.member().unwrap();
        if &*member == "HelloHappened" {
            let arg1 : &str = m.get1().unwrap();
            println!("Hello from {} happened on the bus!", arg1)
        } else {
            println!("Unprocessed message: {:?}", m)
        }
        Ok(())
    });

    // Simultaneously run signal handling and method calling
    rt.block_on(signals.join(calls)).unwrap();
}
