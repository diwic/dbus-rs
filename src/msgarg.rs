#![allow(dead_code)]

use super::{ffi, Message};
use super::message::get_message_ptr;
use std::{mem, ptr};
use std::marker::PhantomData;

use std::ffi::{CStr, CString};
use std::os::raw::{c_void, c_char};
use super::{Signature, Path, OwnedFd};

fn check(f: &str, i: u32) { if i == 0 { panic!("D-Bus error: '{}' failed", f) }} 

fn ffi_iter() -> ffi::DBusMessageIter { unsafe { mem::zeroed() }} 

fn arg_append_basic(i: *mut ffi::DBusMessageIter, arg_type: i32, v: i64) {
    let p = &v as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, p));
    };
}

fn arg_get_basic(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<i64> {
    let mut c = 0i64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut c_void);
    }
    Some(c)
}

fn arg_append_f64(i: *mut ffi::DBusMessageIter, arg_type: i32, v: f64) {
    let p = &v as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, p));
    };
}

fn arg_get_f64(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<f64> {
    let mut c = 0f64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut c_void);
    }
    Some(c)
}

fn arg_append_str(i: *mut ffi::DBusMessageIter, arg_type: i32, v: &CStr) {
    let p = v.as_ptr();
    let q = &p as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, q));
    };
}

unsafe fn arg_get_str<'a>(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<&'a CStr> {
    if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
    let mut p = ptr::null_mut();
    ffi::dbus_message_iter_get_basic(i, &mut p as *mut _ as *mut c_void);
    Some(CStr::from_ptr(p as *const c_char))
}

/// Types that can represent a D-Bus message argument implement this trait.
///
/// Types should also implement either Append or Get to be useful. 
pub trait Arg {
    /// The corresponding D-Bus argument type code. 
    fn arg_type() -> i32;
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
    fn arg_type() -> i32 { ffi::$s }
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

integer_impl!(u8, DBUS_TYPE_BYTE, b"y\0");
integer_impl!(i16, DBUS_TYPE_INT16, b"n\0");
integer_impl!(u16, DBUS_TYPE_UINT16, b"q\0");
integer_impl!(i32, DBUS_TYPE_INT32, b"i\0");
integer_impl!(u32, DBUS_TYPE_UINT32, b"u\0");
integer_impl!(i64, DBUS_TYPE_INT64, b"x\0");
integer_impl!(u64, DBUS_TYPE_UINT64, b"t\0");


impl Arg for bool {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_BOOLEAN }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_UNIX_FD }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_DOUBLE }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRING }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRING }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRING }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_OBJECT_PATH }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_SIGNATURE }
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
    fn arg_type() -> i32 { T::arg_type() }
    fn signature() -> Signature<'static> { T::signature() }
}
impl<'a, T: Append + Clone> Append for &'a T {
    fn append(self, i: &mut IterAppend) { self.clone().append(i) }
}
impl<'a, T: DictKey> DictKey for &'a T {}


// Map DBus-Type -> Alignment. Copied from _dbus_marshal_write_fixed_multi in
// http://dbus.freedesktop.org/doc/api/html/dbus-marshal-basic_8c_source.html#l01020
// Note that Rust booleans are one byte, dbus booleans are four bytes!
const FIXED_ARRAY_ALIGNMENTS: [(i32, usize); 9] = [
    (ffi::DBUS_TYPE_BYTE, 1),
    (ffi::DBUS_TYPE_INT16, 2),
    (ffi::DBUS_TYPE_UINT16, 2),
    (ffi::DBUS_TYPE_UINT32, 4),
    (ffi::DBUS_TYPE_INT32, 4),
    (ffi::DBUS_TYPE_BOOLEAN, 4),
    (ffi::DBUS_TYPE_INT64, 8),
    (ffi::DBUS_TYPE_UINT64, 8),
    (ffi::DBUS_TYPE_DOUBLE, 8)
];

/// Represents a D-Bus array.
impl<'a, T: Arg> Arg for &'a [T] {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
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
                ffi::dbus_message_iter_append_fixed_array(&mut s.0, a.0, &zptr as *const _ as *const c_void, zlen)) }}
            else { for arg in z { arg.clone().append(s) }});
    }
}

impl<'a, T: Get<'a> + FixedArray> Get<'a> for &'a [T] {
    fn get(i: &mut Iter<'a>) -> Option<&'a [T]> {
        debug_assert!(FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == (T::arg_type(), mem::size_of::<T>())));
        i.recurse(Self::arg_type()).and_then(|mut si| unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut si.0) != T::arg_type() { return None };

            let mut v = ptr::null_mut();
            let mut i = 0;
            ffi::dbus_message_iter_get_fixed_array(&mut si.0, &mut v as *mut _ as *mut c_void, &mut i);
            Some(::std::slice::from_raw_parts(v, i as usize))
        })
    }
}


#[derive(Copy, Clone)]
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{}", Self::entry_sig())) }
}

