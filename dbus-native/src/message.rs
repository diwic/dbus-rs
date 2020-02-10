use std::borrow::Cow;
use dbus_strings as strings;
use crate::types;
use crate::types::Marshal;
use std::convert::TryInto;
use std::num::NonZeroU32;
use std::io;

const FIXED_HEADER_SIZE: usize = 16;

const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

#[cfg(target_endian = "little")]
const ENDIAN: u8 = b'l';
#[cfg(target_endian = "big")]
const ENDIAN: u8 = b'B';

#[derive(Clone, Debug)]
pub struct Message<'a> {
    msg_type: u8,
    flags: u8,
    serial: Option<NonZeroU32>,
    path: Option<Cow<'a, strings::ObjectPath>>,
    interface: Option<Cow<'a, strings::InterfaceName>>,
    member: Option<Cow<'a, strings::MemberName>>,
    error_name: Option<Cow<'a, strings::ErrorName>>,
    reply_serial: Option<u32>,
    destination: Option<Cow<'a, strings::BusName>>,
    sender: Option<Cow<'a, strings::BusName>>,
    signature: Option<Cow<'a, strings::SignatureMulti>>,
//    unix_fds: Option<u32>,
    body: Cow<'a, [u8]>,
    is_big_endian: bool,
}

impl<'a> Message<'a> {
    fn new_internal(t: u8) -> Self {
        Message {
            msg_type: t,
            flags: 0,
            serial: None,
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
            #[cfg(target_endian = "little")]
            is_big_endian: false,
            #[cfg(target_endian = "big")]
            is_big_endian: true,
        }
    }

    pub fn into_owned(self) -> Message<'static> {
        todo!()
    }

    pub fn new_method_call(path: Cow<'a, strings::ObjectPath>, member: Cow<'a, strings::MemberName>) -> Result<Self, ()> {
        let mut m = Message::new_internal(METHOD_CALL);
        m.set_path(Some(path))?;
        m.set_member(Some(member))?;
        Ok(m)
    }

    pub fn set_path(&mut self, value: Option<Cow<'a, strings::ObjectPath>>) -> Result<(), ()> {
        if value.is_none() && (self.msg_type == METHOD_CALL || self.msg_type == SIGNAL) { Err(())? }
        self.path = value;
        Ok(())
    }

    pub fn set_interface(&mut self, value: Option<Cow<'a, strings::InterfaceName>>) -> Result<(), ()> {
        if value.is_none() && self.msg_type == SIGNAL { Err(())? }
        self.interface = value;
        Ok(())
    }

    pub fn set_member(&mut self, value: Option<Cow<'a, strings::MemberName>>) -> Result<(), ()> {
        if value.is_none() && (self.msg_type == METHOD_CALL || self.msg_type == SIGNAL) { Err(())? }
        self.member = value;
        Ok(())
    }

    pub fn set_destination(&mut self, value: Option<Cow<'a, strings::BusName>>) -> Result<(), ()> {
        self.destination = value;
        Ok(())
    }

    pub fn set_error_name(&mut self, value: Option<Cow<'a, strings::ErrorName>>) -> Result<(), ()> {
        if value.is_none() && self.msg_type == ERROR { Err(())? }
        self.error_name = value;
        Ok(())
    }

    pub fn set_reply_serial(&mut self, value: Option<u32>) -> Result<(), ()> {
        if value.is_none() && (self.msg_type == ERROR || self.msg_type == METHOD_RETURN) { Err(())? }
        self.reply_serial = value;
        Ok(())
    }

    pub fn write_header<B: io::Write + io::Seek>(&self, serial: std::num::NonZeroU32, buf: &mut B) -> io::Result<()> {

        fn add_header_field<B, Z, Y: Marshal, F>(b: &mut types::MarshalState<B>, header_type: u8, field: Option<Z>, f: F) -> io::Result<()>
        where F: FnOnce(Z) -> Y, B: io::Write + io::Seek {
            if let Some(field) = field {
                let field = f(field);
                let s = types::Struct((header_type, types::Variant(field)));
                s.write_buf(b)
            } else { Ok(()) }
        }

        let mut b = types::MarshalState::new(buf);
        let body_len = self.body.len();
        if body_len >= 134217728 { Err(io::ErrorKind::InvalidData)? }

        b.write_single(&[ENDIAN, self.msg_type, self.flags, 1])?;
        b.write_fixed(4, &(body_len as u32).to_ne_bytes())?;
        b.write_fixed(4, &(serial.get()).to_ne_bytes())?;
        b.write_array(|b| {
            add_header_field(b, 1, self.path.as_ref(), |x| types::ObjectPath(&**x))?;
            add_header_field(b, 2, self.interface.as_ref(), |x| types::Str(&**x))?;
            add_header_field(b, 3, self.member.as_ref(), |x| types::Str(&**x))?;
            add_header_field(b, 4, self.error_name.as_ref(), |x| types::Str(&**x))?;
            add_header_field(b, 5, self.reply_serial.as_ref(), |x| *x)?;
            add_header_field(b, 6, self.destination.as_ref(), |x| types::Str(&**x))?;
            add_header_field(b, 7, self.sender.as_ref(), |x| types::Str(&**x))?;
            add_header_field(b, 8, self.signature.as_ref(), |x| types::Signature(&**x))?;
            Ok(())
        })?;
        b.write_single(b.align_buf(8))?;
        if body_len + b.pos >= 134217728 { Err(io::ErrorKind::InvalidData)? }
        Ok(())
    }

    pub fn body(&self) -> &[u8] { &self.body }

    pub fn is_big_endian(&self) -> bool { self.is_big_endian }

    // Should disconnect on error. If Ok(None) is returned, its a message that should be ignored.
    pub fn parse(buf: &'a [u8]) -> Result<Option<Self>, &'static str> {
        let start = message_start_parse(buf)?;
        if buf.len() < start.total_size { Err("Not enough message data")? }
        let msg_type = buf[1];
        if msg_type < 1 || msg_type > 4 { return Ok(None) };
        let mut m = Self::new_internal(msg_type);
        m.is_big_endian = buf[0] == b'B';
        m.flags = buf[2] & 0x7;
        if buf[3] != 1 { Err("Invalid protocol version")? };
        let serial = buf[8..12].try_into().unwrap();
        let serial = if m.is_big_endian { u32::from_be_bytes(serial) } else { u32::from_le_bytes(serial) };
        m.serial = Some(NonZeroU32::new(serial).ok_or("Serial cannot be zero")?);
        m.body = Cow::Borrowed(&buf[start.body_start..start.total_size]);

        let _header_fields = &buf[12..start.body_start];

        Ok(Some(m))
    }
}

