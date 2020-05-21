use dbus_native as dbus;
use dbus_strings as strings;

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

    use strings::StringLike;
    let path = strings::ObjectPath::new("/org/freedesktop/DBus").unwrap();
    let member = strings::MemberName::new("Hello").unwrap();
    let dest = strings::BusName::new("org.freedesktop.DBus").unwrap();
    let interface = strings::InterfaceName::new("org.freedesktop.DBus").unwrap();
    let mut m = message::Message::new_method_call(path.into(), member.into()).unwrap();
    m.set_destination(Some(dest.into())).unwrap();
    m.set_interface(Some(interface.into())).unwrap();
    println!("{:?}", m);

    let mut v_cursor = std::io::Cursor::new(vec!());
    m.write_header(std::num::NonZeroU32::new(1u32).unwrap(), &mut v_cursor).unwrap();
    let v = &v_cursor.get_ref()[..(v_cursor.position() as usize)];
    println!("{:?}", v);

    use std::io::{Write, Read};
    writer.write_all(&v).unwrap();
    writer.flush().unwrap();

    let mut mr = message::MessageReader::new();
    let v = loop {
        let buflen = {
            let buf = mr.get_buf();
            reader.read_exact(buf).unwrap();
            buf.len()
        };
        if let Some(v) = mr.buf_written_to(buflen).unwrap() { break v; }
    };
    println!("{:?}", v);
    let reply = message::Message::demarshal(&v).unwrap().unwrap();
    println!("{:?}", reply);

    let mut body = reply.read_body();
    let r = body.next().unwrap().unwrap();
    let r2 = r.parse().unwrap();
    assert!(body.next().is_none());
    assert_eq!(reply.reply_serial().unwrap().get(), 1u32);
    assert!(r2.as_dbus_str().unwrap().starts_with(":1."));
    println!("Our ID is {}", r2.as_dbus_str().unwrap());

}
