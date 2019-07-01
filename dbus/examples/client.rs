extern crate dbus;

use dbus::ffidisp::{Connection, BusType};

fn main() -> Result<(), Box<std::error::Error>> {
    // First open up a connection to the session bus.
    let conn = Connection::get_private(BusType::Session)?;

    // Second, create a wrapper struct around the connection that makes it easy
    // to send method calls to a specific destination and path.
    let obj = conn.with_path("org.freedesktop.DBus", "/", 5000);

    // Now make the method call. The ListNames method call takes zero input parameters and 
    // one output parameter which is an array of strings.
    // Therefore the input is a zero tuple "()", and the output is a single tuple "(names,)".
    let (names,): (Vec<String>,) = obj.method_call("org.freedesktop.DBus", "ListNames", ())?;

    // Let's print all the names to stdout.
    for name in names { println!("{}", name); }

    Ok(())
}