impl<'a, K: 'a + DictKey + Append, V: 'a + Append + Arg, I: Iterator<Item=(K, V)>> Append for Dict<'a, K, V, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&CString::new(Self::entry_sig()).unwrap()), |s| for (k, v) in z {
            s.append_container(ffi::DBUS_TYPE_DICT_ENTRY, None, |ss| {
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
        let i = self.0.recurse(ffi::DBUS_TYPE_DICT_ENTRY).and_then(|mut si| {
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_VARIANT }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
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
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRUCT }
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
        let si = i.recurse(ffi::DBUS_TYPE_STRUCT);
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

    fn append_container<F: FnOnce(&mut IterAppend<'a>)>(&mut self, arg_type: i32, sig: Option<&CStr>, f: F) {
        let mut s = IterAppend(ffi_iter(), self.1);
        let p = sig.map(|s| s.as_ptr()).unwrap_or(ptr::null());
        check("dbus_message_iter_open_container",
            unsafe { ffi::dbus_message_iter_open_container(&mut self.0, arg_type, p, &mut s.0) });
        f(&mut s);
        check("dbus_message_iter_close_container",
            unsafe { ffi::dbus_message_iter_close_container(&mut self.0, &mut s.0) });
    }
}


#[derive(Clone, Copy)]
/// Helper struct for retrieve one or more arguments from a Message.
/// Note that this is not a Rust iterator, because arguments are often of different types
pub struct Iter<'a>(ffi::DBusMessageIter, &'a Message);

impl<'a> Iter<'a> {
    /// Creates a new struct for iterating over the arguments of a message, starting with the first argument. 
    pub fn new(m: &'a Message) -> Iter<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init(get_message_ptr(m), &mut i) };
        Iter(i, m)
    }

    /// Returns the current argument, if T is the argument type. Otherwise returns None.
    pub fn get<T: Get<'a>>(&mut self) -> Option<T> {
        T::get(self)
    }

    /// The raw arg_type for the current item.
    /// Unlike Arg::arg_type, this requires access to self and is not a static method.
    /// You can match this against Arg::arg_type for different types to understand what type the current item is.  
    pub fn arg_type(&mut self) -> i32 { unsafe { ffi::dbus_message_iter_get_arg_type(&mut self.0) } }

    /// Returns false if there are no more items.
    pub fn next(&mut self) -> bool {
        unsafe { ffi::dbus_message_iter_next(&mut self.0) != 0 } 
    }

    /// If the current argument is a container of the specified arg_type, then a new
    /// Iter is returned which is for iterating over the contents inside the container.
    ///
    /// Primarily for internal use (the "get" function is more ergonomic), but could be
    /// useful for recursing into containers with unknown types.
    pub fn recurse(&mut self, arg_type: i32) -> Option<Iter<'a>> {
        let containers = [ffi::DBUS_TYPE_ARRAY, ffi::DBUS_TYPE_DICT_ENTRY, ffi::DBUS_TYPE_STRUCT, ffi::DBUS_TYPE_VARIANT];
        if !containers.iter().any(|&t| t == arg_type) { return None; }

        let mut subiter = ffi_iter();
        unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut self.0) != arg_type { return None };
            ffi::dbus_message_iter_recurse(&mut self.0, &mut subiter)
        }
        Some(Iter(subiter, self.1))
    }
}

use std::fmt;
impl<'a> fmt::Debug for Iter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut z = self.clone();
        let mut t = f.debug_tuple("Iter");
        loop {
            t.field(& match z.arg_type() {
                ffi::DBUS_TYPE_VARIANT => "Variant",
                ffi::DBUS_TYPE_ARRAY =>
                    if z.recurse(ffi::DBUS_TYPE_ARRAY).unwrap().arg_type() == ffi::DBUS_TYPE_DICT_ENTRY { "Dict" } else { "Array" },
                ffi::DBUS_TYPE_STRUCT => "(...)",
                ffi::DBUS_TYPE_STRING => "&str",
                ffi::DBUS_TYPE_OBJECT_PATH => "Path",
                ffi::DBUS_TYPE_SIGNATURE => "Signature",
                ffi::DBUS_TYPE_UNIX_FD => "OwnedFd",
                ffi::DBUS_TYPE_BOOLEAN => "bool",
                ffi::DBUS_TYPE_BYTE => "u8",
                ffi::DBUS_TYPE_INT16 => "i16",
                ffi::DBUS_TYPE_INT32 => "i32",
                ffi::DBUS_TYPE_INT64 => "i64",
                ffi::DBUS_TYPE_UINT16 => "u16",
                ffi::DBUS_TYPE_UINT32 => "u32",
                ffi::DBUS_TYPE_UINT64 => "u64",
                ffi::DBUS_TYPE_DOUBLE => "f64",
                ffi::DBUS_TYPE_INVALID => { break },
                _ => "Unknown?!"
            });
            if !z.next() { break }
        }
        t.finish()
    }  
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::super::{Connection, ConnectionItem, Message, BusType, Path, Signature};
    use super::{Array, Variant, Dict, Iter};

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

                    let mut g = m.iter_init();
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
