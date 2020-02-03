use std::borrow::Cow;
use crate::strings;
use crate::types;
use crate::types::Marshal;


const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

#[cfg(target_endian = "little")]
const ENDIAN: u8 = b'l';
#[cfg(target_endian = "big")]
const ENDIAN: u8 = b'B';

fn add_header_string<'a>(buf: &'a mut [u8], header_type: u8, x: &Option<Cow<str>>) -> &'a mut [u8] {
    if let Some(p) = x.as_ref() {
        let s = types::Struct((header_type, types::Variant(types::Str(p.as_bytes()))));
        s.write_buf(buf)
    } else { buf }
}

#[derive(Clone, Debug)]
pub struct Message<'a> {
    msg_type: u8,
    flags: u8,
//    serial: Option<NonZeroU32>,
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
/*
fn size_estimate(x: &Option<Cow<str>>) -> usize {
    if let Some(s) = x.as_ref() { 7 + 1 + 3 + s.len() + 1 } else { 0 }
}
*/
impl<'a> Message<'a> {
/*
    fn marshal_header(&self, serial: u32, f: impl FnOnce(&mut [u8])) -> Result<(), ()> {
        const BIG_HEADER: usize = 256;

        // Estimate header size
        // First 7 (pre-align) + 16 + 16 (reply serial) + 7 (post-align)
        let mut size = 7 + 16 + 16 + 7;
        size += size_estimate(&self.path);
        size += size_estimate(&self.interface);
        size += size_estimate(&self.member);
        size += size_estimate(&self.error_name);
        size += size_estimate(&self.destination);
        size += size_estimate(&self.sender);
        size += size_estimate(&self.signature);

        let mut dyn_buf;
        let mut stack_buf;
        let b: &mut [u8] = if size >= BIG_HEADER {
            dyn_buf = vec![0; size];
            &mut *dyn_buf
        } else {
            stack_buf = [0; BIG_HEADER];
            &mut stack_buf[..]
        };

        let mut b = types::align_buf_mut::<types::Struct::<(u8, u8)>>(b);

        let p = ENDIAN.write_buf(&mut b);
        let p = self.msg_type.write_buf(p);
        let p = self.flags.write_buf(p);
        let p = 1u8.write_buf(p);

        let body_len = self.body.len();
        if body_len >= 134217728 { Err(())? }
        let p = (body_len as u32).write_buf(p);
        let p = serial.write_buf(p);
        let (arr_size_buf, mut p) = p.split_at_mut(4);
        let arr_begin = p.as_ptr() as usize;
        p = add_header_string(p, 1, &self.path);
        p = add_header_string(p, 2, &self.interface);
        p = add_header_string(p, 3, &self.member);
        p = add_header_string(p, 4, &self.error_name);
        if let Some(r) = self.reply_serial.as_ref() {
            let s = types::Struct((5u8, *r));
            p = s.write_buf(p)
        }
        p = add_header_string(p, 6, &self.destination);
        p = add_header_string(p, 7, &self.sender);
        if let Some(r) = self.signature.as_ref() {
            let s = types::Struct((8u8, types::Signature(r.as_bytes())));
            p = s.write_buf(p)
        }

        let arr_end = p.as_ptr() as usize;
        let arr_size = arr_end - arr_begin;
        (arr_size as u32).write_buf(arr_size_buf);
        let header_size = types::align_up(arr_end, 8) - (b.as_ptr() as usize);
        if body_len + header_size >= 134217728 { Err(())? }

        f(&mut b[..header_size]);
        Ok(())
    }
*/
    fn new_internal(t: u8) -> Self {
        Message {
            msg_type: t,
            flags: 0,
//            serial: 0,
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

    pub fn write_header<'b>(&self, serial: std::num::NonZeroU32, buf: &'b mut [u8]) -> Result<&'b mut [u8], ()> {
        let mut b = types::align_buf_mut::<types::Struct::<(u8, u8)>>(buf);

        let p = ENDIAN.write_buf(&mut b);
        let p = self.msg_type.write_buf(p);
        let p = self.flags.write_buf(p);
        let p = 1u8.write_buf(p);

        let body_len = self.body.len();
        if body_len >= 134217728 { Err(())? }
        let p = (body_len as u32).write_buf(p);
        let p = serial.get().write_buf(p);
        let (arr_size_buf, mut p) = p.split_at_mut(4);
        let arr_begin = p.as_ptr() as usize;
        if let Some(r) = self.path.as_ref() {
            let s = types::Struct((1u8, types::Variant(types::ObjectPath(r.as_bytes()))));
            p = s.write_buf(p)
        }
        p = add_header_string(p, 2, &self.interface);
        p = add_header_string(p, 3, &self.member);
        p = add_header_string(p, 4, &self.error_name);
        if let Some(r) = self.reply_serial.as_ref() {
            let s = types::Struct((5u8, types::Variant(*r)));
            p = s.write_buf(p)
        }
        p = add_header_string(p, 6, &self.destination);
        p = add_header_string(p, 7, &self.sender);
        if let Some(r) = self.signature.as_ref() {
            let s = types::Struct((8u8, types::Variant(types::Signature(r.as_bytes()))));
            p = s.write_buf(p)
        }

        let arr_end = p.as_ptr() as usize;
        let arr_size = arr_end - arr_begin;
        (arr_size as u32).write_buf(arr_size_buf);
        let header_size = types::align_up(arr_end, 8) - (b.as_ptr() as usize);
        if body_len + header_size >= 134217728 { Err(())? }

        Ok(&mut b[..header_size])
    }

/*
    pub fn write_header<W: std::io::Write>(&self, w: &mut W, serial: std::num::NonZeroU32, mut offset: usize)
    -> std::io::Result<Option<usize>> {
        let mut res = Err(std::io::ErrorKind::InvalidData.into());
        let _ = self.marshal_header(serial.get(), |buf| {
            let buf = &buf[offset..];
            res = w.write(buf).map(|written| {
                offset += written;
                if offset >= buf.len() { None } else { Some(offset) }
            })
        });
        res
    }
    */
}

#[test]
fn hello() {
    let mut m = Message::new_method_call("/org/freedesktop/DBus".into(), "Hello".into()).unwrap();
    m.set_destination(Some("org.freedesktop.DBus".into())).unwrap();
    m.set_interface(Some("org.freedesktop.DBus".into())).unwrap();

    let mut v_storage = vec![0u8; 256];
    let v = m.write_header(std::num::NonZeroU32::new(1u32).unwrap(), &mut v_storage).unwrap();
    println!("{:?}", v);
    assert_eq!(v.len() % 8, 0);
}
