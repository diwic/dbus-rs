use std::os::unix::net::SocketAddr;

#[cfg(target_os = "android")]
use std::os::android::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;

pub fn address_to_sockaddr_un(s: &str) -> Result<SocketAddr, Box<dyn std::error::Error>> {
    if !s.starts_with("unix:") {
        Err("Address is not a unix socket")?
    };
    for pair in s["unix:".len()..].split(',') {
        let mut kv = pair.splitn(2, "=");
        if let Some(key) = kv.next() {
            if let Some(value) = kv.next() {
                if key == "path" {
                    return Ok(SocketAddr::from_pathname(value)?);
                }
                if key == "abstract" {
                    return Ok(SocketAddr::from_abstract_name(value)?);
                }
            }
        }
    }
    Err(format!("unsupported address type: {}", s))?
}
