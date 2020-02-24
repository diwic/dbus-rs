#![warn(missing_docs)]

//! A small crate which has a Rust native implementation of different kinds of D-Bus string types.


use std::borrow::Cow;
use std::borrow::Borrow;
use std::fmt;
use std::error::Error;
use std::ops::Deref;
use std::convert::TryFrom;

mod validity;

/// The supplied string was not a valid string of the desired type.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct InvalidStringError(&'static str);

impl fmt::Display for InvalidStringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "String is not a valid {}", self.0)
    }
}

impl Error for InvalidStringError {}

/// A D-Bus string-like type - a basic (non-container) type with variable length.
///
/// It wraps a str, which means that it is unsized.
pub trait StringLike: ToOwned {
    /// The name of the type
    const NAME: &'static str;

    /// Creates a new borrowed string
    fn new(s: &str) -> Result<&Self, InvalidStringError> {
        Self::is_valid(s)?;
        Ok(Self::new_unchecked(s))
    }

    /// Creates a new owned string
    fn new_owned<S: Into<String>>(s: S) -> Result<<Self as ToOwned>::Owned, InvalidStringError> {
        let s = s.into();
        Self::is_valid(&s)?;
        Ok(Self::new_unchecked_owned(s))
    }

    /// Creates a new borrowed string without actually checking that it is valid.
    ///
    /// Sending this over D-Bus if actually invalid, could result in e g immediate disconnection
    /// from the server.
    fn new_unchecked(_: &str) -> &Self;

    /// Creates a new owned string without actually checking that it is valid.
    ///
    /// Sending this over D-Bus if actually invalid, could result in e g immediate disconnection
    /// from the server.
    fn new_unchecked_owned(_: String) -> <Self as ToOwned>::Owned;

    /// Checks whether or not a string is valid.
    fn is_valid(_: &str)  -> Result<(), InvalidStringError>;
}

