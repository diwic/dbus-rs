#![allow(dead_code)]

use super::{ffi, libc, Message};
use super::message::get_message_ptr;
use std::{mem, ptr};
use std::marker::PhantomData;

use std::ffi::{CStr, CString};

use super::Signature;

fn check(f: &str, i: u32) { if i == 0 { panic!("D-Bus error: '{}' failed", f) }} 

fn ffi_iter() -> ffi::DBusMessageIter { unsafe { mem::zeroed() }} 

fn arg_append_basic(i: *mut ffi::DBusMessageIter, arg_type: i32, v: i64) {
    let p = &v as *const _ as *const libc::c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, p));
    };
}

fn arg_get_basic(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<i64> {
    let mut c = 0i64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut libc::c_void);
    }
    Some(c)
}

fn arg_append_f64(i: *mut ffi::DBusMessageIter, arg_type: i32, v: f64) {
    let p = &v as *const _ as *const libc::c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, p));
    };
}

fn arg_get_f64(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<f64> {
    let mut c = 0f64;
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut libc::c_void);
    }
    Some(c)
}

fn arg_append_str(i: *mut ffi::DBusMessageIter, arg_type: i32, v: &CStr) {
    let p = v.as_ptr();
    let q = &p as *const _ as *const libc::c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, q));
    };
}

unsafe fn arg_get_str<'a>(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<&'a CStr> {
    if ffi::dbus_message_iter_get_arg_type(i) != arg_type { return None };
    let mut p = ptr::null_mut();
    ffi::dbus_message_iter_get_basic(i, &mut p as *mut _ as *mut libc::c_void);
    Some(CStr::from_ptr(p as *const libc::c_char))
}

/// Types that can represent a D-Bus message argument implement this trait.
///
/// Types should also implement either Append or Get to be useful. 
pub trait Arg {
    fn arg_type() -> i32;
    fn signature() -> Signature<'static>;
}

/// Types that can be appended to a message as arguments implement this trait.
pub trait Append: Arg + Clone {
    fn append(self, &mut IterAppend);
}

/// Types that can be retrieved from a message as arguments implement this trait.
pub trait Get<'a>: Sized {
    fn get(i: &mut IterGet<'a>) -> Option<Self>;
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
    fn arg_type() -> i32 { ffi::$s }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl Append for $t {
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, Self::arg_type(), self as i64) }
}

impl<'a> Get<'a> for $t {
    fn get(i: &mut IterGet) -> Option<Self> { arg_get_basic(&mut i.0, Self::arg_type()).map(|q| q as $t) }
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
    fn get(i: &mut IterGet) -> Option<Self> { arg_get_basic(&mut i.0, Self::arg_type()).map(|q| q != 0) }
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
    fn get(i: &mut IterGet) -> Option<Self> { arg_get_f64(&mut i.0, Self::arg_type()) }
}
unsafe impl FixedArray for f64 {}

/// Represents a D-Bus string.
impl<'a> Arg for &'a str {
    /// Returns the D-Bus argument type.
    ///
    /// This should probably rather be an associated constant instead, but those are still experimental. 
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRING }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"s\0") } }
}

/// # Panic
/// FIXME: Will panic in case the str contains \0 characters.
impl<'a> Append for &'a str {
    fn append(self, i: &mut IterAppend) {
        let z = CString::new(self).unwrap(); // FIXME: Do not unwrap here
        arg_append_str(&mut i.0, Self::arg_type(), &z)
    }
}
impl<'a> DictKey for &'a str {}
impl<'a> Get<'a> for &'a str {
    fn get(i: &mut IterGet<'a>) -> Option<&'a str> { unsafe { arg_get_str(&mut i.0, Self::arg_type()) }
        .and_then(|s| s.to_str().ok()) }
}

/// Simple lift over reference to value - this makes some iterators more ergonomic to use
impl<'a, T: Arg> Arg for &'a T {
    fn arg_type() -> i32 { T::arg_type() }
    fn signature() -> Signature<'static> { T::signature() }
}

impl<'a, T: Append> Append for &'a T {
    fn append(self, i: &mut IterAppend) { self.clone().append(i) }
}

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
impl<'a, T: Append> Append for &'a [T] {
    fn append(self, i: &mut IterAppend) {
        let z = self;
        let zptr = z.as_ptr();
        let zlen = z.len() as i32;

        // Can we do append_fixed_array?
        let a = (T::arg_type(), mem::size_of::<T>());
        let can_fixed_array = (zlen > 1) && (z.len() == zlen as usize) && FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == a);

        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s|
            if can_fixed_array { unsafe { check("dbus_message_iter_append_fixed_array",
                ffi::dbus_message_iter_append_fixed_array(&mut s.0, a.0, &zptr as *const _ as *const libc::c_void, zlen)) }}
            else { for arg in z { arg.clone().append(s) }});
    }
}

