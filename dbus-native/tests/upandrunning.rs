use dbus_native as dbus;
use dbus::{address, message, authentication};

#[test]
fn connect_to_session_bus() {
    let addr = address::read_session_address().unwrap();
    let stream = address::connect_blocking(&addr).unwrap();

    let mut reader = std::io::BufReader::new(&stream);
    let mut writer = &stream;
    assert!(!authentication::Authentication::blocking(&mut reader, &mut writer, false).unwrap());
    writer.flush().unwrap();

    // Send Hello message

    let m = dbus::message::get_hello_message();
    println!("{:?}", m);
    let v = m.marshal(std::num::NonZeroU32::new(1u32).unwrap(), false).unwrap();
    println!("{:?}", v);

    use std::io::{Write};
    writer.write_all(&v).unwrap();
    writer.flush().unwrap();

    let mut mr = message::MessageReader::new();
    let v = mr.block_until_next_message(&mut reader).unwrap();
    println!("{:?}", v);
    let reply = message::Message::demarshal(&v).unwrap().unwrap();
    println!("{:?}", reply);

    let mut body = reply.read_body().iter();
    let r = body.next().unwrap().unwrap();
    let r2 = r.parse().unwrap();
    assert!(body.next().is_none());
    assert_eq!(reply.reply_serial().unwrap().get(), 1u32);
    assert!(r2.as_dbus_str().unwrap().starts_with(":1."));
    println!("Our ID is {}", r2.as_dbus_str().unwrap());

}
