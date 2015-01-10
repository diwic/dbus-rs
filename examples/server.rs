extern crate "dbus-rs" as dbus;

use dbus::{Connection, BusType, NameFlag, ConnectionItem, Message};

static DBUS_ERROR_FAILED: &'static str = "org.freedesktop.DBus.Error.Failed";

fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.test", NameFlag::ReplaceExisting as u32).unwrap();
    c.register_object_path("/hello").unwrap();
    for n in c.iter(1000) {
        match n {
            ConnectionItem::MethodCall(m) => {
                c.send(Message::new_error(&m, DBUS_ERROR_FAILED, "Method not found").unwrap())
                    .ok().expect("Failed to send reply");
                println!("MethodCall: {:?}", m);
            },
            ConnectionItem::Signal(m) => {
                println!("Signal: {:?}", m);
            },
            ConnectionItem::Nothing => (),
        }
    }
}
