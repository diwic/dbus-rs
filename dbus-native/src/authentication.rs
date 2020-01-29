
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Debug)]
pub enum Authentication {
    WaitingForOK,
    WaitingForAgreeUnixFD,
    Error,
    Begin(bool),
}

impl Authentication {
    pub fn new() -> (Self, String) {
        let uid = unsafe { libc::getuid() };
        let uid = uid.to_string();
        let mut s = String::from("\0AUTH EXTERNAL ");
        for c in uid.as_bytes() {
            s.push_str(&format!("{:2x}", c));
        }
        s.push_str("\r\n");
        (Authentication::WaitingForOK, s)
    }
    pub fn handle(&mut self, data: &[u8]) -> Result<&'static str, Box<dyn std::error::Error>> {
        let old_state = *self;
        *self = Authentication::Error;
        let s = std::str::from_utf8(data)?;
        if !s.ends_with("\r\n") { Err("D-Bus authentication error (no newline)")? };
        let s = s.trim();
        match old_state {
            Authentication::Error | Authentication::Begin(_) => Err("D-Bus invalid authentication state")?,
            Authentication::WaitingForOK => if s.starts_with("OK ") || s == "OK" {
                *self = Authentication::WaitingForAgreeUnixFD;
                Ok("NEGOTIATE_UNIX_FD\r\n")
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
}


#[test]
fn session_auth() {
    let addr = crate::address::read_session_address().unwrap();
    assert!(addr.starts_with("unix:path="));
    let path = std::path::Path::new(&addr["unix:path=".len()..]);

    use std::io::prelude::*;
    let mut stream = std::os::unix::net::UnixStream::connect(&path).unwrap();
    let (mut a, s) = Authentication::new();
    stream.write_all(s.as_bytes()).unwrap();
    let mut reader = std::io::BufReader::new(&stream);

    let mut b = vec![];
    reader.read_until(b'\n', &mut b).unwrap();
    let s = a.handle(&b).unwrap();
    (&stream).write_all(s.as_bytes()).unwrap();
    assert_eq!(a, Authentication::WaitingForAgreeUnixFD);

    let mut b = vec![];
    reader.read_until(b'\n', &mut b).unwrap();
    a.handle(&b).unwrap();
    assert_eq!(a, Authentication::Begin(true));
    (&stream).write_all(s.as_bytes()).unwrap();

}
