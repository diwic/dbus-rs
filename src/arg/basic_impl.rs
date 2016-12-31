use ffi;
use super::*;
use super::check;
use {Signature, Path, OwnedFd};
use std::{ptr, any};
use std::ffi::CStr;
use std::os::raw::{c_void, c_char, c_int};

fn arg_append_basic(i: *mut ffi::DBusMessageIter, arg_type: ArgType, v: i64) {
    let p = &v as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type as c_int, p));
    };
}

fn arg_get_basic(i: *mut ffi::DBusMessageIter, arg_type: ArgType) -> Option<i64> {
    let mut c = 0i64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type as c_int { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut c_void);
    }
    Some(c)
}

fn arg_append_f64(i: *mut ffi::DBusMessageIter, arg_type: ArgType, v: f64) {
    let p = &v as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type as c_int, p));
    };
}

fn arg_get_f64(i: *mut ffi::DBusMessageIter, arg_type: ArgType) -> Option<f64> {
    let mut c = 0f64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type as c_int { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut c_void);
    }
    Some(c)
}

fn arg_append_str(i: *mut ffi::DBusMessageIter, arg_type: ArgType, v: &CStr) {
    let p = v.as_ptr();
    let q = &p as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type as c_int, q));
    };
}

unsafe fn arg_get_str<'a>(i: *mut ffi::DBusMessageIter, arg_type: ArgType) -> Option<&'a CStr> {
    if ffi::dbus_message_iter_get_arg_type(i) != arg_type as c_int { return None };
    let mut p = ptr::null_mut();
    ffi::dbus_message_iter_get_basic(i, &mut p as *mut _ as *mut c_void);
    Some(CStr::from_ptr(p as *const c_char))
}




// Implementation for basic types.

macro_rules! integer_impl {
    ($t: ident, $s: ident, $f: expr) => {

impl Arg for $t {
    #[inline]
    fn arg_type() -> ArgType { ArgType::$s }
    #[inline]
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl Append for $t {
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::$s, self as i64) }
}

impl<'a> Get<'a> for $t {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, ArgType::$s).map(|q| q as $t) }
}

impl RefArg for $t {
    #[inline]
    fn arg_type(&self) -> ArgType { ArgType::$s }
    #[inline]
    fn signature(&self) -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }

    /* fn get<'a>(&mut self, i: &mut Iter<'a>) -> Result<(), ()> {
        arg_get_basic(&mut i.0, ArgType::$s).map(|q| { *self = q as $t; }).ok_or(())
    } */

    fn append(&self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::$s, *self as i64) }
    #[inline]
    fn as_any(&self) -> &any::Any { self }
}

impl DictKey for $t {}
unsafe impl FixedArray for $t {}

}} // End of macro_rules

integer_impl!(u8, Byte, b"y\0");
integer_impl!(i16, Int16, b"n\0");
integer_impl!(u16, UInt16, b"q\0");
integer_impl!(i32, Int32, b"i\0");
integer_impl!(u32, UInt32, b"u\0");
integer_impl!(i64, Int64, b"x\0");
integer_impl!(u64, UInt64, b"t\0");


macro_rules! refarg_impl {
    ($t: ty) => {

impl RefArg for $t {
    #[inline]
    fn arg_type(&self) -> ArgType { <$t as Arg>::arg_type() }
    #[inline]
    fn signature(&self) -> Signature<'static> { <$t as Arg>::signature() }
    /* fn get<'a>(&mut self, i: &mut Iter<'a>) -> Result<(), ()> {
        <$t as Get>::get(i).map(|q| { *self = q; }).ok_or(())
    } */
    #[inline]
    fn append(&self, i: &mut IterAppend) { <$t as Append>::append(self.clone(), i) }

    #[inline]
    fn as_any(&self) -> &any::Any { self }
}

    }
}


impl Arg for bool {
    fn arg_type() -> ArgType { ArgType::Boolean }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"b\0") } }
}
impl Append for bool {
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::Boolean, if self {1} else {0}) }
}
impl DictKey for bool {}
impl<'a> Get<'a> for bool {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, ArgType::Boolean).map(|q| q != 0) }
}

refarg_impl!(bool);

impl Arg for f64 {
    fn arg_type() -> ArgType { ArgType::Double }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"d\0") } }
}
impl Append for f64 {
    fn append(self, i: &mut IterAppend) { arg_append_f64(&mut i.0, ArgType::Double, self) }
}
impl DictKey for f64 {}
impl<'a> Get<'a> for f64 {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_f64(&mut i.0, ArgType::Double) }
}
unsafe impl FixedArray for f64 {}

