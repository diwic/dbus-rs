use std::os::unix::net::UnixStream;

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

fn make_sockaddr_un(start: usize, s: &str) -> Result<libc::sockaddr_un, Box<dyn std::error::Error>> {
    let bytes = s.as_bytes();
    let mut r = libc::sockaddr_un {
        sun_family: libc::AF_UNIX as libc::sa_family_t,
        sun_path: [0; 108],
    };
    if start+bytes.len()+1 >= r.sun_path.len() { Err("Address too long")? }
    for (i, &x) in bytes.into_iter().enumerate() {
        r.sun_path[i+start] = x as libc::c_char;
    }
    Ok(r)
}

pub fn address_to_sockaddr_un(s: &str) -> Result<libc::sockaddr_un, Box<dyn std::error::Error>> {
    if !s.starts_with("unix:") { Err("Address is not a unix socket")? };
    for pair in s["unix:".len()..].split(',') {
        let mut kv = pair.splitn(2, "=");
        if let Some(key) = kv.next() {
            if let Some(value) = kv.next() {
                if key == "path" { return make_sockaddr_un(0, value); }
                if key == "abstract" { return make_sockaddr_un(1, value); }
            }
        }
    }
    Err(format!("unsupported address type: {}", s))?
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
