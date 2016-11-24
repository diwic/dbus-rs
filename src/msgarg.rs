#![allow(dead_code)]

use super::{ffi, Message};
use super::message::get_message_ptr;
use std::{mem, ptr, error, fmt};
use std::marker::PhantomData;

use std::ffi::{CStr, CString};
use std::os::raw::{c_void, c_char, c_int};
use super::{Signature, Path, OwnedFd};

fn check(f: &str, i: u32) { if i == 0 { panic!("D-Bus error: '{}' failed", f) }} 

fn ffi_iter() -> ffi::DBusMessageIter { unsafe { mem::zeroed() }} 

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

/// Type of Argument
///
/// use this to figure out, e g, which type of argument is at the current position of Iter. 
#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum ArgType {
    /// Dicts are Arrays of dict entries, so Dict types will have Array as ArgType.
    Array = ffi::DBUS_TYPE_ARRAY as u8,
    Variant = ffi::DBUS_TYPE_VARIANT as u8,
    Boolean = ffi::DBUS_TYPE_BOOLEAN as u8,
    /// This is also the ArgType returned when there are no more arguments available.
    Invalid = ffi::DBUS_TYPE_INVALID as u8,
    String = ffi::DBUS_TYPE_STRING as u8,
    DictEntry = ffi::DBUS_TYPE_DICT_ENTRY as u8,
    Byte = ffi::DBUS_TYPE_BYTE as u8,
    Int16 = ffi::DBUS_TYPE_INT16 as u8,
    UInt16 = ffi::DBUS_TYPE_UINT16 as u8,
    Int32 = ffi::DBUS_TYPE_INT32 as u8,
    UInt32 = ffi::DBUS_TYPE_UINT32 as u8,
    Int64 = ffi::DBUS_TYPE_INT64 as u8,
    UInt64 = ffi::DBUS_TYPE_UINT64 as u8,
    Double = ffi::DBUS_TYPE_DOUBLE as u8,
    UnixFd = ffi::DBUS_TYPE_UNIX_FD as u8,
    Struct = ffi::DBUS_TYPE_STRUCT as u8,
    ObjectPath = ffi::DBUS_TYPE_OBJECT_PATH as u8,
    Signature = ffi::DBUS_TYPE_SIGNATURE as u8,
}

/// Types that can represent a D-Bus message argument implement this trait.
///
/// Types should also implement either Append or Get to be useful. 
pub trait Arg {
    /// The corresponding D-Bus argument type code. 
    fn arg_type() -> ArgType;
    /// The corresponding D-Bus type signature for this type. 
    fn signature() -> Signature<'static>;
}

/// Types that can be appended to a message as arguments implement this trait.
pub trait Append: Sized {
    /// Performs the append operation.
    fn append(self, &mut IterAppend);
}

/// Types that can be retrieved from a message as arguments implement this trait.
pub trait Get<'a>: Sized {
    /// Performs the get operation.
    fn get(i: &mut Iter<'a>) -> Option<Self>;
}

/// If a type implements this trait, it means the size and alignment is the same
/// as in D-Bus. This means that you can quickly append and get slices of this type.
///
/// Note: Booleans do not implement this trait because D-Bus booleans are 4 bytes and Rust booleans are 1 byte.
pub unsafe trait FixedArray: Arg {}

/// Types that can be used as keys in a dict type implement this trait. 
pub trait DictKey: Arg {}

macro_rules! integer_impl {
    ($t: ident, $s: ident, $f: expr) => {

impl Arg for $t {
    /// Returns the D-Bus argument type.
    ///
    /// This should probably be an associated constant instead, but those are still experimental. 
    fn arg_type() -> ArgType { ArgType::$s }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl Append for $t {
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, Self::arg_type(), self as i64) }
}

impl<'a> Get<'a> for $t {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, Self::arg_type()).map(|q| q as $t) }
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


impl Arg for bool {
    fn arg_type() -> ArgType { ArgType::Boolean }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"b\0") } }
}
impl Append for bool {
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, Self::arg_type(), if self {1} else {0}) }
}
impl DictKey for bool {}
impl<'a> Get<'a> for bool {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, Self::arg_type()).map(|q| q != 0) }
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


