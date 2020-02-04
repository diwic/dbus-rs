//! This module contains strings with a specific format, such as a valid
//! Interface name, a valid Error name, etc.
//!
//! (The internal representation of these strings are `Cow<CStr>`, which
//! makes it possible to use them in libdbus without conversion costs.)

use std::{str, fmt, ops, default, hash};
use std::ffi::{CStr, CString};
use std::borrow::{Borrow, Cow};
use std::os::raw::c_char;

#[cfg(not(feature = "no-string-validation"))]
use crate::Error;

#[cfg(feature="native")]
pub (crate) type DStr = str;

#[cfg(not(feature="native"))]
pub (crate) type DStr = CStr;

macro_rules! dstring_wrapper {
    ($(#[$comments:meta])* $t: ident, $s: ident, $n: ident) => {

$(#[$comments])*
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct $t<'a>(Cow<'a, DStr>);

#[cfg(not(feature="native"))]
impl<'m> $t<'m> {
    #[cfg(feature = "no-string-validation")]
    fn check_valid(_: *const c_char) -> Result<(), String> { Ok(()) }

    #[cfg(not(feature = "no-string-validation"))]
    fn check_valid(c: *const c_char) -> Result<(), String> {
        let mut e = Error::empty();
        let b = unsafe { ffi::$s(c, e.get_mut()) };
        if b != 0 { Ok(()) } else { Err(e.message().unwrap().into()) }
    }

    /// Creates a new instance of this struct.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn new<S: Into<Vec<u8>>>(s: S) -> Result<$t<'m>, String> {
        let c = CString::new(s).map_err(|e| e.to_string())?;
        $t::check_valid(c.as_ptr()).map(|_| $t(Cow::Owned(c)))
    }

    /// Creates a new instance of this struct. If you end it with \0,
    /// it can borrow the slice without extra allocation.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn from_slice(s: &'m [u8]) -> Result<$t<'m>, String> {
        if s.len() == 0 || s[s.len()-1] != 0 { return $t::new(s) };
        $t::check_valid(s.as_ptr() as *const c_char).map(|_| {
            let c = unsafe { CStr::from_ptr(s.as_ptr() as *const c_char) };
            $t(Cow::Borrowed(c))
        })
    }

    /// This function creates a new instance of this struct, without checking.
    /// It's up to you to guarantee that s ends with a \0 and is valid.
    pub unsafe fn from_slice_unchecked(s: &'m [u8]) -> $t<'m> {
        debug_assert!(s[s.len()-1] == 0);
        $t(Cow::Borrowed(CStr::from_ptr(s.as_ptr() as *const c_char)))
    }

    /// View this struct as a CStr.
    pub fn as_cstr(&self) -> &CStr { &self.0 }

    #[allow(dead_code)]
    pub (crate) fn as_dstr(&self) -> &DStr { &self.0 }

    #[allow(dead_code)]
    pub (crate) unsafe fn from_dstr_unchecked(s: &'m [u8], _: &'m str) -> $t<'m> {
        Self::from_slice_unchecked(s)
    }

    /// Makes sure this string does not contain borrows.
    pub fn into_static(self) -> $t<'static> {
        $t(Cow::Owned(self.0.into_owned()))
    }

    /// Converts this struct to a CString.
    pub fn into_cstring(self) -> CString { self.0.into_owned() }
}


#[cfg(feature="native")]
impl<'m> $t<'m> {
    /// Creates a new instance of this struct.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn new<S: Into<String>>(s: S) -> Result<$t<'m>, String> {
        let c = s.into();
        $t::check_valid(&c).map(|_| $t(Cow::Owned(c)))
    }

    fn check_valid(s: &str) -> Result<(), String> {
        native::strings::$n(s.as_bytes()).map_err(|_| format!("'{}' is not a valid {}", s, stringify!($t)))
    }

    /// Creates a new instance of this struct. If you end it with \0,
    /// it can borrow the slice without extra allocation.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn from_slice(s: &'m [u8]) -> Result<$t<'m>, String> {
        let s = std::str::from_utf8(s).map_err(|e| e.to_string())?;
        $t::check_valid(s).map(|_| {
            $t(Cow::Borrowed(s))
        })
    }

    /// Makes sure this string does not contain borrows.
    pub fn into_static(self) -> $t<'static> {
        $t(Cow::Owned(self.0.into_owned()))
    }

    pub (crate) fn as_dstr(&self) -> &DStr { &self.0 }

    #[allow(dead_code)]
    pub (crate) unsafe fn from_dstr_unchecked(_: &'m [u8], s: &'m str) -> $t<'m> {
        $t(Cow::Borrowed(s))
    }
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

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m CStr> for $t<'m> { fn from(s: &'m CStr) -> $t<'m> { $t::from_slice(s.to_bytes_with_nul()).unwrap() } }

