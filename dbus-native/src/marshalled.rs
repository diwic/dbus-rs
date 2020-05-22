#![allow(dead_code)]

use std::str::from_utf8;
use dbus_strings::{SignatureMulti, SignatureSingle, StringLike, DBusStr};
use std::convert::TryInto;
use crate::types::DemarshalError;

#[derive(Clone, Debug, Copy)]
pub struct Multi<'a> {
    sig: &'a SignatureMulti,
    data: &'a [u8],
    is_big_endian: bool,
}

#[derive(Clone, Debug, Copy)]
pub struct MultiIter<'a> {
    inner: Multi<'a>,
    start_pos: usize,
}

#[derive(Clone, Debug, Copy)]
pub struct Single<'a> {
    sig: &'a SignatureSingle,
    data: &'a [u8],
    start_pos: usize,
    is_big_endian: bool,
}

impl<'a> Multi<'a> {
    pub fn new(sig: &'a SignatureMulti, data: &'a [u8], is_big_endian: bool) -> Self {
        Multi { sig, data, is_big_endian }
    }

    fn get_real_length(&self) -> Result<usize, DemarshalError> {
        let x = self.data.len();
        let mut iter = self.iter();
        while let Some(r) = iter.next() { r?; }
        Ok(x - iter.inner.data.len())
    }

    pub fn iter(&self) -> MultiIter<'a> {
        MultiIter { inner: *self, start_pos: 0 }
    }
}

impl<'a> Iterator for MultiIter<'a> {
    type Item = Result<Single<'a>, DemarshalError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.sig.single().map(|(first, rest)| {
            let mut s = Single {
                sig: first,
                data: self.inner.data,
                start_pos: self.start_pos,
                is_big_endian: self.inner.is_big_endian,
            };
            let mut len = s.get_real_length()?;
            if rest.len() > 0 {
                len = align_up(len + self.start_pos, align_of(rest.as_bytes()[0])) - self.start_pos;
            }
            if len > self.inner.data.len() { Err(DemarshalError::NotEnoughData)? }
            let (fdata, rdata) = self.inner.data.split_at(len);
            s.data = fdata;
            self.inner.data = rdata;
            self.inner.sig = rest;
            self.start_pos += len;
            Ok(s)
        })
    }
}

pub fn align_up(pos: usize, align: usize) -> usize {
    (pos + align - 1) & !(align - 1)
}

pub fn align_of(c: u8) -> usize {
    match c {
        b'y' | b'g' | b'v' => 1,
        b'n' | b'q' => 2,
        b'i' | b'u' | b'b' | b's' | b'o' | b'a' | b'h' => 4,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,

        _ => panic!("Unexpected byte in type signature: {}", c)
    }
}

