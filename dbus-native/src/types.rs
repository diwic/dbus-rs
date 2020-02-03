use std::borrow::Cow;

pub fn align_up(pos: usize, align: usize) -> usize {
    (pos + align - 1) & !(align - 1)
}

pub fn align_buf_mut<M: Marshal>(a: &mut [u8]) -> &mut [u8] {
    let p = a.as_ptr() as usize;
    let n = align_up(p, M::ALIGN);
    &mut a[(n-p)..]
}

pub trait Marshal {
    const ALIGN: usize;
    fn signature() -> Cow<'static, str>;
    // Expects a buffer filled with zeroes, that is sufficiently large
    fn write_buf<'b>(&self, _: &'b mut [u8]) -> &'b mut [u8] { todo!() }
}

pub struct Str<'a>(pub (crate) &'a [u8]);

impl Marshal for Str<'_> {
    const ALIGN: usize = 4;
    fn signature() -> Cow<'static, str> {
        "s".into()
    }
    fn write_buf<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let buf = align_buf_mut::<Self>(buf);
        let len = self.0.len();
        buf[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        buf[4..len+4].copy_from_slice(self.0);
        &mut buf[len+5..]
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
}

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
