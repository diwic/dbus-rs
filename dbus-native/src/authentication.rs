
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Debug)]
pub enum Authentication {
    WaitingForOK(bool),
    WaitingForAgreeUnixFD,
    Error,
    Begin(bool),
}

impl Authentication {
    pub fn new(do_unix_fd: bool) -> (Self, String) {
        let uid = crate::sys::getuid();
        let uid = uid.to_string();
        let mut s = String::from("\0AUTH EXTERNAL ");
        for c in uid.as_bytes() {
            s.push_str(&format!("{:2x}", c));
        }
        s.push_str("\r\n");
        (Authentication::WaitingForOK(do_unix_fd), s)
    }
    pub fn handle(&mut self, data: &[u8]) -> Result<&'static str, Box<dyn std::error::Error>> {
        let old_state = *self;
        *self = Authentication::Error;
        let s = std::str::from_utf8(data)?;
        if !s.ends_with("\r\n") { Err("D-Bus authentication error (no newline)")? };
        let s = s.trim();
        match old_state {
            Authentication::Error | Authentication::Begin(_) => Err("D-Bus invalid authentication state")?,
            Authentication::WaitingForOK(b) => if s.starts_with("OK ") || s == "OK" {
                if b {
                    *self = Authentication::WaitingForAgreeUnixFD;
                    Ok("NEGOTIATE_UNIX_FD\r\n")
                } else {
                    *self = Authentication::Begin(false);
                    Ok("BEGIN\r\n")
                }
            } else {
                Err(format!("D-Bus authentication error ({})", s))?
            },
            Authentication::WaitingForAgreeUnixFD => if s == "AGREE_UNIX_FD" {
                *self = Authentication::Begin(true);
                Ok("BEGIN\r\n")
            } else if s.starts_with("ERROR ") || s == "ERROR" {
                *self = Authentication::Begin(false);
                Ok("BEGIN\r\n")
            } else {
                Err(format!("D-Bus invalid response ({})", s))?
            },
        }
    }

    pub fn blocking<R: std::io::BufRead, W: std::io::Write>(r: &mut R, w: &mut W, do_unix_fd: bool) -> Result<bool, Box<dyn std::error::Error>> {
        let (mut a, s) = Authentication::new(do_unix_fd);
        w.write_all(s.as_bytes())?;

        let mut b = vec![];
        r.read_until(b'\n', &mut b)?;
        let s = a.handle(&b)?;
        w.write_all(s.as_bytes())?;
        if a == Authentication::WaitingForAgreeUnixFD {
            let mut b = vec![];
            r.read_until(b'\n', &mut b)?;
            let s = a.handle(&b)?;
            w.write_all(s.as_bytes())?;
        }
        if let Authentication::Begin(ufd) = a { Ok(ufd) } else { unreachable!() }
    }
}


#[test]
fn session_auth() {
    let addr = crate::address::read_session_address().unwrap();
    // dbus-deamon (not the systemd variant) has abstract sockets, which rust does not
    // support. https://github.com/rust-lang/rust/issues/42048
    if !addr.starts_with("unix:path=") { return; }
    let path = std::path::Path::new(&addr["unix:path=".len()..]);
    let stream = std::os::unix::net::UnixStream::connect(&path).unwrap();

    let mut reader = std::io::BufReader::new(&stream);
    assert!(Authentication::blocking(&mut reader, &mut &stream, true).unwrap());
}