impl<'a, T: Get<'a> + FixedArray> Get<'a> for &'a [T] {
    fn get(i: &mut IterGet<'a>) -> Option<&'a [T]> {
        debug_assert!(FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == (T::arg_type(), mem::size_of::<T>())));
        i.recurse(Self::arg_type()).and_then(|mut si| unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut si.0) != T::arg_type() { return None };

            let mut v = ptr::null_mut();
            let mut i = 0;
            ffi::dbus_message_iter_get_fixed_array(&mut si.0, &mut v as *mut _ as *mut libc::c_void, &mut i);
            Some(::std::slice::from_raw_parts(v, i as usize))
        })
    }
}


#[derive(Copy, Clone)]
/// Append a D-Bus dict type (as an array of dict entries).
pub struct Dict<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>>(I, PhantomData<&'a ()>);

impl<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>> Dict<'a, K, V, I> {
    fn entry_sig() -> String { format!("{{{}{}}}", K::signature(), V::signature()) } 
    pub fn new<J: IntoIterator<IntoIter=I, Item=(&'a K, &'a V)>>(j: J) -> Dict<'a, K, V, I> { Dict(j.into_iter(), PhantomData) }
}

impl<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>> Arg for Dict<'a, K, V, I> {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{}", Self::entry_sig())) }
}

impl<'a, K: 'a + DictKey + Append, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>> Append for Dict<'a, K, V, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&CString::new(Self::entry_sig()).unwrap()), |s| for (k, v) in z {
            s.append_container(ffi::DBUS_TYPE_DICT_ENTRY, None, |ss| {
                k.clone().append(ss);
                v.clone().append(ss);
            })
        });
    }
}


/*
#[derive(Copy, Clone)]
pub struct DictIter<'a, K, V>(IterGet<'a>, PhantomData<(K, V)>); 

impl<'a, K: Get<'a>, V: Get<'a>> Iterator for DictIter<'a, K, V> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.0.recurse(ffi::DBUS_TYPE_DICT_ENTRY).map(|si| {
            let k = si.get().unwrap();
            assert!(si.next());
            let v = si.get().unwrap();
            (k, v)
        });
        self.0.next();
        i
    }
}

impl<'a, K: DictKey + Get<'a>, V: Get<'a>> Get<'a> for Dict<'a, K, V, DictIter<'a, K, V>> {
    fn get(i: &mut IterGet<'a>) -> Option<Dict<'a, K, V, DictIter<'a, K, V>>> {
        i.recurse(Self::arg_type()).and_then(|mut si| unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut si.0) != ffi::DBUS_TYPE_DICT_ENTRY { return None };
            // FIXME: Verify signature so K and V are both correct
            Some(Dict(DictIter(si, PhantomData), PhantomData))
        })
    }
}

impl<'a, K: DictKey + Get<'a>, V: Get<'a>> IntoIterator for Dict<'a, K, V, DictIter<'a, K, V>> {
    type IntoIter=DictIter<'a, K, V>;
    type Item=(K, V);
    fn into_iter(self) -> DictIter<'a, K, V> { self.0 }
}
*/

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
/// A simple wrapper to specify a D-Bus variant.
pub struct Variant<T>(pub T);

impl<T> Arg for Variant<T> {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_VARIANT }
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked(b"v\0") } }
}

impl<T: Append> Append for Variant<T> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s| z.append(s));
    }
}

impl<'a, T: Get<'a>> Get<'a> for Variant<T> {
    fn get(i: &mut IterGet<'a>) -> Option<Variant<T>> {
        i.recurse(Self::arg_type()).and_then(|mut si| si.get())
    }
}

impl<'a> Get<'a> for Variant<IterGet<'a>> {
    fn get(i: &mut IterGet<'a>) -> Option<Variant<IterGet<'a>>> {
        i.recurse(Self::arg_type()).map(|v| Variant(v))
    }
}

#[derive(Copy, Clone, Debug)]
/// Represents a D-Bus Array. Maximum flexibility (wraps an iterator of items to append). 
/// Note: Slices of FixedArray can be faster.
pub struct Array<'a, T, I>(I, PhantomData<(*const T, &'a Message)>);

