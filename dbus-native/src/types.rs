use std::borrow::Cow;
use std::convert::TryInto;

use std::io::{Result as IoResult, Write, Seek, IoSlice, ErrorKind, SeekFrom};

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

pub struct MarshalState<B> {
    pub buf: B,
    pub pos: usize
}

const ZEROS: [u8; 8] = [0; 8];

impl<B: Write + Seek> MarshalState<B> {
    pub fn new(buf: B) -> Self {
        MarshalState { buf, pos: 0 }
    }
    pub fn align_buf(&self, align: usize) -> &'static [u8] {
        let x = align_up(self.pos, align);
        &ZEROS[..(x-self.pos)]
    }
    pub fn write_single(&mut self, data: &[u8]) -> IoResult<()> {
        let written = self.buf.write(data)?;
        if written != data.len() { Err(ErrorKind::WriteZero)? }
        self.pos += written;
        Ok(())
    }
    pub fn write_vectored(&mut self, data: &[IoSlice]) -> IoResult<()> {
        let total = data.iter().map(|x| x.len()).sum();
        let written = self.buf.write_vectored(data)?;
        if written != total { Err(ErrorKind::WriteZero)? }
        self.pos += written;
        Ok(())
    }
    pub fn write_str(&mut self, s: &str) -> IoResult<()> {
        self.write_vectored(&[
            IoSlice::new(self.align_buf(4)),
            IoSlice::new(&(s.len() as u32).to_ne_bytes()[..]),
            IoSlice::new(s.as_bytes()),
            IoSlice::new(&[0])
        ])
    }
    pub fn write_fixed(&mut self, align: usize, data: &[u8]) -> IoResult<()> {
        self.write_vectored(&[
            IoSlice::new(self.align_buf(align)),
            IoSlice::new(data),
        ])
    }
    pub fn write_array<F: FnOnce(&mut Self) -> IoResult<()>>(&mut self, f: F) -> IoResult<()> {
        self.write_fixed(4, &[0, 0, 0, 0])?;
        let arr_start = self.pos;
        f(self)?;
        let arr_end = self.pos;
        let arr_size = arr_end - arr_start;
        if arr_size > 67108864 { Err(ErrorKind::InvalidData)? };

        // Go back and write the array size
        self.buf.seek(SeekFrom::Current(-(arr_size as i64) - 4))?;
        let written = self.buf.write(&((arr_end - arr_start) as u32).to_ne_bytes())?;
        if written != 4 { Err(ErrorKind::WriteZero)? }
        self.buf.seek(SeekFrom::Current(arr_size as i64))?;
        Ok(())
    }
}

pub trait Marshal {
    const ALIGN: usize;
    fn signature() -> Cow<'static, str>;
    // Expects a buffer filled with zeroes, that is sufficiently large
    fn write_buf<B: Write + Seek>(&self, _: &mut MarshalState<B>) -> IoResult<()>;
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
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_str(self.0)
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


pub struct ObjectPath<'a>(pub (crate) &'a str);
impl Marshal for ObjectPath<'_> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "o".into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_str(self.0)
    }
}


pub struct Signature<'a>(pub (crate) &'a str);

impl Marshal for Signature<'_> {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, str> {
        "g".into()
    }

    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_vectored(&[
            IoSlice::new(&[self.0.len() as u8]),
            IoSlice::new(self.0.as_bytes()),
            IoSlice::new(&[0])
        ])
    }
}

impl Marshal for u32 {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "u".into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_fixed(4, &self.to_ne_bytes())
    }
}

impl Marshal for u8 {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, str> {
        "y".into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_fixed(1, &[*self])
    }
}

pub struct Struct<T>(pub T);

impl<T1: Marshal, T2: Marshal> Marshal for Struct<(T1, T2)> {
    const ALIGN: usize = 8;
    fn signature() -> Cow<'static, str> {
        format!("({}{})", T1::signature(), T2::signature()).into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_single(b.align_buf(8))?;
        (self.0).0.write_buf(b)?;
        (self.0).1.write_buf(b)
    }
}

pub struct Array<T>(T);
impl<'a, T: Marshal> Marshal for Array<&'a [T]> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        format!("a{}", T::signature()).into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_array(|b| {
            for elem in self.0 {
                elem.write_buf(b)?;
            }
            Ok(())
        })
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
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        Signature(&T::signature()).write_buf(b)?;
        self.0.write_buf(b)
    }
}