impl Arg for f64 {
    fn arg_type() -> ArgType { ArgType::Double }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"d\0") } }
}
impl Append for f64 {
    fn append(self, i: &mut IterAppend) { arg_append_f64(&mut i.0, Self::arg_type(), self) }
}
impl DictKey for f64 {}
impl<'a> Get<'a> for f64 {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_f64(&mut i.0, Self::arg_type()) }
}
unsafe impl FixedArray for f64 {}

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
        arg_append_str(&mut i.0, Self::arg_type(), &z)
    }
}
impl<'a> DictKey for &'a str {}
impl<'a> Get<'a> for &'a str {
    fn get(i: &mut Iter<'a>) -> Option<&'a str> { unsafe { arg_get_str(&mut i.0, Self::arg_type()) }
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



impl<'a> Arg for Path<'a> {
    fn arg_type() -> ArgType { ArgType::ObjectPath }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"o\0") } }
}
impl<'a> DictKey for Path<'a> {}
impl<'a> Append for Path<'a> {
    fn append(self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, Self::arg_type(), self.as_cstr())
    }
}
impl<'a> Get<'a> for Path<'a> {
    fn get(i: &mut Iter<'a>) -> Option<Path<'a>> { unsafe { arg_get_str(&mut i.0, Self::arg_type()) }
        .map(|s| unsafe { Path::from_slice_unchecked(s.to_bytes_with_nul()) } ) }
}

impl<'a> Arg for Signature<'a> {
    fn arg_type() -> ArgType { ArgType::Signature }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"g\0") } }
}
impl<'a> DictKey for Signature<'a> {}
impl<'a> Append for Signature<'a> {
    fn append(self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, Self::arg_type(), self.as_cstr())
    }
}
impl<'a> Get<'a> for Signature<'a> {
    fn get(i: &mut Iter<'a>) -> Option<Signature<'a>> { unsafe { arg_get_str(&mut i.0, Self::arg_type()) }
        .map(|s| unsafe { Signature::from_slice_unchecked(s.to_bytes_with_nul()) } ) }
}


/// Simple lift over reference to value - this makes some iterators more ergonomic to use
impl<'a, T: Arg> Arg for &'a T {
    fn arg_type() -> ArgType { T::arg_type() }
    fn signature() -> Signature<'static> { T::signature() }
}
impl<'a, T: Append + Clone> Append for &'a T {
    fn append(self, i: &mut IterAppend) { self.clone().append(i) }
}
impl<'a, T: DictKey> DictKey for &'a T {}


// Map DBus-Type -> Alignment. Copied from _dbus_marshal_write_fixed_multi in
// http://dbus.freedesktop.org/doc/api/html/dbus-marshal-basic_8c_source.html#l01020
// Note that Rust booleans are one byte, dbus booleans are four bytes!
const FIXED_ARRAY_ALIGNMENTS: [(ArgType, usize); 9] = [
    (ArgType::Byte, 1),
    (ArgType::Int16, 2),
    (ArgType::UInt16, 2),	
    (ArgType::UInt32, 4),
    (ArgType::Int32, 4),
    (ArgType::Boolean, 4),
    (ArgType::Int64, 8),
    (ArgType::UInt64, 8),
    (ArgType::Double, 8)
];

/// Represents a D-Bus array.
impl<'a, T: Arg> Arg for &'a [T] {
    fn arg_type() -> ArgType { ArgType::Array }
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

/// Appends a D-Bus array. Note: In case you have a large array of a type that implements FixedArray,
/// using this method will be more efficient than using an Array.
impl<'a, T: Arg + Append + Clone> Append for &'a [T] {
    fn append(self, i: &mut IterAppend) {
        let z = self;
        let zptr = z.as_ptr();
        let zlen = z.len() as i32;

        // Can we do append_fixed_array?
        let a = (T::arg_type(), mem::size_of::<T>());
        let can_fixed_array = (zlen > 1) && (z.len() == zlen as usize) && FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == a);

        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s|
            if can_fixed_array { unsafe { check("dbus_message_iter_append_fixed_array",
                ffi::dbus_message_iter_append_fixed_array(&mut s.0, a.0 as c_int, &zptr as *const _ as *const c_void, zlen)) }}
            else { for arg in z { arg.clone().append(s) }});
    }
}

