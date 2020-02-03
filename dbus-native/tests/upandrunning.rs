use dbus_native as dbus;

#[test]
fn connect_to_session_bus() {
    let addr = dbus::address::read_session_address().unwrap();
    // dbus-deamon (not the systemd variant) has abstract sockets, which rust does not
    // support. https://github.com/rust-lang/rust/issues/42048
    if !addr.starts_with("unix:path=") { return; }
    let path = std::path::Path::new(&addr["unix:path=".len()..]);
    let stream = std::os::unix::net::UnixStream::connect(&path).unwrap();

    let mut reader = std::io::BufReader::new(&stream);
    let mut writer = &stream;
    assert!(!dbus::authentication::Authentication::blocking(&mut reader, &mut writer, false).unwrap());
    writer.flush().unwrap();

    // Send Hello message

    let mut m = dbus::message::Message::new_method_call("/org/freedesktop/DBus".into(), "Hello".into()).unwrap();
    m.set_destination(Some("org.freedesktop.DBus".into())).unwrap();
    m.set_interface(Some("org.freedesktop.DBus".into())).unwrap();

    let mut v_storage = vec![0u8; 256];
    let v = m.write_header(std::num::NonZeroU32::new(1u32).unwrap(), &mut v_storage).unwrap();
    println!("{:?}", v);

    use std::io::{Write, Read};
    writer.write_all(v).unwrap();
    writer.flush().unwrap();

    let mut v_storage = vec![0u8; 256];
    reader.read_exact(&mut v_storage[0..16]).unwrap();
    println!("{:?}", &v_storage[0..16]);
}
