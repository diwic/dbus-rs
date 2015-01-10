extern crate "dbus-rs" as dbus;

use dbus::{Connection, BusType, Message};


fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let mut r = c.send_with_reply_and_block(m, 2000).unwrap();
    let reply = r.get_items();
    println!("{:?}", reply);
}