impl<'a, T: Get<'a> + FixedArray> Get<'a> for &'a [T] {
    fn get(i: &mut Iter<'a>) -> Option<&'a [T]> {
        debug_assert!(FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == (T::arg_type(), mem::size_of::<T>())));
        i.recurse(Self::arg_type()).and_then(|mut si| unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut si.0) != T::arg_type() as c_int { return None };

            let mut v = ptr::null_mut();
            let mut i = 0;
            ffi::dbus_message_iter_get_fixed_array(&mut si.0, &mut v as *mut _ as *mut c_void, &mut i);
            Some(::std::slice::from_raw_parts(v, i as usize))
        })
    }
}


#[derive(Copy, Clone, Debug)]
/// Append a D-Bus dict type (i e, an array of dict entries).
pub struct Dict<'a, K: DictKey, V: Arg, I>(I, PhantomData<(&'a Message, *const K, *const V)>);

impl<'a, K: DictKey, V: Arg, I> Dict<'a, K, V, I> {
    fn entry_sig() -> String { format!("{{{}{}}}", K::signature(), V::signature()) } 
}

impl<'a, K: 'a + DictKey, V: 'a + Append + Arg, I: Iterator<Item=(K, V)>> Dict<'a, K, V, I> {
    /// Creates a new Dict from an iterator. The iterator is consumed when appended.
    pub fn new<J: IntoIterator<IntoIter=I, Item=(K, V)>>(j: J) -> Dict<'a, K, V, I> { Dict(j.into_iter(), PhantomData) }
}

impl<'a, K: DictKey, V: Arg, I> Arg for Dict<'a, K, V, I> {
    fn arg_type() -> ArgType { ArgType::Array }
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{}", Self::entry_sig())) }
}

impl<'a, K: 'a + DictKey + Append, V: 'a + Append + Arg, I: Iterator<Item=(K, V)>> Append for Dict<'a, K, V, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&CString::new(Self::entry_sig()).unwrap()), |s| for (k, v) in z {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append(ss);
                v.append(ss);
            })
        });
    }
}


impl<'a, K: DictKey + Get<'a>, V: Arg + Get<'a>> Get<'a> for Dict<'a, K, V, Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        i.recurse(Self::arg_type()).map(|si| Dict(si, PhantomData))
        // TODO: Verify full element signature?
    }
}

impl<'a, K: DictKey + Get<'a>, V: Arg + Get<'a>> Iterator for Dict<'a, K, V, Iter<'a>> {
    type Item = (K, V);
    fn next(&mut self) -> Option<(K, V)> {
        let i = self.0.recurse(ArgType::DictEntry).and_then(|mut si| {
            let k = si.get();
            if k.is_none() { return None };
            assert!(si.next());
            let v = si.get(); 
            if v.is_none() { return None };
            Some((k.unwrap(), v.unwrap()))
        });
        self.0.next();
        i
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
/// A simple wrapper to specify a D-Bus variant.
pub struct Variant<T>(pub T);

impl<T> Arg for Variant<T> {
    fn arg_type() -> ArgType { ArgType::Variant }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"v\0") } }
}

impl<T: Arg + Append> Append for Variant<T> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s| z.append(s));
    }
}

impl Append for Variant<super::MessageItem> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        let sig = CString::new(z.type_sig().into_owned()).unwrap();
        i.append_container(Self::arg_type(), Some(&sig), |s| z.append(s));
    }
}


impl<'a, T: Get<'a>> Get<'a> for Variant<T> {
    fn get(i: &mut Iter<'a>) -> Option<Variant<T>> {
        i.recurse(Self::arg_type()).and_then(|mut si| si.get().map(|v| Variant(v)))
    }
}

impl<'a> Get<'a> for Variant<Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Variant<Iter<'a>>> {
        i.recurse(Self::arg_type()).map(|v| Variant(v))
    }
}