refarg_impl!(f64);

/// Represents a D-Bus string.
impl<'a> Arg for &'a str {
    fn arg_type() -> ArgType { ArgType::String }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"s\0") } }
}

impl<'a> Append for &'a str {
    fn append(self, i: &mut IterAppend) {
        use std::borrow::Cow;
        let b: &[u8] = self.as_bytes();
        let v: Cow<[u8]> = if b.len() > 0 && b[b.len()-1] == 0 { Cow::Borrowed(b) }
        else {
            let mut bb: Vec<u8> = b.into();
            bb.push(0);
            Cow::Owned(bb)
        };
        let z = unsafe { CStr::from_ptr(v.as_ptr() as *const c_char) };
        arg_append_str(&mut i.0, ArgType::String, &z)
    }
}
impl<'a> DictKey for &'a str {}
impl<'a> Get<'a> for &'a str {
    fn get(i: &mut Iter<'a>) -> Option<&'a str> { unsafe { arg_get_str(&mut i.0, ArgType::String) }
        .and_then(|s| s.to_str().ok()) }
}

impl<'a> Arg for String {
    fn arg_type() -> ArgType { ArgType::String }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"s\0") } }
}
impl<'a> Append for String {
    fn append(mut self, i: &mut IterAppend) {
        self.push_str("\0");
        let s: &str = &self;
        s.append(i)
    }
}
impl<'a> DictKey for String {}
impl<'a> Get<'a> for String {
    fn get(i: &mut Iter<'a>) -> Option<String> { <&str>::get(i).map(|s| String::from(s)) }
}

refarg_impl!(String);

/// Represents a D-Bus string.
impl<'a> Arg for &'a CStr {
    fn arg_type() -> ArgType { ArgType::String }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"s\0") } }
}

/*
/// Note: Will give D-Bus errors in case the CStr is not valid UTF-8.
impl<'a> Append for &'a CStr {
    fn append(self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, Self::arg_type(), &self)
    }
}
*/

impl<'a> DictKey for &'a CStr {}
impl<'a> Get<'a> for &'a CStr {
    fn get(i: &mut Iter<'a>) -> Option<&'a CStr> { unsafe { arg_get_str(&mut i.0, Self::arg_type()) }}
}

impl Arg for OwnedFd {
    fn arg_type() -> ArgType { ArgType::UnixFd }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"h\0") } }
}
impl Append for OwnedFd {
    fn append(self, i: &mut IterAppend) {
        use std::os::unix::io::AsRawFd;
        arg_append_basic(&mut i.0, Self::arg_type(), self.as_raw_fd() as i64)
    }
}
impl DictKey for OwnedFd {}
impl<'a> Get<'a> for OwnedFd {
    fn get(i: &mut Iter) -> Option<Self> {
        use std::os::unix::io::RawFd;
        arg_get_basic(&mut i.0, Self::arg_type()).map(|q| OwnedFd::new(q as RawFd)) 
    }
}

macro_rules! string_impl {
    ($t: ident, $s: ident, $f: expr) => {

impl<'a> Arg for $t<'a> {
    fn arg_type() -> ArgType { ArgType::$s }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl RefArg for $t<'static> {
    fn arg_type(&self) -> ArgType { ArgType::$s }
    fn signature(&self) -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }

    /* fn get<'b>(&mut self, i: &mut Iter<'b>) -> Result<(), ()> {
        unsafe { arg_get_str(&mut i.0, ArgType::$s).map(|s| {
            *self = $t::from_slice_unchecked(s.to_bytes_with_nul()).into_static()
        }).ok_or(()) }
    } */
    fn append(&self, i: &mut IterAppend) { arg_append_str(&mut i.0, ArgType::$s, self.as_cstr()) }
    fn as_any(&self) -> &any::Any { self }
}

impl<'a> DictKey for $t<'a> {}

impl<'a> Append for $t<'a> {
    fn append(self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, ArgType::$s, self.as_cstr())
    }
}
impl<'a> Get<'a> for $t<'a> {
    fn get(i: &mut Iter<'a>) -> Option<$t<'a>> { unsafe { arg_get_str(&mut i.0, ArgType::$s) }
        .map(|s| unsafe { $t::from_slice_unchecked(s.to_bytes_with_nul()) } ) }
}

    }
}

string_impl!(Path, ObjectPath, b"o\0");
string_impl!(Signature, Signature, b"g\0");

