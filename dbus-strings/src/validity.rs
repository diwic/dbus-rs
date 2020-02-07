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

pub fn is_valid_string(s: &str) -> Result<(), ()> {
    let s = s.as_bytes();
    // 134217728 (128 MiB) is the maximum length of a message, so it follows that
    // a string can't be longer.
    if s.len() >= 134217728 { Err(()) }
    else if s.iter().any(|&b| b == 0) { Err(()) } else { Ok(()) }
}

pub fn is_valid_member_name(s: &[u8]) -> Result<(), ()> {
    if s.len() > 255 { Err(())? }
    let mut x = s.into_iter();
    let c = *x.next().ok_or(())?;
    is_az_(c)?;
    for c in x { is_az09_(*c)? };
    Ok(())
}

pub fn is_valid_error_name(s: &[u8]) -> Result<(), ()> {
    return is_valid_interface_name(s)
}

pub fn is_valid_interface_name(s: &[u8]) -> Result<(), ()> {
    if s.len() > 255 { Err(())? }
    let mut x = s.into_iter();
    let mut elements = 1;
    'outer: loop {
        let c = *x.next().ok_or(())?;
        is_az_(c)?;
        while let Some(&c) = x.next() {
            if c == b'.' {
                elements += 1;
                continue 'outer;
            }
            is_az09_(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

fn is_valid_unique_conn_name(mut x: std::slice::Iter<u8>) -> Result<(), ()> {
    let mut elements = 1;
    'outer: loop {
        let c = *x.next().ok_or(())?;
        is_az09_hyphen(c)?;
        while let Some(&c) = x.next() {
            if c == b'.' {
                elements += 1;
                continue 'outer;
            }
            is_az09_hyphen(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

pub fn is_valid_bus_name(s: &[u8]) -> Result<(), ()> {
    if s.len() > 255 { return Err(()); }
    let mut x = s.into_iter();
    let mut c_first = *x.next().ok_or(())?;
    if c_first == b':' { return is_valid_unique_conn_name(x); }
    let mut elements = 1;
    'outer: loop {
        is_az_hyphen(c_first)?;
        while let Some(&c) = x.next() {
            if c == b'.' {
                elements += 1;
                c_first = *x.next().ok_or(())?;
                continue 'outer;
            }
            is_az09_hyphen(c)?;
        }
        return if elements > 1 { Ok(()) } else { Err(()) }
    }
}

pub fn is_valid_object_path(s: &[u8]) -> Result<(), ()> {
    let mut x = s.into_iter();
    let c = x.next();
    if c != Some(&b'/') { Err(())? };
    if s.len() == 1 { return Ok(()) };

    'outer: loop {
        let c = *x.next().ok_or(())?;
        is_az09_(c)?;
        while let Some(&c) = x.next() {
            if c == b'/' { continue 'outer; }
            is_az09_(c)?;
        }
        return Ok(());
    }
}

const BASIC_TYPES: &[u8] = b"ybnqiuxtdhsog";

fn sig_multi(s: &[u8], arrs: u8, structs: u8) -> Option<usize> {
    let mut pos = 0;
    while pos < s.len() {
        if s.get(pos) == Some(&b')') { return Some(pos) }
        pos += sig_single(&s[pos..], arrs, structs)?;
    }
    Some(pos)
}

fn sig_single(s: &[u8], arrs: u8, structs: u8) -> Option<usize> {
    s.first().and_then(|c| {
        if BASIC_TYPES.into_iter().any(|x| x == c) { Some(1) }
        else {
            Some(1 + match c {
                b'v' => 0, // Variant
                b'a' => { // Array
                    if arrs >= 32 { None? };
                    if s.get(1) == Some(&b'{') { // Dict
                        let c = s.get(2)?;
                        if !BASIC_TYPES.into_iter().any(|x| x == c) { None? };
                        let pos = 3 + sig_single(&s[3..], arrs+1, structs)?;
                        if s.get(pos)? != &b'}' { None? }
                        pos
                    } else {
                        sig_single(&s[1..], arrs+1, structs)?
                    }
                },
                b'(' => {
                    if structs >= 32 { None? };
                    let pos = 1 + sig_multi(&s[1..], arrs, structs+1)?;
                    if pos == 1 || s.get(pos)? != &b')' { None? }
                    pos
                },
                _ => None?,
            })
        }
    })
}

pub fn is_valid_signature_single(s: &[u8]) -> Result<(), ()> {
    if s.len() > 255 { Err(())? }
    let pos = sig_single(s, 0, 0).ok_or(())?;
    return if pos == s.len() { Ok(()) } else { Err(()) }
}

pub fn is_valid_signature_multi(s: &[u8]) -> Result<(), ()> {
    if s.len() > 255 { Err(())? }
    let pos = sig_multi(s, 0, 0).ok_or(())?;
    return if pos == s.len() { Ok(()) } else { Err(()) }
}

#[test]
fn string() {
    assert!(is_valid_string("").is_ok());
    assert!(is_valid_string("Hell\0").is_err());
    assert!(is_valid_string("\u{ffff}").is_ok());
}

#[test]
fn member() {
    assert!(is_valid_member_name(b"").is_err());
    assert!(is_valid_member_name(b"He11o").is_ok());
    assert!(is_valid_member_name(b"He11o!").is_err());
    assert!(is_valid_member_name(b"1Hello").is_err());
    assert!(is_valid_member_name(b":1.54").is_err());
}

#[test]
fn interface() {
    assert!(is_valid_interface_name(b"").is_err());
    assert!(is_valid_interface_name(b"He11o").is_err());
    assert!(is_valid_interface_name(b"Hello.").is_err());
    assert!(is_valid_interface_name(b"Hello!.World").is_err());
    assert!(is_valid_interface_name(b"ZZZ.1Hello").is_err());
    assert!(is_valid_interface_name(b"Hello.W0rld").is_ok());
    assert!(is_valid_interface_name(b":1.54").is_err());
}

#[test]
fn bus() {
    assert!(is_valid_bus_name(b"").is_err());
    assert!(is_valid_bus_name(b"He11o").is_err());
    assert!(is_valid_bus_name(b"Hello.").is_err());
    assert!(is_valid_bus_name(b"Hello!.World").is_err());
    assert!(is_valid_bus_name(b"ZZZ.1Hello").is_err());
    assert!(is_valid_bus_name(b"Hello.W0rld").is_ok());
    assert!(is_valid_bus_name(b":1.54").is_ok());
    assert!(is_valid_bus_name(b"1.54").is_err());
}

#[test]
fn object_path() {
    assert!(is_valid_object_path(b"").is_err());
    assert!(is_valid_object_path(b"/").is_ok());
    assert!(is_valid_object_path(b"/1234").is_ok());
    assert!(is_valid_object_path(b"/abce/").is_err());
    assert!(is_valid_object_path(b"/ab//c/d").is_err());
    assert!(is_valid_object_path(b"/a/c/df1").is_ok());
    assert!(is_valid_object_path(b"/12.43/fasd").is_err());
    assert!(is_valid_object_path(b"/asdf/_123").is_ok());
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