#[derive(Copy, Clone, Debug)]
/// Represents a D-Bus Array. Maximum flexibility (wraps an iterator of items to append). 
/// Note: Slices of FixedArray can be faster.
pub struct Array<'a, T, I>(I, PhantomData<(*const T, &'a Message)>);

impl<'a, T: 'a + Append, I: Iterator<Item=T>> Array<'a, T, I> {
    /// Creates a new Array from an iterator. The iterator is consumed when appending.
    pub fn new<J: IntoIterator<IntoIter=I, Item=T>>(j: J) -> Array<'a, T, I> { Array(j.into_iter(), PhantomData) }
}

impl<'a, T: Arg, I> Arg for Array<'a, T, I> {
    fn arg_type() -> ArgType { ArgType::Array }
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

impl<'a, T: 'a + Arg + Append, I: Iterator<Item=T>> Append for Array<'a, T, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s| for arg in z { arg.append(s) });
    }
}

impl<'a, T: Arg + Get<'a>> Get<'a> for Array<'a, T, Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Array<'a, T, Iter<'a>>> {
        i.recurse(Self::arg_type()).map(|si| Array(si, PhantomData))
        // TODO: Verify full element signature?
    }
}

impl<'a, T: Get<'a>> Iterator for Array<'a, T, Iter<'a>> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let i = self.0.get();
        self.0.next();
        i
    }
}

macro_rules! struct_impl {
    ( $($n: ident $t: ident,)+ ) => {

/// Tuples are represented as D-Bus structs. 
impl<$($t: Arg),*> Arg for ($($t,)*) {
    fn arg_type() -> ArgType { ArgType::Struct }
    fn signature() -> Signature<'static> {
        let mut s = String::from("(");
        $( s.push_str(&$t::signature()); )*
        s.push_str(")");
        Signature::from(s)
    }
}

impl<$($t: Arg + Append),*> Append for ($($t,)*) {
    fn append(self, i: &mut IterAppend) {
        let ( $($n,)*) = self;
        i.append_container(Self::arg_type(), None, |s| { $( $n.append(s); )* });
    }
}

impl<'a, $($t: Get<'a>),*> Get<'a> for ($($t,)*) {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        let si = i.recurse(ArgType::Struct);
        if si.is_none() { return None; }
        let mut si = si.unwrap();
        let mut _valid_item = true;
        $(
            if !_valid_item { return None; }
            let $n: Option<$t> = si.get();
            if $n.is_none() { return None; }
            _valid_item = si.next();
        )*
        Some(($( $n.unwrap(), )* ))
    }
}

}} // macro_rules end

struct_impl!(a A,);
struct_impl!(a A, b B,);
struct_impl!(a A, b B, c C,);
struct_impl!(a A, b B, c C, d D,);
struct_impl!(a A, b B, c C, d D, e E,);
struct_impl!(a A, b B, c C, d D, e E, f F,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K, l L,);

impl Append for super::MessageItem {
    fn append(self, i: &mut IterAppend) {
        super::message::append_messageitem(&mut i.0, &self)
    }
}

impl<'a> Get<'a> for super::MessageItem {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        super::message::get_messageitem(&mut i.0)
    }
}


fn test_compile() {
    let mut q = IterAppend::new(unsafe { mem::transmute(0usize) });

    q.append(5u8);
    q.append(Array::new(&[5u8, 6, 7]));
    q.append((8u8, &[9u8, 6, 7][..]));
    q.append(Variant((6u8, 7u8)));
}

#[derive(Clone, Copy)]
/// Helper struct for appending one or more arguments to a Message. 
pub struct IterAppend<'a>(ffi::DBusMessageIter, &'a Message);

impl<'a> IterAppend<'a> {
    /// Creates a new IterAppend struct.
    pub fn new(m: &'a mut Message) -> IterAppend<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init_append(get_message_ptr(m), &mut i) };
        IterAppend(i, m)
    }

    /// Appends the argument.
    pub fn append<T: Append>(&mut self, a: T) { a.append(self) }