#[cfg(not(feature="native"))]
impl<'m> From<$t<'m>> for CString { fn from(s: $t<'m>) -> CString { s.0.into_owned() } }


/// #Panics
///
/// If given string is not valid.
impl<'m> From<Cow<'m, str>> for $t<'m> {
    fn from(s: Cow<'m, str>) -> $t<'m> {
        match s {
            Cow::Borrowed(z) => z.into(),
            Cow::Owned(z) => z.into(),
        }
    }
}

impl<'inner, 'm: 'inner> From<&'m $t<'inner>> for $t<'m> {
    fn from(borrow: &'m $t<'inner>) -> $t<'m> {
        $t(Cow::Borrowed(borrow.0.borrow()))
    }
}

impl<'m> ops::Deref for $t<'m> {
    type Target = str;
    #[cfg(feature="native")]
    fn deref(&self) -> &str { &self.0 }
    #[cfg(not(feature="native"))]
    fn deref(&self) -> &str { str::from_utf8(self.0.to_bytes()).unwrap() }
}

impl<'m> fmt::Display for $t<'m> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(self, f)
    }
}

#[cfg(not(feature="native"))]
impl<'m> AsRef<CStr> for $t<'m> {
    fn as_ref(&self) -> &CStr { &self.0 }
}

impl<'m> hash::Hash for $t<'m> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

}}

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid (single) D-Bus type signature.
    Signature, dbus_signature_validate_single, is_valid_signature_single
);

impl Signature<'static> {
    /// Makes a D-Bus signature that corresponds to A.
    pub fn make<A: super::arg::Arg>() -> Signature<'static> { A::signature() }
}

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid D-Bus object path.
    Path, dbus_validate_path, is_valid_object_path
);

// This is needed so one can make arrays of paths easily
impl<'a> default::Default for Path<'a> {
    fn default() -> Path<'a> { unsafe { Path::from_dstr_unchecked(b"/\0", "/") } }
}

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid D-Bus member, i e, a signal or method name.
    Member, dbus_validate_member, is_valid_member_name
);

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid D-Bus interface name.
    Interface, dbus_validate_interface, is_valid_interface_name
);

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid D-Bus bus name.
    BusName, dbus_validate_bus_name, is_valid_bus_name
);

dstring_wrapper!(
    /// A wrapper around a string that is guaranteed to be
    /// a valid D-Bus error name.
    ErrorName, dbus_validate_error_name, is_valid_error_name
);

#[test]
fn some_path() {
    use std::os::raw::c_char;
    let p1: Path = "/valid".into();
    let p2 = Path::new("##invalid##");

    #[cfg(feature="native")] {
        assert_eq!(p1, Path(Cow::Borrowed("/valid")));
        #[cfg(not(feature = "no-string-validation"))]
        assert_eq!(p2, Err("'##invalid##' is not a valid Path".into()));
        #[cfg(feature = "no-string-validation")]
        assert_eq!(p2, Ok(Path(Cow::Borrowed(unsafe { CStr::from_ptr(b"##invalid##\0".as_ptr() as *const c_char) }))));
    }
    #[cfg(not(feature="native"))] {
        let p3 = unsafe { CStr::from_ptr(b"/valid\0".as_ptr() as *const c_char) };
        assert_eq!(p1, Path(Cow::Borrowed(p3)));
        #[cfg(not(feature = "no-string-validation"))]
        assert_eq!(p2, Err("Object path was not valid: '##invalid##'".into()));
        #[cfg(feature = "no-string-validation")]
        assert_eq!(p2, Ok(Path(Cow::Borrowed(unsafe { CStr::from_ptr(b"##invalid##\0".as_ptr() as *const c_char) }))));
    }
}

#[test]
fn reborrow_path() {
    let p1 = Path::from("/valid");
    let p2 = p1.clone();
    {
        let p2_borrow: &Path = &p2;
        let p3 = Path::from(p2_borrow);
        // Check path created from borrow
        assert_eq!(p2, p3);
    }
    // Check path that was previously borrowed
    assert_eq!(p1, p2);
}

#[test]
fn make_sig() {
    assert_eq!(&*Signature::make::<(&str, u8)>(), "(sy)");
}