impl Single<'_> {
    fn read_f64(&self) -> Result<f64, DemarshalError> {
        let x: [u8; 8] = self.data[0..8].try_into().map_err(|_| DemarshalError::NotEnoughData)?;
        Ok(if self.is_big_endian { f64::from_be_bytes(x) } else { f64::from_le_bytes(x) })
    }

    fn read8(&self) -> Result<u64, DemarshalError> {
        let x: [u8; 8] = self.data[0..8].try_into().map_err(|_| DemarshalError::NotEnoughData)?;
        Ok(if self.is_big_endian { u64::from_be_bytes(x) } else { u64::from_le_bytes(x) })
    }

    fn read4(&self) -> Result<u32, DemarshalError> {
        let x: [u8; 4] = self.data[0..4].try_into().map_err(|_| DemarshalError::NotEnoughData)?;
        Ok(if self.is_big_endian { u32::from_be_bytes(x) } else { u32::from_le_bytes(x) })
    }

    fn read2(&self) -> Result<u16, DemarshalError> {
        let x: [u8; 2] = self.data[0..1].try_into().map_err(|_| DemarshalError::NotEnoughData)?;
        Ok(if self.is_big_endian { u16::from_be_bytes(x) } else { u16::from_le_bytes(x) })
    }

    fn read1(&self) -> Result<u8, DemarshalError> {
        self.data.get(0).ok_or(DemarshalError::NotEnoughData).map(|x| *x)
    }

    fn read_sig(&self) -> Result<&SignatureMulti, DemarshalError> {
        let siglen = self.read1()? as usize;
        let sig = self.data.get(1..siglen).ok_or(DemarshalError::NotEnoughData)?;
        from_utf8(sig).ok().and_then(|s| SignatureMulti::new(s).ok()).ok_or(DemarshalError::InvalidString)
    }

    fn read_str<T: StringLike+ ?Sized>(&self) -> Result<&T, DemarshalError> {
        let len = self.read4()? as usize;
        let s = self.data.get(4..len+4).ok_or(DemarshalError::NotEnoughData)?;
        from_utf8(s).ok().and_then(|s| T::new(s).ok()).ok_or(DemarshalError::InvalidString)
    }

    fn inner_variant(&self) -> Result<Single, DemarshalError> {
        let siglen = self.read1()? as usize;
        let sig = self.data.get(1..siglen).ok_or(DemarshalError::NotEnoughData)?;
        let sig = from_utf8(sig).ok().and_then(|s| SignatureSingle::new(s).ok()).ok_or(DemarshalError::InvalidString)?;
        let data_start = align_up(self.start_pos + siglen+2, align_of(sig.as_bytes()[0])) - self.start_pos;
        Ok(Single {
            sig,
            start_pos: self.start_pos + data_start,
            data: self.data.get(data_start..).ok_or(DemarshalError::NotEnoughData)?,
            is_big_endian: self.is_big_endian,
        })
    }

    fn inner_struct(&self) -> Multi {
        let s: &str = self.sig;
        let (s0, s) = s.split_at(1);
        let (s, s9) = s.split_at(s.len() - 1);

        debug_assert_eq!(s0, "(");
        debug_assert_eq!(s9, ")");
        Multi {
            sig: SignatureMulti::new_unchecked(s),
            data: self.data,
            is_big_endian: self.is_big_endian,
        }
    }

    fn get_real_length(&self) -> Result<usize, DemarshalError> {
        Ok(match self.sig.as_bytes()[0] {
            b'y' => 1,
            b'n' | b'q' => 2,
            b'i' | b'u' | b'b' | b'h' => 4,
            b'x' | b't' | b'd' => 8,
            b's' | b'o' => self.read4()? as usize + 4 + 1,
            b'g' => self.read1()? as usize + 1 + 1,
            b'a' => {
                let x = self.read4()? as usize;
                if x > 67108864 { Err(DemarshalError::NumberTooBig)? };
                x + 4
            },
            b'v' => self.inner_variant()?.get_real_length()?,
            b'(' => self.inner_struct().get_real_length()?,
            c => panic!("Unexpected byte in type signature: {}", c)
        })
    }

    fn parse_array(&self) -> Result<Parsed, DemarshalError> {
        let x = self.read4()? as usize;
        if x > 67108864 { Err(DemarshalError::NumberTooBig)? };
        Ok(if self.sig.as_bytes()[1] == b'{' {
            let inner_sig = SignatureMulti::new_unchecked(&self.sig[1..self.sig.len()-1]);
            let (key_sig, value_sig) = inner_sig.single().unwrap();
            let (value_sig, _) = value_sig.single().unwrap();
            let data_start = align_up(self.start_pos + 4, align_of(b'{')) - self.start_pos;
            if data_start + x > self.data.len() { Err(DemarshalError::NotEnoughData)? };
            Parsed::Dict(Dict {
                key_sig, value_sig,
                is_big_endian: self.is_big_endian,
                data: &self.data[data_start..data_start + x],
            })
        } else {
            let inner_sig = SignatureSingle::new_unchecked(&self.sig[1..]);
            let data_start = align_up(self.start_pos + 4, align_of(inner_sig.as_bytes()[0])) - self.start_pos;
            if data_start + x > self.data.len() { Err(DemarshalError::NotEnoughData)? };
            Parsed::Array(Array {
                data: &self.data[data_start..data_start + x],
                start_pos: data_start + self.start_pos,
                is_big_endian: self.is_big_endian,
                inner_sig,
            })
        })
    }

    pub fn parse(&self) -> Result<Parsed, DemarshalError> {
        Ok(match self.sig.as_bytes()[0] {
            b'y' => Parsed::Byte(self.read1()?),
            b'n' => Parsed::Int16(self.read2()? as i16),
            b'q' => Parsed::UInt16(self.read2()?),
            b'i' => Parsed::Int32(self.read4()? as i32),
            b'u' => Parsed::UInt32(self.read4()?),
            b'b' => Parsed::Boolean(match self.read4()? {
                0 => false,
                1 => true,
                _ => Err(DemarshalError::InvalidBoolean)?
            }),
            b'h' => Parsed::UnixFd(self.read4()? as usize),
            b'x' => Parsed::Int64(self.read8()? as i64),
            b't' => Parsed::UInt64(self.read8()?),
            b'd' => Parsed::Double(self.read_f64()?),
            b'g' => Parsed::Signature(self.read_sig()?),
            b's' => Parsed::String(self.read_str()?),
            b'o' => Parsed::ObjectPath(self.read_str()?),
            b'v' => Parsed::Variant(self.inner_variant()?),
            b'(' => Parsed::Struct(self.inner_struct()),
            b'a' => self.parse_array()?,
            c => panic!("Unexpected byte in type signature: {}", c)
        })
    }
}