    fn append_container<F: FnOnce(&mut IterAppend<'a>)>(&mut self, arg_type: ArgType, sig: Option<&CStr>, f: F) {
        let mut s = IterAppend(ffi_iter(), self.1);
        let p = sig.map(|s| s.as_ptr()).unwrap_or(ptr::null());
        check("dbus_message_iter_open_container",
            unsafe { ffi::dbus_message_iter_open_container(&mut self.0, arg_type as c_int, p, &mut s.0) });
        f(&mut s);
        check("dbus_message_iter_close_container",
            unsafe { ffi::dbus_message_iter_close_container(&mut self.0, &mut s.0) });
    }

    /// Low-level function to append a variant.
    ///
    /// Use in case the `Variant` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Variant` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f" you need to call "append" on
    /// the supplied `IterAppend` exactly once,
    /// and with a value which has the same signature as inner_sig.  
    pub fn append_variant<F: FnOnce(&mut IterAppend<'a>)>(&mut self, inner_sig: &Signature, f: F) {
        self.append_container(ArgType::Variant, Some(inner_sig.as_cstr()), f)
    }

    /// Low-level function to append an array.
    ///
    /// Use in case the `Array` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply an `Array` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should only call "append" on
    /// the supplied `IterAppend` with values which has the same signature as inner_sig.
    pub fn append_array<F: FnOnce(&mut IterAppend<'a>)>(&mut self, inner_sig: &Signature, f: F) {
        self.append_container(ArgType::Array, Some(inner_sig.as_cstr()), f)
    }

    /// Low-level function to append a struct.
    ///
    /// Use in case tuples are not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a tuple parameter.
    pub fn append_struct<F: FnOnce(&mut IterAppend<'a>)>(&mut self, f: F) {
        self.append_container(ArgType::Struct, None, f)
    }

    /// Low-level function to append a dict entry.
    ///
    /// Use in case the `Dict` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Dict` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should call "append" once
    /// for the key, then once for the value. You should only call this function for a subiterator
    /// you got from calling "append_dict", and signatures need to match what you specified in "append_dict".
    pub fn append_dict_entry<F: FnOnce(&mut IterAppend<'a>)>(&mut self, f: F) {
        self.append_container(ArgType::DictEntry, None, f)
    }

    /// Low-level function to append a dict.
    ///
    /// Use in case the `Dict` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Dict` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should only call "append_dict_entry"
    /// for the subiterator - do this as many times as the number of dict entries.
    pub fn append_dict<F: FnOnce(&mut IterAppend<'a>)>(&mut self, key_sig: &Signature, value_sig: &Signature, f: F) {
        let sig = format!("{{{}{}}}", key_sig, value_sig);
        self.append_container(Array::<bool,()>::arg_type(), Some(&CString::new(sig).unwrap()), f);
    }

}

const ALL_ARG_TYPES: [(ArgType, &'static str); 18] =
    [(ArgType::Variant, "Variant"),
    (ArgType::Array, "Array/Dict"),
    (ArgType::Struct, "Struct"),
    (ArgType::String, "String"),
    (ArgType::DictEntry, "Dict entry"),
    (ArgType::ObjectPath, "Path"),
    (ArgType::Signature, "Signature"),
    (ArgType::UnixFd, "OwnedFd"),
    (ArgType::Boolean, "bool"),
    (ArgType::Byte, "u8"),
    (ArgType::Int16, "i16"),
    (ArgType::Int32, "i32"),
    (ArgType::Int64, "i64"),
    (ArgType::UInt16, "u16"),
    (ArgType::UInt32, "u32"),
    (ArgType::UInt64, "u64"),
    (ArgType::Double, "f64"),
    (ArgType::Invalid, "nothing")];

impl ArgType {
    /// A str corresponding to the name of a Rust type. 
    pub fn as_str(self) -> &'static str {
        ALL_ARG_TYPES.iter().skip_while(|a| a.0 != self).next().unwrap().1
    }
}

/// Error struct to indicate a D-Bus argument type mismatch.
///
/// Might be returned from `iter::read()`. 
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeMismatchError {
    expected: ArgType,
    found: ArgType,
    position: u32,
}

impl TypeMismatchError {
    /// The ArgType we were trying to read, but failed
    pub fn expected_arg_type(&self) -> ArgType { self.expected }

    /// The ArgType we should have been trying to read, if we wanted the read to succeed 
    pub fn found_arg_type(&self) -> ArgType { self.found }

    /// At what argument was the error found?
    ///
    /// Returns 0 for first argument, 1 for second argument, etc.
    pub fn pos(&self) -> u32 { self.position }
}

impl error::Error for TypeMismatchError {
    fn description(&self) -> &str { "D-Bus argument type mismatch" }
    fn cause(&self) -> Option<&error::Error> { None }
}

impl fmt::Display for TypeMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} at position {}: expected {}, found {}",
            (self as &error::Error).description(),
            self.position, self.expected.as_str(),
            if self.expected == self.found { "same but still different somehow" } else { self.found.as_str() }
        )
    }
}

