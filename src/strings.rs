// CString wrappers.

use ffi;
use std::{str, fmt, ptr, ops, default};
use std::ffi::{CStr, CString};

macro_rules! cstring_wrapper {
    ($t: ident, $s: ident) => {


impl $t {
    pub fn new<S: Into<Vec<u8>>>(s: S) -> Result<$t, ()> {
        let c = try!(CString::new(s).map_err(|_| ()));
        let b = unsafe { ffi::$s(c.as_ptr(), ptr::null_mut()) };
        if b != 0 { Ok($t(c)) } else { Err(()) }
    }
}

/// #Panics
///
/// If given string is not valid.
impl<S: Into<Vec<u8>>> From<S> for $t { fn from(s: S) -> $t { $t::new(s).unwrap() } }

impl ops::Deref for $t {
    type Target = str;
    fn deref(&self) -> &str { str::from_utf8(self.0.to_bytes()).unwrap() }
}

impl fmt::Display for $t {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = &self;
        (&s as &fmt::Display).fmt(f)
    }
}

impl AsRef<CStr> for $t {
    fn as_ref(&self) -> &CStr { &self.0 }
}

}}

/// A wrapper around a string that is guaranteed to be
/// a valid (single) D-Bus type signature. Supersedes TypeSig.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Signature(CString);

cstring_wrapper!(Signature, dbus_signature_validate_single);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus object path.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Path(CString);

cstring_wrapper!(Path, dbus_validate_path);

// This is needed so one can make arrays of paths easily
impl default::Default for Path {
    fn default() -> Path { Path(CString::new("/").unwrap()) }
}

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus member, i e, a signal or method name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Member(CString);

cstring_wrapper!(Member, dbus_validate_member);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus interface name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Interface(CString);

cstring_wrapper!(Interface, dbus_validate_interface);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus bus name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BusName(CString);

cstring_wrapper!(BusName, dbus_validate_bus_name);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus bus name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ErrorName(CString);

cstring_wrapper!(ErrorName, dbus_validate_error_name);

