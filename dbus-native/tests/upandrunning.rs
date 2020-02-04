use dbus_native as dbus;

use dbus::{address, types, message, authentication};

#[test]
fn connect_to_session_bus() {
    let addr = address::read_session_address().unwrap();
    let stream = address::connect_blocking(&addr).unwrap();

    let mut reader = std::io::BufReader::new(&stream);
    let mut writer = &stream;
    assert!(!authentication::Authentication::blocking(&mut reader, &mut writer, false).unwrap());
    writer.flush().unwrap();

    // Send Hello message

    let mut m = message::Message::new_method_call("/org/freedesktop/DBus".into(), "Hello".into()).unwrap();
    m.set_destination(Some("org.freedesktop.DBus".into())).unwrap();
    m.set_interface(Some("org.freedesktop.DBus".into())).unwrap();
    println!("{:?}", m);

    let mut v_storage = vec![0u8; 256];
    let v = m.write_header(std::num::NonZeroU32::new(1u32).unwrap(), &mut v_storage).unwrap();
    println!("{:?}", v);

    use std::io::{Write, Read};
    writer.write_all(v).unwrap();
    writer.flush().unwrap();

    let mut v_storage = vec![0u8; 256];
    reader.read_exact(&mut v_storage[0..16]).unwrap();
    let total_len = message::total_message_size(&v_storage[0..16]).unwrap();
    reader.read_exact(&mut v_storage[16..total_len]).unwrap();
    println!("{:?}", &v_storage[0..total_len]);

    let reply = message::Message::parse(&v_storage[0..total_len]).unwrap().unwrap();
    println!("{:?}", reply);

    let (r, q): (types::Str, _) = types::Demarshal::parse(reply.body(), reply.is_big_endian()).unwrap();
    assert_eq!(q.len(), 0);
    assert!(r.starts_with(":1."));
    println!("Our ID is {}", &*r);

}