#[derive(Clone, Copy)]
/// Helper struct for retrieve one or more arguments from a Message.
/// Note that this is not a Rust iterator, because arguments are often of different types
pub struct Iter<'a>(ffi::DBusMessageIter, &'a Message, u32);

impl<'a> Iter<'a> {
    /// Creates a new struct for iterating over the arguments of a message, starting with the first argument. 
    pub fn new(m: &'a Message) -> Iter<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init(get_message_ptr(m), &mut i) };
        Iter(i, m, 0)
    }

    /// Returns the current argument, if T is the argument type. Otherwise returns None.
    pub fn get<T: Get<'a>>(&mut self) -> Option<T> {
        T::get(self)
    }

    /// Returns the type signature for the current argument.
    pub fn signature(&mut self) -> Signature<'static> {
        unsafe {
            let c = ffi::dbus_message_iter_get_signature(&mut self.0);
            assert!(c != ptr::null_mut());
            let cc = CStr::from_ptr(c);
            let r = Signature::new(cc.to_bytes());
            ffi::dbus_free(c as *mut c_void);
            r.unwrap()
        } 
    }

    /// The raw arg_type for the current item.
    ///
    /// Unlike Arg::arg_type, this requires access to self and is not a static method.
    /// You can match this against Arg::arg_type for different types to understand what type the current item is.
    /// In case you're past the last argument, this function will return 0.
    pub fn arg_type(&mut self) -> ArgType {
        let s = unsafe { ffi::dbus_message_iter_get_arg_type(&mut self.0) };
        for &(a, _) in &ALL_ARG_TYPES {
            if a as c_int == s { return a; }
        }
        panic!("Invalid arg_type {} returned from D-Bus", s);
    }

    /// Returns false if there are no more items.
    pub fn next(&mut self) -> bool {
        self.2 += 1;
        unsafe { ffi::dbus_message_iter_next(&mut self.0) != 0 } 
    }

    /// Wrapper around `get` and `next`. Calls `get`, and then `next` if `get` succeeded. 
    ///
    /// Also returns a `Result` rather than an `Option` to work better with `try!`.
    ///
    /// # Example
    /// ```ignore
    /// struct ServiceBrowserItemNew {
    ///     interface: i32,
    ///     protocol: i32,
    ///     name: String,
    ///     item_type: String,
    ///     domain: String,
    ///     flags: u32,
    /// }
    ///
    /// fn service_browser_item_new_msg(m: &Message) -> Result<ServiceBrowserItemNew, TypeMismatchError> {
    ///     let mut iter = m.iter_init();
    ///     Ok(ServiceBrowserItemNew {
    ///         interface: try!(iter.read()),
    ///         protocol: try!(iter.read()),
    ///         name: try!(iter.read()),
    ///         item_type: try!(iter.read()),
    ///         domain: try!(iter.read()),
    ///         flags: try!(iter.read()),
    ///     })
    /// }
    /// ```
    pub fn read<T: Arg + Get<'a>>(&mut self) -> Result<T, TypeMismatchError> {
        let r = try!(self.get().ok_or_else(||
             TypeMismatchError { expected: T::arg_type(), found: self.arg_type(), position: self.2 }));
        self.next();
        Ok(r)
    }

    /// If the current argument is a container of the specified arg_type, then a new
    /// Iter is returned which is for iterating over the contents inside the container.
    ///
    /// Primarily for internal use (the "get" function is more ergonomic), but could be
    /// useful for recursing into containers with unknown types.
    pub fn recurse(&mut self, arg_type: ArgType) -> Option<Iter<'a>> {
        let containers = [ArgType::Array, ArgType::DictEntry, ArgType::Struct, ArgType::Variant];
        if !containers.iter().any(|&t| t == arg_type) { return None; }

        let mut subiter = ffi_iter();
        unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut self.0) != arg_type as c_int { return None };
            ffi::dbus_message_iter_recurse(&mut self.0, &mut subiter)
        }
        Some(Iter(subiter, self.1, 0))
    }
}

