use std::borrow::Cow;
use std::convert::TryInto;

pub fn align_up(pos: usize, align: usize) -> usize {
    (pos + align - 1) & !(align - 1)
}

pub fn align_buf_mut<M: Marshal>(a: &mut [u8]) -> &mut [u8] {
    let p = a.as_ptr() as usize;
    let n = align_up(p, M::ALIGN);
    &mut a[(n-p)..]
}

pub fn align_buf<M: Marshal>(a: &[u8]) -> Result<&[u8], &'static str> {
    let p = a.as_ptr() as usize;
    let n = align_up(p, M::ALIGN);
    let z = n-p;
    if z >= a.len() { Err("Not enough message data (while aligning)")? }
    Ok(&a[(n-p)..])
}


pub trait Marshal {
    const ALIGN: usize;
    fn signature() -> Cow<'static, str>;
    // Expects a buffer filled with zeroes, that is sufficiently large
    fn write_buf<'b>(&self, _: &'b mut [u8]) -> &'b mut [u8];
}

pub trait Demarshal<'a>: Marshal + Sized {
    fn parse(buf: &'a [u8], is_be: bool) -> Result<(Self, &'a[u8]), &'static str>;
}

pub struct Str<'a>(pub (crate) &'a str);

impl std::ops::Deref for Str<'_> {
    type Target = str;
    fn deref(&self) -> &str { self.0 }
}

impl Marshal for Str<'_> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "s".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        let len = self.0.len();
        buf[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        buf[4..len+4].copy_from_slice(self.0.as_bytes());
        &mut buf[len+5..]
    }
}

impl<'a> Demarshal<'a> for Str<'a> {
    fn parse(buf: &'a [u8], is_be: bool) -> Result<(Self, &'a[u8]), &'static str> {
        let buf = align_buf::<Self>(buf)?;
        let src_len = buf.len();
        if src_len < 5 { Err("Not enough message data (str length)")? }
        let dest_len = buf[0..4].try_into().unwrap();
        let dest_len = (if is_be { u32::from_be_bytes(dest_len) } else { u32::from_le_bytes(dest_len) }) as usize;
        if src_len < 4 + dest_len + 1 { Err("Not enough message data (str)")? }
        if buf[4+dest_len] != b'\0' { Err("Missing nul terminator")? }
        let dest = &buf[4..4+dest_len];
        if dest.iter().any(|&b| b == b'\0') { Err("Interior nul")? }
        let dest = std::str::from_utf8(dest).map_err(|_| "String is not UTF-8")?;
        Ok((Str(dest), &buf[5+dest_len..]))
    }
}


pub struct ObjectPath<'a>(pub (crate) &'a [u8]);
impl Marshal for ObjectPath<'_> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "o".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        let len = self.0.len();
        buf[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        buf[4..len+4].copy_from_slice(self.0);
        &mut buf[len+5..]
    }
}


pub struct Signature<'a>(pub (crate) &'a [u8]);

impl Marshal for Signature<'_> {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, str> {
        "g".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let len = self.0.len();
        buf[0] = len as u8;
        buf[1..len+1].copy_from_slice(self.0);
        &mut buf[len+2..]
    }
}

impl Marshal for u32 {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "u".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        buf[..4].copy_from_slice(&self.to_ne_bytes());
        &mut buf[4..]
    }
}

impl Marshal for u8 {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, str> {
        "y".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        buf[0] = *self;
        &mut buf[1..]
    }
}

pub struct Struct<T>(pub T);

impl<T1: Marshal, T2: Marshal> Marshal for Struct<(T1, T2)> {
    const ALIGN: usize = 8;
    fn signature() -> Cow<'static, str> {
        format!("({}{})", T1::signature(), T2::signature()).into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        let buf = (self.0).0.write_buf(buf);
        (self.0).1.write_buf(buf)
    }
}

pub struct Array<T>(T);
impl<'a, T: Marshal> Marshal for Array<&'a [T]> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        format!("a{}", T::signature()).into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        let (arr_size, mut buf) = buf.split_at_mut(4);
        let start = buf.as_ptr() as usize;
        for elem in self.0 {
            buf = elem.write_buf(buf);
        }
        let end = buf.as_ptr() as usize;
        if end - start > 67108864 { panic!("Array too large to write") }
        arr_size.copy_from_slice(&((end - start) as u32).to_ne_bytes());
        buf
    }
}
/*
pub struct ArrayParser<'a, T> {
    buf: &'a [u8],
    is_be: bool,
}

impl Iterator for ArrayParser<T> {
    type Item = ();
    fn next(&mut self) -> Option<Result<Self, &'static str>> {
        if buf.len() { return None; }

    }
}

pub fn parse_array(buf: &[u8])

impl<'a, T: Demarshal> Demarshal for Array<&'a [T]> {
}
*/
pub struct Variant<T>(pub T);
impl<T: Marshal> Marshal for Variant<T> {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, str> {
        "v".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = Signature(T::signature().as_bytes()).write_buf(buf);
        self.0.write_buf(buf)
    }
}
