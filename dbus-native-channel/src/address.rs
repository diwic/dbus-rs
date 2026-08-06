use std::os::unix::net::UnixStream;

pub use crate::sys::address_to_sockaddr_un;

fn env_key(key: &str) -> Option<String> {
    for (akey, value) in std::env::vars_os() {
        if akey == key {
            if let Ok(v) = value.into_string() { return Some(v) }
        }
    }
    None
}

pub fn read_session_address() -> Result<String, Box<dyn std::error::Error>> {
    Ok(env_key("DBUS_SESSION_BUS_ADDRESS").ok_or_else(|| "Environment variable not found")?)
    // TODO: according to the D-Bus spec, there are more ways to find the address, such
    // as asking the X window system.
}

pub fn read_system_address() -> Result<String, Box<dyn std::error::Error>> {
    Ok(env_key("DBUS_SYSTEM_BUS_ADDRESS").unwrap_or_else(||
        "unix:path=/var/run/dbus/system_bus_socket".into()
    ))
}

pub fn read_starter_address() -> Result<String, Box<dyn std::error::Error>> {
    Ok(env_key("DBUS_SESSION_BUS_ADDRESS").ok_or_else(|| "Environment variable not found")?)
}

pub fn connect_blocking(addr: &str) -> Result<UnixStream, Box<dyn std::error::Error>> {
    let sockaddr = address_to_sockaddr_un(addr)?;
    crate::sys::connect_blocking(&sockaddr)
}

#[test]
fn bus_exists() {
    let addr = read_session_address().unwrap();
    println!("Bus address is: {:?}", addr);
    if addr.starts_with("unix:path=") {
        let path = std::path::Path::new(&addr["unix:path=".len()..]);
        assert!(path.exists());
    }

    let addr = read_system_address().unwrap();
    if addr.starts_with("unix:path=") {
        let path = std::path::Path::new(&addr["unix:path=".len()..]);
        assert!(path.exists());
    }
}
