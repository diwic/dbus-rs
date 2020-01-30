use std::borrow::Cow;
use crate::strings;

const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

#[cfg(target_endian = "little")]
const ENDIAN: u8 = b'l';
#[cfg(target_endian = "big")]
const ENDIAN: u8 = b'B';

enum State {
    FixedHeader([u8; 16]),
    VarHeader(u8),
    _Body,
}

fn add_header_string(old_size: usize, x: &Option<Cow<str>>) -> usize {
    if let Some(p) = x.as_ref() {
        let n = (old_size + 7) & !7; // struct must start on 8-byte boundary
        // u8 + padding + u32 (string length) + string + nul byte
        n + 1 + 3 + 4 + p.len() + 1
    } else { old_size }
}

impl State {
    fn start(msg: &Message, serial: u32) -> Result<State, ()> {
        let h1 = [ENDIAN, msg.msg_type, msg.flags, 1];

        let b = msg.body.len();
        if b >= 134217728 { Err(())? }
        let h2 = (b as u32).to_ne_bytes();

        let h3 = serial.to_ne_bytes();

        let mut hs = 4; // array size
        hs = add_header_string(hs, &msg.path);
        hs = add_header_string(hs, &msg.interface);
        hs = add_header_string(hs, &msg.member);
        hs = add_header_string(hs, &msg.error_name);
        if msg.reply_serial.is_some() {
            hs = (hs + 7) & !7;
            hs += 1 + 3 + 4;
        }
        hs = add_header_string(hs, &msg.destination);
        hs = add_header_string(hs, &msg.sender);
        if let Some(r) = msg.signature.as_ref() { // Signatures require no padding
            hs = (hs + 7) & !7;
            hs += 1 + 1 + r.len() + 1;
        }
        debug_assert!(hs <= 67108864); // All fields have lengths < 256
        let h4 = hs.to_ne_bytes();

        if hs + 16 + 8 + b > 134217728 { Err(())? }

        let a = [
            h1[0], h1[1], h1[2], h1[3],
            h2[0], h2[1], h2[2], h2[3],
            h3[0], h3[1], h3[2], h3[3],
            h4[0], h4[1], h4[2], h4[3],
        ];

        Ok(State::FixedHeader(a))
    }

}

pub struct WriteState<'b, 'a> {
    state: State,
    pos: usize,
    msg: &'b Message<'a>,
}

impl WriteState<'_, '_> {
    // Return true if finished
    pub fn write<W: std::io::Write>(&mut self, w: &mut W) -> std::io::Result<bool> {
        loop {
            match self.state {
                State::FixedHeader(h) => {
                    let p = w.write(&h[self.pos..])?;
                    self.pos += p;
                    debug_assert!(self.pos <= 16);
                    if self.pos == 16 {
                        self.pos = 0;
                        self.state = State::VarHeader(1);
                    } else { return Ok(false) };
                }
                State::_Body => {
                    let body_len = self.msg.body.len();
                    debug_assert!(self.pos <= body_len);
                    if self.pos == body_len { return Ok(true) };
                    let p = w.write(&self.msg.body[self.pos..])?;
                    self.pos += p;
                    debug_assert!(self.pos <= body_len);
                    return Ok(self.pos == body_len);
                }
                State::VarHeader(_) => todo!(),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message<'a> {
    msg_type: u8,
    flags: u8,
    serial: u32,
    path: Option<Cow<'a, str>>,
    interface: Option<Cow<'a, str>>,
    member: Option<Cow<'a, str>>,
    error_name: Option<Cow<'a, str>>,
    reply_serial: Option<u32>,
    destination: Option<Cow<'a, str>>,
    sender: Option<Cow<'a, str>>,
    signature: Option<Cow<'a, str>>,
//    unix_fds: Option<u32>,
    body: Cow<'a, [u8]>,
}

impl<'a> Message<'a> {
    fn new_internal(t: u8) -> Self {
        Message {
            msg_type: t,
            flags: 0,
            serial: 0,
            path: None,
            interface: None,
            member: None,
            error_name: None,
            reply_serial: None,
            destination: None,
            sender: None,
            signature: None,
//            unix_fds: None,
            body: Cow::Borrowed(&[]),
        }
    }

    pub fn into_owned(self) -> Message<'static> {
        todo!()
    }

    pub fn new_method_call(path: Cow<'a, str>, member: Cow<'a, str>) -> Result<Self, ()> {
        let mut m = Message::new_internal(METHOD_CALL);
        m.set_path(Some(path))?;
        m.set_member(Some(member))?;
        Ok(m)
    }

    pub fn set_path(&mut self, value: Option<Cow<'a, str>>) -> Result<(), ()> {
        if let Some(v) = value.as_ref() {
            strings::is_valid_object_path(v.as_bytes())?
        } else if self.msg_type == METHOD_CALL || self.msg_type == SIGNAL {
            Err(())?
        }
        self.path = value;
        Ok(())
    }

    pub fn set_interface(&mut self, value: Option<Cow<'a, str>>) -> Result<(), ()> {
        if let Some(v) = value.as_ref() {
            strings::is_valid_interface_name(v.as_bytes())?
        } else if self.msg_type == SIGNAL {
            Err(())?
        }
        self.interface = value;
        Ok(())
    }

    pub fn set_member(&mut self, value: Option<Cow<'a, str>>) -> Result<(), ()> {
        if let Some(v) = value.as_ref() {
            strings::is_valid_member_name(v.as_bytes())?
        } else if self.msg_type == METHOD_CALL || self.msg_type == SIGNAL {
            Err(())?
        }
        self.member = value;
        Ok(())
    }

    pub fn set_destination(&mut self, value: Option<Cow<'a, str>>) -> Result<(), ()> {
        if let Some(v) = value.as_ref() {
            strings::is_valid_bus_name(v.as_bytes())?
        };
        self.destination = value;
        Ok(())
    }

    pub fn set_error_name(&mut self, value: Option<Cow<'a, str>>) -> Result<(), ()> {
        if let Some(v) = value.as_ref() {
            strings::is_valid_error_name(v.as_bytes())?
        } else if self.msg_type == ERROR {
            Err(())?
        }
        self.error_name = value;
        Ok(())
    }

    pub fn set_reply_serial(&mut self, value: Option<u32>) -> Result<(), ()> {
        if value.is_none() && (self.msg_type == ERROR || self.msg_type == METHOD_RETURN) {
            Err(())?
        }
        self.reply_serial = value;
        Ok(())
    }

    pub fn write<'b, W: std::io::Write>(&'b mut self, serial: std::num::NonZeroU32) -> Result<WriteState<'b, 'a>, ()> {
        Ok(WriteState {
            state: State::start(self, serial.get())?,
            pos: 0,
            msg: self
        })
    }
}

#[test]
fn hello() {
    let mut m = Message::new_method_call("/org/freedesktop/DBus".into(), "Hello".into()).unwrap();
    m.set_destination(Some("org.freedesktop.DBus".into())).unwrap();
    m.set_interface(Some("org.freedesktop.DBus".into())).unwrap();

}