struct MsgStart {
    body_start: usize,
    total_size: usize,
}

fn message_start_parse(buf: &[u8]) -> Result<MsgStart, &'static str> {
    if buf.len() < FIXED_HEADER_SIZE { Err("Message start must be 16 bytes")? };
    let body_len = buf[4..8].try_into().unwrap();
    let arr_len = buf[12..16].try_into().unwrap();
    let (body_len, arr_len) = match buf[0] {
        b'l' => (u32::from_le_bytes(body_len), u32::from_le_bytes(arr_len)),
        b'B' => (u32::from_be_bytes(body_len), u32::from_be_bytes(arr_len)),
        _ => Err("Invalid first byte of message")?
    };
    let body_len = body_len as usize;
    let body_start = types::align_up(arr_len as usize, 8) + FIXED_HEADER_SIZE;
    let total_size = body_start + body_len;
    if body_len >= 134217728 || arr_len >= 67108864 || total_size >= 134217728 {
        Err("Message too large")?
    }
    Ok(MsgStart { total_size, body_start })
}

pub fn total_message_size(buf: &[u8]) -> Result<usize, &'static str> {
    message_start_parse(buf).map(|x| x.total_size)
}

#[derive(Debug, Clone)]
pub struct MessageReader {
    storage: Vec<u8>,
    read_bytes: usize,
    total_size: Option<usize>,
}

impl MessageReader {
    pub fn new() -> Self {
        MessageReader {
            storage:  vec![0u8; 256],
            read_bytes: 0,
            total_size: None,
        }
    }
    pub fn clear(&mut self) {
        if self.storage.capacity() < 256 {
            self.storage = vec![0u8; 256];
        } else {
            self.storage.clear();
            self.storage.resize(256, 0);
        }
        self.read_bytes = 0;
        self.total_size = None;
    }
    pub fn get_buf(&mut self) -> &mut [u8] {
        if let Some(ts) = self.total_size {
            &mut self.storage[self.read_bytes..ts]
        } else {
            &mut self.storage[self.read_bytes..FIXED_HEADER_SIZE]
        }
    }

    pub fn buf_written_to(&mut self, count: usize) -> Result<Option<Vec<u8>>, &'static str> {
        self.read_bytes += count;
        if self.total_size.is_none() && self.read_bytes >= FIXED_HEADER_SIZE {
            let start = message_start_parse(&self.storage)?;
            self.total_size = Some(start.total_size);
            self.storage.resize(start.total_size, 0);
        }
        if Some(self.read_bytes) == self.total_size {
            let r = std::mem::replace(&mut self.storage, vec!());
            assert_eq!(r.len(), self.read_bytes);
            self.clear();
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn hello() {
    use dbus_strings::StringLike;
    let path = strings::ObjectPath::new("/org/freedesktop/DBus").unwrap();
    let member = strings::MemberName::new("Hello").unwrap();
    let dest = strings::BusName::new("org.freedesktop.DBus").unwrap();
    let interface = strings::InterfaceName::new("org.freedesktop.DBus").unwrap();
    let mut m = Message::new_method_call(path.into(), member.into()).unwrap();
    m.set_destination(Some(dest.into())).unwrap();
    m.set_interface(Some(interface.into())).unwrap();

    let mut v_cursor = io::Cursor::new(vec!());
    m.write_header(std::num::NonZeroU32::new(1u32).unwrap(), &mut v_cursor).unwrap();
    assert_eq!(v_cursor.get_ref().len() as u64, v_cursor.position());
    let v = v_cursor.into_inner();
    println!("{:?}", v);
    assert_eq!(v.len() % 8, 0);

    assert_eq!(&*v, &[108, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 109, 0, 0, 0,
        1, 1, 111, 0, 21, 0, 0, 0, 47, 111, 114, 103, 47, 102, 114, 101, 101, 100, 101, 115, 107, 116, 111, 112, 47, 68, 66, 117, 115, 0, 0, 0,
        2, 1, 115, 0, 20, 0, 0, 0, 111, 114, 103, 46, 102, 114, 101, 101, 100, 101, 115, 107, 116, 111, 112, 46, 68, 66, 117, 115, 0, 0, 0, 0,
        3, 1, 115, 0, 5, 0, 0, 0, 72, 101, 108, 108, 111, 0, 0, 0,
        6, 1, 115, 0, 20, 0, 0, 0, 111, 114, 103, 46, 102, 114, 101, 101, 100, 101, 115, 107, 116, 111, 112, 46, 68, 66, 117, 115, 0, 0, 0, 0
    ][..]);
}
