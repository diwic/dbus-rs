use std::borrow::Cow;
use std::convert::TryInto;
use dbus_strings as strings;
use std::fmt;
use strings::{SignatureSingle, StringLike};

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

#[derive(Debug, Clone, Copy)]
pub struct MarshalState<B> {
    pub buf: B,
    pub pos: usize
}

#[derive(Debug, Clone, Copy)]
pub struct DemarshalState<'a> {
    pub signature: &'a str,
    pub buf: &'a [u8],
    pub pos: usize,
    pub is_big_endian: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum DemarshalError {
    NotEnoughData,
    InvalidString,
    InvalidProtocol,
    WrongType,
    NumberTooBig,
}

impl fmt::Display for DemarshalError {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
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
    pub fn write_array<F: FnOnce(&mut Self) -> IoResult<()>>(&mut self, el_align: usize, f: F) -> IoResult<()> {
        let len_pos = self.pos;
        self.write_vectored(&[
            IoSlice::new(self.align_buf(4)),
            IoSlice::new(&[0, 0, 0, 0]),
        ])?;
        self.write_single(self.align_buf(el_align))?;

        let arr_start = self.pos;
        f(self)?;
        let arr_end = self.pos;
        let arr_size = arr_end - arr_start;
        if arr_size > 67108864 { Err(ErrorKind::InvalidData)? };

        // Go back and write the array size
        let seek = (arr_end as i64) - (len_pos as i64);
        self.buf.seek(SeekFrom::Current(-seek))?;
        let written = self.buf.write(&(arr_size as u32).to_ne_bytes())?;
        if written != 4 { Err(ErrorKind::WriteZero)? }
        self.buf.seek(SeekFrom::Current(seek - 4))?;
        Ok(())
    }
}

impl<'a> DemarshalState<'a> {
    pub fn new(buf: &'a [u8], pos: usize, signature: &'a str, is_big_endian: bool) -> Self {
        DemarshalState { buf, pos, signature, is_big_endian }
    }
    pub fn align_buf(&mut self, align: usize) -> Result<(), DemarshalError> {
        self.pos = align_up(self.pos, align);
        if self.pos >= self.buf.len() { Err(DemarshalError::NotEnoughData) } else { Ok(()) }
    }
    pub fn read_single(&mut self, data_len: usize, align: usize) -> Result<&[u8], DemarshalError> {
        let p = align_up(self.pos, align);
        let p2 = p + data_len;
        if p2 > self.buf.len() { Err(DemarshalError::NotEnoughData)? };
        self.pos = p2;
        Ok(&self.buf[p..p2])
    }
    pub fn read_str(&mut self, _sig: u8) -> Result<&'a str, DemarshalError> {
        // if self.signature.as_bytes().get(0) != Some(&sig) { Err(DemarshalError::WrongType)? };
        let x = self.read_single(4, 4)?;
        let x: [u8; 4] = x.try_into().unwrap();
        let z = (if self.is_big_endian { u32::from_be_bytes(x) } else { u32::from_le_bytes(x) }) as usize;
        let new_pos = self.pos + z + 1;
        if new_pos > self.buf.len() { Err(DemarshalError::NotEnoughData)? };
        let r = &self.buf[self.pos..self.pos+z];
        self.pos = new_pos;
        let r = std::str::from_utf8(r).map_err(|_| DemarshalError::InvalidString)?;
        Ok(r)
    }
    pub fn read_array(&mut self, el_align: usize) -> Result<DemarshalState<'a>, DemarshalError> {
        if self.signature.as_bytes().get(0) != Some(&b'a') { Err(DemarshalError::WrongType)? };
        let x = self.read_single(4, 4)?;
        let x: [u8; 4] = x.try_into().unwrap();
        let arr_size = (if self.is_big_endian { u32::from_be_bytes(x) } else { u32::from_le_bytes(x) }) as usize;
        if arr_size > 67108864 { Err(DemarshalError::NumberTooBig)? };
        let arr_start = align_up(self.pos, el_align);
        let new_pos = self.pos + arr_size;
        if new_pos > self.buf.len() { Err(DemarshalError::NotEnoughData)? };
        self.pos = new_pos;

        // FIXME: This signature should be cropped better
        Ok(DemarshalState::new(&self.buf[..new_pos], arr_start, &self.signature[1..], self.is_big_endian))
    }

    pub fn read_variant(&mut self) -> Result<DemarshalState<'a>, DemarshalError> {
        // if self.signature.as_bytes().get(0) != Some(&b'v') { Err(DemarshalError::WrongType)? };
        let z = u8::read_buf(self)? as usize;
        let new_pos = self.pos + z + 1;
        if new_pos > self.buf.len() { Err(DemarshalError::NotEnoughData)? };
        let r = &self.buf[self.pos..self.pos+z];
        let r = std::str::from_utf8(r).map_err(|_| DemarshalError::InvalidString)?;
        self.pos = new_pos;
        strings::SignatureSingle::new(r).map_err(|_| DemarshalError::InvalidString)?;
        let r = DemarshalState::new(&self.buf, new_pos, r, self.is_big_endian);
        Ok(r)
    }
    pub fn finished(&self) -> bool { self.buf.len() <= self.pos }
}