macro_rules! string_wrapper_base {
    ($(#[$comment:meta])* $t: ident, $towned: ident) => {
        $(#[$comment])*
        #[repr(transparent)]
        #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $t(str);

        impl Deref for $t {
            type Target = str;
            fn deref(&self) -> &str { &self.0 }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.fmt(f) }
        }

        impl AsRef<str> for $t {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl ToOwned for $t {
            type Owned = $towned;
            fn to_owned(&self) -> $towned { $towned(self.0.into()) }
        }

        impl<'a> TryFrom<&'a str> for &'a $t {
            type Error = InvalidStringError;
            fn try_from(s: &'a str) -> Result<&'a $t, Self::Error> { $t::new(s) }
        }

        $(#[$comment])*
        #[repr(transparent)]
        #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
        pub struct $towned(String);

        impl $towned {
            /// Creates a new string.
            pub fn new<S: Into<String>>(s: S) -> Result<Self, InvalidStringError> {
                $t::new_owned(s)
            }

            /// Unwraps the inner String.
            pub fn into_inner(self) -> String { self.0 }
        }

        impl Deref for $towned {
            type Target = $t;
            fn deref(&self) -> &$t { $t::new_unchecked(&self.0) }
        }

        impl Borrow<$t> for $towned {
            fn borrow(&self) -> &$t { &self }
        }

        impl fmt::Display for $towned {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.fmt(f) }
        }
        
        impl TryFrom<String> for $towned {
            type Error = InvalidStringError;
            fn try_from(s: String) -> Result<$towned, Self::Error> { $towned::new(s) }
        }

        impl<'a> From<$towned> for Cow<'a, $t> {
            fn from(s: $towned) -> Cow<'a,  $t> { Cow::Owned(s) }
        }

        impl<'a> From<&'a $t> for Cow<'a, $t> {
            fn from(s: &'a $t) -> Cow<'a, $t> { Cow::Borrowed(s) }
        }

        impl<'a> From<&'a $towned> for Cow<'a, $t> {
            fn from(s: &'a $towned) -> Cow<'a, $t> { Cow::Borrowed(&s) }
        }
    }
}

macro_rules! string_wrapper {
    ($(#[$comment:meta])* $t: ident, $towned: ident, $validate: ident) => {
        string_wrapper_base!($(#[$comment])* $t, $towned);

        impl StringLike for $t {
            const NAME: &'static str = stringify!($t);
            fn new_unchecked(s: &str) -> &Self {
                // Unfortunately we have to go unsafe here - there is no safe way to wrap an unsized
                // type into a newtype.
                // We know it's sound because of repr(transparent)
                unsafe { std::mem::transmute(s) }
            }
            fn new_unchecked_owned(s: String) -> $towned {
                $towned(s)
            }
            fn is_valid(s: &str) -> Result<(), InvalidStringError> {
                validity::$validate(s.as_bytes()).map_err(|_| InvalidStringError(Self::NAME))
            }
        }

        impl<'a> From<&'a $t> for &'a DBusStr {
            fn from(s: &'a $t) -> &'a DBusStr { DBusStr::new_unchecked(&*s) }
        }

        impl From<$towned> for DBusString {
            fn from(s: $towned) -> DBusString { DBusStr::new_unchecked_owned(s.into_inner()) }
        }
    }
}

string_wrapper_base!(
    /// A D-Bus string must be valid UTF-8 and contain no interior nul bytes.
    DBusStr, DBusString
);

impl StringLike for DBusStr {
    const NAME: &'static str = "DBusStr";
    fn new_unchecked(s: &str) -> &Self {
        // Unfortunately we have to go unsafe here - there is no safe way to wrap an unsized
        // type into a newtype.
        // We know it's sound because of repr(transparent)
        unsafe { std::mem::transmute(s) }
    }
    fn new_unchecked_owned(s: String) -> DBusString {
        DBusString(s)
    }
    fn is_valid(s: &str) -> Result<(), InvalidStringError> {
        validity::is_valid_string(s).map_err(|_| InvalidStringError(Self::NAME))
    }
}

string_wrapper!(
    /// A D-Bus interface name is usually something like "org.freedesktop.DBus"
    ///
    /// For exact rules see the D-Bus specification.
    InterfaceName, InterfaceNameBuf, is_valid_interface_name
);

string_wrapper!(
    /// A D-Bus member name is usually something like "Hello", a single identifier without special
    /// characters.
    ///
    /// For exact rules see the D-Bus specification.
    MemberName, MemberNameBuf, is_valid_member_name
);

string_wrapper!(
    /// A D-Bus error name is usually something like "org.freedesktop.DBus.Error.Failed"
    ///
    /// For exact rules see the D-Bus specification.
    ErrorName, ErrorNameBuf, is_valid_error_name
);

string_wrapper!(
    /// A D-Bus bus name is either something like "com.example.MyService" or ":1.54"
    ///
    /// For exact rules see the D-Bus specification.
    BusName, BusNameBuf, is_valid_bus_name
);

impl<'a> From<&'a SignatureSingle> for &'a SignatureMulti {
    fn from(s: &'a SignatureSingle) -> &'a SignatureMulti { SignatureMulti::new_unchecked(&s.0) }
}

impl From<SignatureSingleBuf> for SignatureMultiBuf {
    fn from(s: SignatureSingleBuf) -> SignatureMultiBuf { SignatureMulti::new_unchecked_owned(s.0) }
}


string_wrapper!(
    /// A D-Bus type signature of a single type, e g "b" or "a{sv}" but not "ii"
    ///
    /// For exact rules see the D-Bus specification.
    SignatureSingle, SignatureSingleBuf, is_valid_signature_single
);

string_wrapper!(
    /// A D-Bus type signature of zero or more types, e g "ii" or "sa{sv}"
    ///
    /// For exact rules see the D-Bus specification.
    SignatureMulti, SignatureMultiBuf, is_valid_signature_multi
);

string_wrapper!(
    /// A D-Bus object path is usually something like "/org/freedesktop/DBus".
    ///
    /// For exact rules see the D-Bus specification.
    ObjectPath, ObjectPathBuf, is_valid_object_path
);

#[test]
fn type_conversions() {
    use std::borrow::Cow;
    let x: &ObjectPath = ObjectPath::new("/test").unwrap();
    let y: ObjectPathBuf = ObjectPath::new_owned("/test").unwrap();
    assert_eq!(x, &*y);

    let x = Cow::from(x);
    let y = Cow::from(y);
    assert_eq!(x, y);

    let x: &DBusStr = (&*x).into();
    let y = DBusString::from(y.into_owned());
    assert_eq!(x, &*y);
}

#[test]
fn errors() {
    let q = MemberName::new("Hello.world").unwrap_err();
    assert_eq!(q.to_string(), "String is not a valid MemberName".to_string());
}
