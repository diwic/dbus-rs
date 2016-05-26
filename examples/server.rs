/* This example creates a D-Bus server with the following functionality:
   It registers the "com.example.dbustest" name, creates a "/hello" object path,
   which has an "com.example.dbustest" interface.

   The interface has a "Hello" method (which takes no arguments and returns a string),
   and a "HelloHappened" signal (with a string argument) which is sent every time
   someone calls the "Hello" method.
*/


extern crate dbus;

use std::sync::Arc;
use dbus::{Connection, BusType, NameFlag};
use dbus::tree::Factory;

fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.dbustest", NameFlag::ReplaceExisting as u32).unwrap();

    let f = Factory::new_fn();
    let signal = Arc::new(f.signal("HelloHappened").sarg::<&str,_>("sender"));
    let tree = f.tree().add(f.object_path("/hello").introspectable().add(
        f.interface("com.example.dbustest").add_m(
            f.method("Hello", |m,_,_| {
                let sender = m.sender().unwrap();
                let s = format!("Hello {}!", sender);
                let sig = signal.msg().append1(&*sender);
                Ok(vec!(m.method_return().append1(s), sig))
            }).outarg::<&str,_>("reply") // One output argument, no input arguments
        ).add_s_arc(signal.clone())
    ));

    tree.set_registered(&c, true).unwrap();
    for _ in tree.run(&c, c.iter(1000)) {}
}
