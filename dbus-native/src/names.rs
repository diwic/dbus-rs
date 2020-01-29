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

pub fn is_valid_member(s: &str) -> Result<(), ()> {
    if s.len() > 255 { return Err(()); }
    let mut x = s.bytes();
    let c = x.next().ok_or(())?;
    is_az_(c)?;
    for c in x { is_az09_(c)? };
    Ok(())
}

pub fn is_valid_error(s: &str) -> Result<(), ()> {
    return is_valid_interface(s)
}

pub fn is_valid_interface(s: &str) -> Result<(), ()> {
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

pub fn is_valid_bus(s: &str) -> Result<(), ()> {
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

#[test]
fn member() {
    assert!(is_valid_member("").is_err());
    assert!(is_valid_member("He11o").is_ok());
    assert!(is_valid_member("He11o!").is_err());
    assert!(is_valid_member("1Hello").is_err());
    assert!(is_valid_member(":1.54").is_err());
}

#[test]
fn interface() {
    assert!(is_valid_interface("").is_err());
    assert!(is_valid_interface("He11o").is_err());
    assert!(is_valid_interface("Hello.").is_err());
    assert!(is_valid_interface("Hello!.World").is_err());
    assert!(is_valid_interface("ZZZ.1Hello").is_err());
    assert!(is_valid_interface("Hello.W0rld").is_ok());
    assert!(is_valid_interface(":1.54").is_err());
}

#[test]
fn bus() {
    assert!(is_valid_bus("").is_err());
    assert!(is_valid_bus("He11o").is_err());
    assert!(is_valid_bus("Hello.").is_err());
    assert!(is_valid_bus("Hello!.World").is_err());
    assert!(is_valid_bus("ZZZ.1Hello").is_err());
    assert!(is_valid_bus("Hello.W0rld").is_ok());
    assert!(is_valid_bus(":1.54").is_ok());
    assert!(is_valid_bus("1.54").is_err());
}