pub trait Marshal {
    const ALIGN: usize;
    fn signature() -> Cow<'static, SignatureSingle>;
    // Expects a buffer filled with zeroes, that is sufficiently large
    fn write_buf<B: Write + Seek>(&self, _: &mut MarshalState<B>) -> IoResult<()>;
}

pub trait Demarshal<'a>: Marshal + Sized {
    fn read_buf(_: &mut DemarshalState<'a>) -> Result<Self, DemarshalError>;
}


pub struct Str<'a>(pub (crate) &'a str);

impl std::ops::Deref for Str<'_> {
    type Target = str;
    fn deref(&self) -> &str { self.0 }
}

impl Marshal for Str<'_> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("s").into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_str(self.0)
    }
}


impl<'a> Demarshal<'a> for Str<'a> {
    fn read_buf(b: &mut DemarshalState<'a>) -> Result<Self, DemarshalError> {
        let r = b.read_str(b's')?;
        if r.as_bytes().iter().any(|&b| b == b'\0') { Err(DemarshalError::InvalidString)? }
        Ok(Str(r))
    }
}

pub type ObjectPath = strings::ObjectPath;

impl Marshal for &ObjectPath {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("o").into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_str(&*self)
    }
}

impl<'a> Demarshal<'a> for &'a ObjectPath {
    fn read_buf(b: &mut DemarshalState<'a>) -> Result<Self, DemarshalError> {
        let r = b.read_str(b's')?;
        Ok(ObjectPath::new(r).map_err(|_| DemarshalError::InvalidString)?)
    }
}

pub type Signature = strings::SignatureMulti;

impl Marshal for &Signature {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("g").into()
    }

    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_vectored(&[
            IoSlice::new(&[self.len() as u8]),
            IoSlice::new(self.as_bytes()),
            IoSlice::new(&[0])
        ])
    }
}

impl<'a> Demarshal<'a> for &'a Signature {
    fn read_buf(b: &mut DemarshalState<'a>) -> Result<Self, DemarshalError> {
        let z = u8::read_buf(b)? as usize;
        let new_pos = b.pos + z + 1;
        if new_pos > b.buf.len() { Err(DemarshalError::NotEnoughData)? };
        let r = &b.buf[b.pos..b.pos+z];
        let r = std::str::from_utf8(r).map_err(|_| DemarshalError::InvalidString)?;
        b.pos = new_pos;
        let r = Signature::new(r).map_err(|_| DemarshalError::InvalidString)?;
        Ok(r)
    }
}

impl Marshal for u32 {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("u").into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_fixed(4, &self.to_ne_bytes())
    }
}

impl Demarshal<'_> for u32 {
    fn read_buf(b: &mut DemarshalState<'_>) -> Result<Self, DemarshalError> {
        let x = b.read_single(4, 4)?;
        let x: [u8; 4] = x.try_into().unwrap();
        let z = (if b.is_big_endian { u32::from_be_bytes(x) } else { u32::from_le_bytes(x) }) as u32;
        Ok(z)
    }
}

impl Marshal for u8 {
    const ALIGN: usize = 1;
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("y").into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_fixed(1, &[*self])
    }
}

impl Demarshal<'_> for u8 {
    fn read_buf(b: &mut DemarshalState<'_>) -> Result<Self, DemarshalError> {
        // if b.signature.as_bytes().get(0) != Some(&b'y') { Err(DemarshalError::WrongType)? };
        if b.finished() { Err(DemarshalError::NotEnoughData)? };
        let r = b.buf[b.pos];
        b.pos += 1;
        // b.signature = &b.signature[1..];
        Ok(r)
    }
}

pub struct Struct<T>(pub T);

impl<T1: Marshal, T2: Marshal> Marshal for Struct<(T1, T2)> {
    const ALIGN: usize = 8;
    fn signature() -> Cow<'static, SignatureSingle> {
        let x = format!("({}{})", T1::signature(), T2::signature());
        SignatureSingle::new_unchecked_owned(x).into()
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
    fn signature() -> Cow<'static, SignatureSingle> {
        let x = format!("a{}", T::signature());
        SignatureSingle::new_unchecked_owned(x).into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        b.write_array(T::ALIGN, |b| {
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
    fn signature() -> Cow<'static, SignatureSingle> {
        SignatureSingle::new_unchecked("v").into()
    }
    fn write_buf<B: Write + Seek>(&self, b: &mut MarshalState<B>) -> IoResult<()> {
        let sig = T::signature();
        let s: &Signature = (&*sig).into();
        s.write_buf(b)?;
        self.0.write_buf(b)
    }
}
