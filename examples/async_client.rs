extern crate dbus;

use dbus::{Connection, BusType, Message};
use dbus::arg::Array;

fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    c.send_with_reply(m, |result| {
        let r = result.unwrap();
        // ListNames returns one argument, which is an array of strings.
        let arr: Array<&str, _>  = r.get1().unwrap();
        for name in arr { println!("{}", name); }
    }, 2000).unwrap();

    for _ in c.iter(0) {
    }
}