/// Contains multiple values of the same type.
#[derive(Debug, Clone, Copy)]
pub struct Array<'a> {
    inner_sig: &'a SignatureSingle,
    data: &'a [u8],
    start_pos: usize,
    is_big_endian: bool,
}

impl<'a> Iterator for Array<'a> {
    type Item = Result<Single<'a>, DemarshalError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len() == 0 { return None; }
        let mut s = Single {
            is_big_endian: self.is_big_endian,
            data: self.data,
            start_pos: self.start_pos,
            sig: self.inner_sig,
        };
        let mut len = match s.get_real_length() {
            Ok(len) if len <= self.data.len() => len,
            _ => return Some(Err(DemarshalError::NotEnoughData)),
        };
        s.data = &s.data[0..len];
        if len < s.data.len() {
            len = align_up(len + self.start_pos, align_of(self.inner_sig.as_bytes()[0])) - self.start_pos;
            self.start_pos += len;
            self.data = &self.data[len..];
        } else {
            self.data = &[];
        }
        Some(Ok(s))
    }
}

/// Contains multiple keys and values, where every key is of the same type
/// and every value is of the same type.
#[derive(Debug, Clone, Copy)]
pub struct Dict<'a> {
    key_sig: &'a SignatureSingle,
    value_sig: &'a SignatureSingle,
    data: &'a [u8],
    is_big_endian: bool,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Parsed<'a> {
    /// A D-Bus array requires all elements to be of the same type.
    Array(Array<'a>),
    /// A D-Bus dictionary requires all keys and all values to be of the same type.
    Dict(Dict<'a>),
    /// A D-Bus struct is a list of values of different types.
    Struct(Multi<'a>),
    /// A D-Bus variant is a wrapper around another type, which
    /// can be of any valid D-Bus type.
    Variant(Single<'a>),
    /// A D-Bus object path.
    ObjectPath(&'a dbus_strings::ObjectPath),
    /// A D-Bus signature.
    Signature(&'a SignatureMulti),
    /// A D-Bus String.
    String(&'a DBusStr),
    /// A D-Bus boolean type.
    Boolean(bool),
    /// A D-Bus unsigned 8 bit type.
    Byte(u8),
    /// A D-Bus signed 16 bit type.
    Int16(i16),
    /// A D-Bus signed 32 bit type.
    Int32(i32),
    /// A D-Bus signed 64 bit type.
    Int64(i64),
    /// A D-Bus unsigned 16 bit type.
    UInt16(u16),
    /// A D-Bus unsigned 32 bit type.
    UInt32(u32),
    /// A D-Bus unsigned 64 bit type.
    UInt64(u64),
    /// A D-Bus IEEE-754 double-precision floating point type.
    Double(f64),
    /// D-Bus allows for sending file descriptors, which can be used to
    /// set up SHM, unix pipes, or other communication channels.
    ///
    /// The usize is an index that can need to be used with the message to get the actual file descriptor out.
    UnixFd(usize),
}

impl Parsed<'_> {
    pub fn as_dbus_str(&self) -> Result<&DBusStr, DemarshalError> {
        match self {
            Parsed::String(x) => Ok(x),
            Parsed::ObjectPath(x) => Ok(x.as_dbus_str()),
            Parsed::Signature(x) => Ok(x.as_dbus_str()),
            _ => Err(DemarshalError::WrongType),
        }
    }
}
