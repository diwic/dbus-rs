extern crate dbus;

use dbus::{Connection, BusType, NameFlag, ConnectionItem, Message, MessageItem};
use dbus::{ObjectPath, Argument, Method, Interface};

static DBUS_ERROR_FAILED: &'static str = "org.freedesktop.DBus.Error.Failed";

fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.test", NameFlag::ReplaceExisting as u32).unwrap();

    let mut o = ObjectPath::new(&c, "/hello", true);
    o.insert_interface("com.example.test", Interface::new(
        vec!(Method::new("Hello",
            vec!(), // No input arguments
            vec!(Argument::new("reply", "s")),
            Box::new(|msg| Ok(vec!(MessageItem::Str(format!("Hello {}!", msg.sender().unwrap())))))
        )),
        vec!(), vec!() // No properties or signals
    ));
    o.set_registered(true).unwrap();

    for n in c.iter(1000) {
        match n {
            ConnectionItem::MethodCall(mut m) => {
                println!("MethodCall: {:?}", m);
                if o.handle_message(&mut m).is_none() {
                    c.send(Message::new_error(&m, DBUS_ERROR_FAILED, "Object path not found").unwrap()).unwrap();
                };
            },
            ConnectionItem::Signal(m) => {
                println!("Signal: {:?}", m);
            },
            ConnectionItem::Nothing => (),
        }
    }
}
