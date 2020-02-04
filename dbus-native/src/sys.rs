use std::os::unix::net::UnixStream;

pub fn getuid() -> u32 {
    let x = unsafe { libc::getuid() };
    x as u32
}

pub fn connect_blocking(addr: &libc::sockaddr_un) -> Result<UnixStream, Box<dyn std::error::Error>> {
    // We have to do this manually because rust std does not support abstract sockets.
    // https://github.com/rust-lang/rust/issues/42048

    assert_eq!(addr.sun_path[addr.sun_path.len()-1], 0);
    assert_eq!(addr.sun_family, libc::AF_UNIX as libc::sa_family_t);

    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 { Err("Unable to create unix socket")? }

    let sock_len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
    let addr_ptr = addr as *const _ as *const libc::sockaddr;
    let r = unsafe { libc::connect(fd, addr_ptr, sock_len) };
    if r != 0 { Err("Unable to connect to unix socket")? }

    use std::os::unix::io::FromRawFd;
    let u = unsafe { UnixStream::from_raw_fd(fd) };
    Ok(u)
}
