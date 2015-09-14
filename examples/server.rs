extern crate dbus;

use dbus::{Connection, BusType, NameFlag, ConnectionItem, Message, MessageItem};
use dbus::mdisp::Factory;

fn main() {
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.example.dbustest", NameFlag::ReplaceExisting as u32).unwrap();

    let f = Factory::new_fn();
    let tree = f.tree().add(f.object_path("/hello").introspectable().add(
        f.interface("com.example.dbustest").add_m(
            f.method("Hello", |m,_,_| {
                let s = format!("Hello {}!", m.sender().unwrap());
                Ok(vec!(m.method_return().append(s)))
            }).out_arg(("reply", "s")) // One output argument, no input arguments
        )
    ));

    tree.set_registered(&c, true).unwrap();
    for _ in tree.run(&c, c.iter(1000)) {}
}