impl<'a> fmt::Debug for Iter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut z = self.clone();
        let mut t = f.debug_tuple("Iter");
        loop {
            t.field(&z.arg_type());
            if !z.next() { break }
        }
        t.finish()
    }  
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::super::{Connection, ConnectionItem, Message, BusType, Path, Signature};
    use super::{Array, Variant, Dict, Iter, ArgType, TypeMismatchError};

    use std::collections::HashMap;

    #[test]
    fn message_types() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        let m = m.append1(2000u16);
        let m = m.append1(Array::new(&vec![129u8, 5, 254]));
        let m = m.append2(Variant(&["Hello", "world"][..]), &[32768u16, 16u16, 12u16][..]);
        let m = m.append3(-1i32, &*format!("Hello world"), -3.14f64);
        let m = m.append1((256i16, Variant(18_446_744_073_709_551_615u64)));
        let m = m.append2(Path::new("/a/valid/path").unwrap(), Signature::new("a{sv}").unwrap());
        let mut z = HashMap::new();
        z.insert(123543u32, true);
        z.insert(0u32, false);
        let m = m.append1(Dict::new(&z));
        let sending = format!("{:?}", m.iter_init());
        println!("Sending {}", sending);
        c.send(m).unwrap();

        for n in c.iter(1000) {
            match n {
                ConnectionItem::MethodCall(m) => {
                    use super::Arg;
                    let receiving = format!("{:?}", m.iter_init());
                    println!("Receiving {}", receiving);
                    assert_eq!(sending, receiving);

                    assert_eq!(2000u16, m.get1().unwrap());
                    assert_eq!(m.get2(), (Some(2000u16), Some(&[129u8, 5, 254][..])));
                    assert_eq!(m.read2::<u16, bool>().unwrap_err(),
                        TypeMismatchError { position: 1, found: ArgType::Array, expected: ArgType::Boolean });

                    let mut g = m.iter_init();
                    let e = g.read::<u32>().unwrap_err();
                    assert_eq!(e.pos(), 0);
                    assert_eq!(e.expected_arg_type(), ArgType::UInt32);
                    assert_eq!(e.found_arg_type(), ArgType::UInt16);

                    assert!(g.next() && g.next());
                    let v: Variant<Iter> = g.get().unwrap();
                    let mut viter = v.0;
                    assert_eq!(viter.arg_type(), Array::<&str,()>::arg_type());
                    let a: Array<&str, _> = viter.get().unwrap();
                    assert_eq!(a.collect::<Vec<&str>>(), vec!["Hello", "world"]);

                    assert!(g.next());
                    assert_eq!(g.get::<u16>(), None); // It's an array, not a single u16
                    assert!(g.next() && g.next() && g.next() && g.next());

                    assert_eq!(g.get(), Some((256i16, Variant(18_446_744_073_709_551_615u64))));
                    assert!(g.next());
                    assert_eq!(g.get(), Some(Path::new("/a/valid/path").unwrap()));
                    assert!(g.next());
                    assert_eq!(g.get(), Some(Signature::new("a{sv}").unwrap()));
                    assert!(g.next());
                    let d: Dict<u32, bool, _> = g.get().unwrap();
                    let z2: HashMap<_, _> = d.collect();
                    assert_eq!(z, z2);
                    break;
                }
                _ => println!("Got {:?}", n),
            }
        }
    }
}
