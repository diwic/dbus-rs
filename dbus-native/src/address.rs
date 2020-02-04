use std::os::unix::net::UnixStream;

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

pub fn address_to_sockaddr(s: &str) -> Result<libc::sockaddr_un, Box<dyn std::error::Error>> {
    let mut r = libc::sockaddr_un {
        sun_family: libc::AF_UNIX as libc::sa_family_t,
        sun_path: [0; 108],
    };
    let (start, bytes) = if s.starts_with("unix:path=") {
        (0, &s.as_bytes()["unix:path=".len()..])
    } else if s.starts_with("unix:abstract=") {
        (1, &s.as_bytes()["unix:abstract=".len()..])
    } else { Err(format!("unsupported address type: {}", s))? };

    if start+bytes.len()+1 >= r.sun_path.len() { Err("Address too long")? }
    for (i, &x) in bytes.into_iter().enumerate() {
        r.sun_path[i+start] = x as libc::c_char;
    }
    Ok(r)
}

pub fn connect_blocking(addr: &str) -> Result<UnixStream, Box<dyn std::error::Error>> {
    let sockaddr = address_to_sockaddr(addr)?;
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
