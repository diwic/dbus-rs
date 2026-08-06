use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixStream;

#[cfg(all(target_family = "unix", any(target_os = "linux", target_os = "android")))]
mod linux_like;

#[cfg(all(target_family = "unix", not(any(target_os = "linux", target_os = "android"))))]
mod unix;

#[cfg(all(target_family = "unix", any(target_os = "linux", target_os = "android")))]
pub use linux_like::address_to_sockaddr_un;

#[cfg(all(target_family = "unix", not(any(target_os = "linux", target_os = "android"))))]
pub use unix::address_to_sockaddr_un;

pub fn getuid() -> u32 {
    let x = unsafe { libc::getuid() };
    x as u32
}

pub fn connect_blocking(addr: &SocketAddr) -> Result<UnixStream, Box<dyn std::error::Error>> {
    let us = UnixStream::connect_addr(addr)?;
    us.set_nonblocking(false)?;
    Ok(us)
}
