use std::os::unix::net::UnixStream;

pub fn getuid() -> u32 {
    rustix::process::getuid().as_raw()
}

pub fn connect_blocking(addr: &libc::sockaddr_un) -> Result<UnixStream, Box<dyn std::error::Error>> {
    // We have to do this manually because rust std does not support abstract sockets.
    // https://github.com/rust-lang/rust/issues/42048

    assert_eq!(addr.sun_family, libc::AF_UNIX as libc::sa_family_t);

    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 { Err("Unable to create unix socket")? }

    let mut sock_len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
    let mut x = addr.sun_path.len()-1;
    while addr.sun_path[x] == 0 {
        x -= 1;
        sock_len -= 1;
    }

    let addr_ptr = addr as *const _ as *const libc::sockaddr;
    let r = unsafe { libc::connect(fd, addr_ptr, sock_len) };
    if r != 0 {
        let errno = unsafe { (*libc::__errno_location()) as i32 };
        Err(format!("Unable to connect to unix socket, errno = {}", errno))?
    }

    use std::os::unix::io::FromRawFd;
    let u = unsafe { UnixStream::from_raw_fd(fd) };
    Ok(u)
}