impl<'a, T: 'a + Append, I: Iterator<Item=&'a T>> Array<'a, T, I> {
    pub fn new<J: IntoIterator<IntoIter=I, Item=&'a T>>(j: J) -> Array<'a, T, I> { Array(j.into_iter(), PhantomData) }
}

impl<'a, T: Arg, I> Arg for Array<'a, T, I> {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

impl<'a, T: 'a + Append, I: Clone + Iterator<Item=&'a T>> Append for Array<'a, T, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(T::signature().as_cstr()), |s| for arg in z { arg.clone().append(s) });
    }
}

impl<'a, T: Arg + Get<'a>> Get<'a> for Array<'a, T, IterGet<'a>> {
    fn get(i: &mut IterGet<'a>) -> Option<Array<'a, T, IterGet<'a>>> {
        i.recurse(Self::arg_type()).map(|si| Array(si, PhantomData))
        // TODO: Verify full element signature?
    }
}

impl<'a, T: Get<'a>> Iterator for Array<'a, T, IterGet<'a>> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let i = self.0.get();
        self.0.next();
        i
    }
}

macro_rules! struct_append {
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

impl<$($t: Append),*> Append for ($($t,)*) {
    fn append(self, i: &mut IterAppend) {
        let ( $($n,)*) = self;
        i.append_container(Self::arg_type(), None, |s| { $( $n.append(s); )* });
    }
}

    }
} // macro_rules end

struct_append!(a A,);
struct_append!(a A, b B,);
struct_append!(a A, b B, c C,);
struct_append!(a A, b B, c C, d D,);
struct_append!(a A, b B, c C, d D, e E,);
struct_append!(a A, b B, c C, d D, e E, f F,);
struct_append!(a A, b B, c C, d D, e E, f F, g G,);
struct_append!(a A, b B, c C, d D, e E, f F, g G, h H,);
struct_append!(a A, b B, c C, d D, e E, f F, g G, h H, i I,);
struct_append!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J,);
struct_append!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K,);
struct_append!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K, l L,);

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
    pub fn new(m: &'a Message) -> IterAppend<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init_append(get_message_ptr(m), &mut i) };
        IterAppend(i, m)
    }

    pub fn append<T: Append>(&mut self, a: T) { a.append(self) }

    fn append_container<F: FnOnce(&mut IterAppend<'a>)>(&mut self, arg_type: i32, sig: Option<&CStr>, f: F) {
        let mut s = IterAppend(ffi_iter(), self.1);
        let p = sig.map(|s| s.as_ref().as_ptr()).unwrap_or(ptr::null());
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
pub struct IterGet<'a>(ffi::DBusMessageIter, &'a Message);

impl<'a> IterGet<'a> {
    pub fn new(m: &'a Message) -> IterGet<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init(get_message_ptr(m), &mut i) };
        IterGet(i, m)
    }

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

    fn recurse(&mut self, arg_type: i32) -> Option<IterGet<'a>> {
        let mut subiter = ffi_iter();
        unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut self.0) != arg_type { return None };
            ffi::dbus_message_iter_recurse(&mut self.0, &mut subiter)
        }
        Some(IterGet(subiter, self.1))
    }
}


#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::super::{Connection, ConnectionItem, Message, BusType};
    use super::{Array, Variant, Dict, IterGet};

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
        let mut z = HashMap::new();
        z.insert(123543u32, true);
        z.insert(0u32, false);
        let m = m.append1(Dict::new(&z));
        let sending = format!("{:?}", m.get_items());
        println!("Sending {}", sending);
        c.send(m).unwrap();

        for n in c.iter(1000) {
            match n {
                ConnectionItem::MethodCall(m) => {
                    use super::Arg;
                    let receiving = format!("{:?}", m.get_items());
                    println!("Receiving {}", receiving);
                    assert_eq!(sending, receiving);

                    assert_eq!(2000u16, m.get1().unwrap());
                    assert_eq!(m.get2(), (Some(2000u16), Some(&[129u8, 5, 254][..])));

                    let mut g = m.iter_init();
                    assert!(g.next() && g.next());
                    let v: Variant<IterGet> = g.get().unwrap();
                    let mut viter = v.0;
                    assert_eq!(viter.arg_type(), Array::<&str,()>::arg_type());
                    let a: Array<&str, _> = viter.get().unwrap();
                    assert_eq!(a.collect::<Vec<&str>>(), vec!["Hello", "world"]);
                    break;
                }
                _ => println!("Got {:?}", n),
            }
        }
    }
}
