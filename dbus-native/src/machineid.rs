fn is_hex_char(b: u8) -> bool {
    match b {
        b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => true,
        _ => false,
    }
}

pub fn read_machine_id() -> Result<String, Box<dyn std::error::Error>> {
    let mut v = std::fs::read("/etc/machine-id")?;
    while v.last() == Some(&b'\n') { v.pop(); };
    if v.len() != 32 || v.iter().any(|x| !is_hex_char(*x)) { Err("Malformed machine-id file")? };
    let v = String::from_utf8(v)?;
    Ok(v)
}

#[test]
fn machineid() {
    let m = read_machine_id().unwrap();
    println!("My machine id is: {}", m);
}
