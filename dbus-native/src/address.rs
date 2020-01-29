pub fn read_session_address() -> Result<String, Box<dyn std::error::Error>> {
    for (key, value) in std::env::vars_os() {
        if key == "DBUS_SESSION_BUS_ADDRESS" {
            if let Ok(v) = value.into_string() { return Ok(v) }
        }
    }
    // TODO: according to the D-Bus spec, there are more ways to find the address, such
    // as asking the X window system.
    Err("Environment variable not found")?
}

pub fn read_system_address() -> Result<String, Box<dyn std::error::Error>> {
    for (key, value) in std::env::vars_os() {
        if key == "DBUS_SYSTEM_BUS_ADDRESS" {
            if let Ok(v) = value.into_string() { return Ok(v) }
        }
    }
    Ok("unix:path=/var/run/dbus/system_bus_socket".into())
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
