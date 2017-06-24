extern crate dbus;

use dbus::{Connection, BusType, stdintf};

fn main() {
    // Connect to server and create a ConnPath. A ConnPath implements several interfaces,
    // in this case we'll use OrgFreedesktopDBusProperties, which allows us to call "get".
    let c = Connection::get_private(BusType::Session).unwrap();
    let p = c.with_path("org.mpris.MediaPlayer2.rhythmbox", "/org/mpris/MediaPlayer2", 5000);
    use stdintf::OrgFreedesktopDBusProperties;
    let metadata = p.get("org.mpris.MediaPlayer2.Player", "Metadata").unwrap();

    // The Metadata property is a Dict<String, Variant>, we can get the values out by iterating over it.
    // When using "as_iter()" for a dict, we'll get one key, it's value, next key, it's value, etc.
    // The ".0" is needed to traverse into the variant.   
    let mut iter = metadata.0.as_iter().unwrap();

    while let Some(key) = iter.next() {
        // Printing the key is easy, since we know it's a String.
        print!("{}: ", key.as_str().unwrap());

        // We don't know what type the value is. We'll try a few and fall back to
        // debug printing if the value is more complex than that.
        let value = iter.next().unwrap();
        if let Some(s) = value.as_str() { println!("{}", s); }
        else if let Some(i) = value.as_i64() { println!("{}", i); }
        else { println!("{:?}", value); }
    }
}
