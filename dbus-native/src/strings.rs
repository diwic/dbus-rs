// WIP

fn is_az_(b: u8) -> Result<(), ()> {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'_' => Ok(()),
        _ => Err(()),
    }
}

fn is_az09_(b: u8) -> Result<(), ()> {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' => Ok(()),
        _ => Err(()),
    }
}

fn is_az_hyphen(b: u8) -> Result<(), ()> {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-' => Ok(()),
        _ => Err(()),
    }
}

fn is_az09_hyphen(b: u8) -> Result<(), ()> {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => Ok(()),
        _ => Err(()),
    }
}

pub fn is_valid_member_name(s: &str) -> Result<(), ()> {
    if s.len() > 255 { return Err(()); }
    let mut x = s.bytes();
    let c = x.next().ok_or(())?;
    is_az_(c)?;
    for c in x { is_az09_(c)? };
    Ok(())
}

pub fn is_valid_error_name(s: &str) -> Result<(), ()> {
    return is_valid_interface_name(s)
}

pub fn is_valid_interface_name(s: &str) -> Result<(), ()> {
    if s.len() > 255 { return Err(()); }
    let mut x = s.bytes();
    let mut elements = 1;
    'outer: loop {
        let c = x.next().ok_or(())?;
        is_az_(c)?;
        while let Some(c) = x.next() {
            if c == b'.' {
                elements += 1;
                continue 'outer;
            }
            is_az09_(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

fn is_valid_unique_conn_name(mut x: std::str::Bytes) -> Result<(), ()> {
    let mut elements = 1;
    'outer: loop {
        let c = x.next().ok_or(())?;
        is_az09_hyphen(c)?;
        while let Some(c) = x.next() {
            if c == b'.' {
                elements += 1;
                continue 'outer;
            }
            is_az09_hyphen(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

pub fn is_valid_bus_name(s: &str) -> Result<(), ()> {
    if s.len() > 255 { return Err(()); }
    let mut x = s.bytes();
    let mut c_first = x.next().ok_or(())?;
    if c_first == b':' { return is_valid_unique_conn_name(x); }
    let mut elements = 1;
    'outer: loop {
        is_az_hyphen(c_first)?;
        while let Some(c) = x.next() {
            if c == b'.' {
                elements += 1;
                c_first = x.next().ok_or(())?;
                continue 'outer;
            }
            is_az09_hyphen(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

pub fn is_valid_object_path(s: &str) -> Result<(), ()> {
    let mut x = s.bytes();
    let c = x.next();
    if c != Some(b'/') { Err(())? };
    if s.len() == 1 { return Ok(()) };

    'outer: loop {
        let c = x.next().ok_or(())?;
        is_az09_(c)?;
        while let Some(c) = x.next() {
            if c == b'/' { continue 'outer; }
            is_az09_(c)?;
        }
        return Ok(());
    }
}

const BASIC_TYPES: &[u8] = b"ybnqiuxtdhsog";

fn sig_multi(s: &[u8]) -> Option<usize> {
    let mut pos = 0;
    while pos < s.len() {
        if s.get(pos) == Some(&b')') { return Some(pos) }
        pos += sig_single(&s[pos..])?;
    }
    Some(pos)
}

fn sig_single(s: &[u8]) -> Option<usize> {
    s.first().and_then(|c| {
        if BASIC_TYPES.into_iter().any(|x| x == c) { Some(1) }
        else {
            Some(1 + match c {
                b'v' => 0, // Variant
                b'a' => { // Array
                    if s.get(1) == Some(&b'{') { // Dict
                        let c = s.get(2)?;
                        if !BASIC_TYPES.into_iter().any(|x| x == c) { None? };
                        let pos = 3 + sig_single(&s[3..])?;
                        if s.get(pos)? != &b'}' { None? }
                        pos
                    } else {
                        sig_single(&s[1..])?
                    }
                },
                b'(' => {
                    let pos = 1 + sig_multi(&s[1..])?;
                    if pos == 1 || s.get(pos)? != &b')' { None? }
                    pos
                },
                _ => None?,
            })
        }
    })
}

pub fn is_valid_signature_single(s: &[u8]) -> Result<(), ()> {
    let pos = sig_single(s).ok_or(())?;
    return if pos == s.len() { Ok(()) } else { Err(()) }
}

pub fn is_valid_signature_multi(s: &[u8]) -> Result<(), ()> {
    let pos = sig_multi(s).ok_or(())?;
    return if pos == s.len() { Ok(()) } else { Err(()) }
}


#[test]
fn member() {
    assert!(is_valid_member_name("").is_err());
    assert!(is_valid_member_name("He11o").is_ok());
    assert!(is_valid_member_name("He11o!").is_err());
    assert!(is_valid_member_name("1Hello").is_err());
    assert!(is_valid_member_name(":1.54").is_err());
}

#[test]
fn interface() {
    assert!(is_valid_interface_name("").is_err());
    assert!(is_valid_interface_name("He11o").is_err());
    assert!(is_valid_interface_name("Hello.").is_err());
    assert!(is_valid_interface_name("Hello!.World").is_err());
    assert!(is_valid_interface_name("ZZZ.1Hello").is_err());
    assert!(is_valid_interface_name("Hello.W0rld").is_ok());
    assert!(is_valid_interface_name(":1.54").is_err());
}

#[test]
fn bus() {
    assert!(is_valid_bus_name("").is_err());
    assert!(is_valid_bus_name("He11o").is_err());
    assert!(is_valid_bus_name("Hello.").is_err());
    assert!(is_valid_bus_name("Hello!.World").is_err());
    assert!(is_valid_bus_name("ZZZ.1Hello").is_err());
    assert!(is_valid_bus_name("Hello.W0rld").is_ok());
    assert!(is_valid_bus_name(":1.54").is_ok());
    assert!(is_valid_bus_name("1.54").is_err());
}

#[test]
fn object_path() {
    assert!(is_valid_object_path("").is_err());
    assert!(is_valid_object_path("/").is_ok());
    assert!(is_valid_object_path("/1234").is_ok());
    assert!(is_valid_object_path("/abce/").is_err());
    assert!(is_valid_object_path("/ab//c/d").is_err());
    assert!(is_valid_object_path("/a/c/df1").is_ok());
    assert!(is_valid_object_path("/12.43/fasd").is_err());
    assert!(is_valid_object_path("/asdf/_123").is_ok());
}

#[test]
fn signature() {
    assert!(is_valid_signature_single(b"").is_err());
    assert!(is_valid_signature_single(b"i").is_ok());
    assert!(is_valid_signature_single(b"ii").is_err());
    assert!(is_valid_signature_single(b"vi").is_err());
    assert!(is_valid_signature_single(b"g").is_ok());
    assert!(is_valid_signature_single(b"{ss}").is_err());
    assert!(is_valid_signature_single(b"ad").is_ok());
    assert!(is_valid_signature_single(b"a{ss}").is_ok());
    assert!(is_valid_signature_single(b"a{vs}").is_err());
    assert!(is_valid_signature_single(b"a{ss}i").is_err());
    assert!(is_valid_signature_single(b"a{oa{sv}}").is_ok());
    assert!(is_valid_signature_single(b"v").is_ok());
    assert!(is_valid_signature_single(b"()").is_err());
    assert!(is_valid_signature_single(b"(s)").is_ok());
    assert!(is_valid_signature_single(b"(sa{sv}(i))").is_ok());
    assert!(is_valid_signature_single(b"(sa{sv}(i)").is_err());
    assert!(is_valid_signature_single(b"(dbus)").is_ok());

    assert!(is_valid_signature_multi(b"dbus").is_ok());
    assert!(is_valid_signature_multi(b"").is_ok());
    assert!(is_valid_signature_multi(b"dbus)").is_err());

}
