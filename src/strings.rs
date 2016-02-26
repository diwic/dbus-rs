// CString wrappers.

use ffi;
use std::{str, fmt, ops, default};
use std::ffi::{CStr, CString};
use std::borrow::Cow;
use Error;
use libc;

macro_rules! cstring_wrapper {
    ($t: ident, $s: ident) => {


impl<'m> $t<'m> {
    /// Creates a new instance of this struct.
    pub fn new<S: Into<Vec<u8>>>(s: S) -> Result<$t<'m>, String> {
        let c = try!(CString::new(s).map_err(|e| e.to_string()));
        let mut e = Error::empty();
        let b = unsafe { ffi::$s(c.as_ptr(), e.get_mut()) };
        if b != 0 { Ok($t(Cow::Owned(c))) } else { Err(e.message().unwrap().into()) }
    }

    /// Creates a new instance of this struct. If you end it with \0,
    /// it can borrow the slice without extra allocation.
    pub fn from_slice(s: &'m [u8]) -> Result<$t<'m>, String> {
        if s.len() == 0 || s[s.len()-1] != 0 { return $t::new(s) };
        let mut e = Error::empty();
        let b = unsafe { ffi::$s(s.as_ptr() as *const libc::c_char, e.get_mut()) };
        if b != 0 {
            let c = unsafe { CStr::from_ptr(s.as_ptr() as *const libc::c_char) };
            Ok($t(Cow::Borrowed(c))) 
        }
            else { Err(e.message().unwrap().into()) }
    }

    /// This function creates a new instance of this struct, without checking.
    /// It's up to you to guarantee that s ends with a \0 and is valid.
    pub unsafe fn from_slice_unchecked(s: &'m [u8]) -> $t<'m> {
        debug_assert!(s[s.len()-1] == 0);
        $t(Cow::Borrowed(CStr::from_ptr(s.as_ptr() as *const libc::c_char)))
    }

    /// View this struct as a CStr.
    pub fn as_cstr(&self) -> &CStr { &self.0 }
}

/*
/// #Panics
///
/// If given string is not valid.
/// impl<S: Into<Vec<u8>>> From<S> for $t { fn from(s: S) -> $t { $t::new(s).unwrap() } }
*/

/// #Panics
///
/// If given string is not valid.
impl<'m> From<String> for $t<'m> { fn from(s: String) -> $t<'m> { $t::new(s).unwrap() } }

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m String> for $t<'m> { fn from(s: &'m String) -> $t<'m> { $t::from_slice(s.as_bytes()).unwrap() } }

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m str> for $t<'m> { fn from(s: &'m str) -> $t<'m> { $t::from_slice(s.as_bytes()).unwrap() } }


impl<'m> ops::Deref for $t<'m> {
    type Target = str;
    fn deref(&self) -> &str { str::from_utf8(self.0.to_bytes()).unwrap() }
}

impl<'m> fmt::Display for $t<'m> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = &self;
        (&s as &fmt::Display).fmt(f)
    }
}

impl<'m> AsRef<CStr> for $t<'m> {
    fn as_ref(&self) -> &CStr { &self.0 }
}

}}

/// A wrapper around a string that is guaranteed to be
/// a valid (single) D-Bus type signature. Supersedes TypeSig.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Signature<'a>(Cow<'a, CStr>);

cstring_wrapper!(Signature, dbus_signature_validate_single);

impl Signature<'static> {
    /// Makes a D-Bus signature that corresponds to A. 
    pub fn make<A: super::arg::Arg>() -> Signature<'static> { A::signature() }
}

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus object path.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Path<'a>(Cow<'a, CStr>);

cstring_wrapper!(Path, dbus_validate_path);

// This is needed so one can make arrays of paths easily
impl<'a> default::Default for Path<'a> {
    fn default() -> Path<'a> { Path(Cow::Borrowed(unsafe { CStr::from_ptr(b"/\0".as_ptr() as *const libc::c_char)})) }
}

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus member, i e, a signal or method name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Member<'a>(Cow<'a, CStr>);

cstring_wrapper!(Member, dbus_validate_member);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus interface name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Interface<'a>(Cow<'a, CStr>);

cstring_wrapper!(Interface, dbus_validate_interface);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus bus name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BusName<'a>(Cow<'a, CStr>);

cstring_wrapper!(BusName, dbus_validate_bus_name);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus bus name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ErrorName<'a>(Cow<'a, CStr>);

cstring_wrapper!(ErrorName, dbus_validate_error_name);

#[test]
fn some_path() {
    let p1: Path = "/valid".into();
    let p2 = Path::new("##invalid##");
    assert_eq!(p1, Path(Cow::Borrowed(unsafe { CStr::from_ptr(b"/valid\0".as_ptr() as *const libc::c_char) })));
    assert_eq!(p2, Err("Object path was not valid: '##invalid##'".into()));
}

#[test]
fn make_sig() {
    assert_eq!(&*Signature::make::<(&str, u8)>(), "(sy)");
}
