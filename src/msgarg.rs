#![allow(dead_code)]

use super::{ffi, libc, Message};
use super::message::get_message_ptr;
use std::{mem, ptr};
use std::marker::PhantomData;

use std::borrow::Cow;
use std::ffi::{CStr, CString};

// FIXME: Make strings::Signature a Cow instead
#[derive(Clone, Debug)]
pub struct Signature<'a>(Cow<'a, CStr>);

impl<'a> Signature<'a> {
    fn borrowed(a: &'a [u8]) -> Signature<'a> {
        assert_eq!(a[a.len()-1], 0);
        let c = unsafe { CStr::from_ptr(a.as_ptr() as *const i8)};
        Signature(Cow::Borrowed(c))
    }
    fn owned(a: String) -> Signature<'static> {
        let c = CString::new(a).unwrap();
        Signature(Cow::Owned(c))
    }
}

fn check(f: &str, i: u32) { if i == 0 { panic!("D-Bus error: '{}' failed", f) }} 

fn ffi_iter() -> ffi::DBusMessageIter { unsafe { mem::zeroed() }} 

fn arg_append_basic(i: *mut ffi::DBusMessageIter, arg_type: i32, v: i64) {
    let p = &v as *const _ as *const libc::c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type, p));
    };
}

fn arg_get_basic(i: *mut ffi::DBusMessageIter, arg_type: i32) -> Option<i64> {
    let mut c: i64 = 0;
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

/// Types that can be appended to a message as argument implement this helper trait.
pub trait Append: Clone {
    fn arg_type() -> i32;
    fn signature() -> Signature<'static>;
    fn append(self, &mut IterAppend);
}

macro_rules! integer_append {
    ($t: ident, $s: ident, $f: expr) => {

impl Append for $t {
    fn arg_type() -> i32 { ffi::$s }
    fn signature() -> Signature<'static> { Signature::borrowed($f) }
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, Self::arg_type(), self as i64) }
}
impl DictKey for $t {}

}}

integer_append!(u8, DBUS_TYPE_BYTE, b"y\0");
integer_append!(i16, DBUS_TYPE_INT16, b"n\0");
integer_append!(u16, DBUS_TYPE_UINT16, b"q\0");
integer_append!(i32, DBUS_TYPE_INT32, b"i\0");
integer_append!(u32, DBUS_TYPE_UINT32, b"u\0");
integer_append!(i64, DBUS_TYPE_INT64, b"x\0");
integer_append!(u64, DBUS_TYPE_UINT64, b"t\0");


impl Append for bool {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_BOOLEAN }
    fn signature() -> Signature<'static> { Signature::borrowed(b"b\0") }
    fn append(self, i: &mut IterAppend) { arg_append_basic(&mut i.0, Self::arg_type(), if self {1} else {0}) }
}
impl DictKey for bool {}

impl Append for f64 {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_DOUBLE }
    fn signature() -> Signature<'static> { Signature::borrowed(b"d\0") }
    fn append(self, i: &mut IterAppend) { arg_append_f64(&mut i.0, Self::arg_type(), self) }
}
impl DictKey for f64 {}

/// Represents a D-Bus string.
/// # Panic
/// Will panic in case the str contains \0 characters. 
impl<'a> Append for &'a str {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRING }
    fn signature() -> Signature<'static> { Signature::borrowed(b"s\0") }
    fn append(self, i: &mut IterAppend) {
        let z = CString::new(self).unwrap(); // FIXME: Do not unwrap here
        arg_append_str(&mut i.0, Self::arg_type(), &z)
    }
}
impl<'a> DictKey for &'a str {}


impl<'a, T: Append> Append for &'a T {
    fn arg_type() -> i32 { T::arg_type() }
    fn signature() -> Signature<'static> { T::signature() }
    fn append(self, i: &mut IterAppend) { self.clone().append(i) }
}
 
impl<'a, T: Append> Append for &'a [T] {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> { Signature::owned(format!("a{}", T::signature().0.to_str().unwrap())) }
    fn append(self, i: &mut IterAppend) {
        let z = self;
        i.append_container(Self::arg_type(), Some(&T::signature()), |s| for arg in z { arg.clone().append(s) });
    }
}

/// Types that can be used as keys in a dict type implement this trait. 
pub trait DictKey: Append {}

#[derive(Copy, Clone)]
pub struct Dict<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>>(I, PhantomData<&'a ()>);

impl<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>> Dict<'a, K, V, I> {
    fn entry_sig() -> String { format!("{{{}{}}}", K::signature().0.to_str().unwrap(), V::signature().0.to_str().unwrap()) } 
    pub fn new<J: IntoIterator<IntoIter=I, Item=(&'a K, &'a V)>>(j: J) -> Dict<'a, K, V, I> { Dict(j.into_iter(), PhantomData) }
}

impl<'a, K: 'a + DictKey, V: 'a + Append, I: Clone + Iterator<Item=(&'a K, &'a V)>> Append for Dict<'a, K, V, I> {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> {
        Signature::owned(format!("a{}", Self::entry_sig())) }
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&Signature::owned(Self::entry_sig())), |s| for (k, v) in z {
            s.append_container(ffi::DBUS_TYPE_DICT_ENTRY, None, |ss| {
                k.clone().append(ss);
                v.clone().append(ss);
            })
        });
    }
}

#[derive(Copy, Clone, Debug, Hash)]
pub struct Variant<T>(pub T);

impl<T: Append> Append for Variant<T> {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_VARIANT }
    fn signature() -> Signature<'static> { Signature::borrowed(b"v\0") }
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&T::signature()), |s| z.append(s));
    }
}

#[derive(Copy, Clone)]
pub struct Array<'a, T: 'a + Append, I: Iterator<Item=&'a T>>(I, PhantomData<&'a ()>);

impl<'a, T: 'a + Append, I: Iterator<Item=&'a T>> Array<'a, T, I> {
    pub fn new<J: IntoIterator<IntoIter=I, Item=&'a T>>(j: J) -> Array<'a, T, I> { Array(j.into_iter(), PhantomData) }
}

impl<'a, T: 'a + Append, I: Clone + Iterator<Item=&'a T>> Append for Array<'a, T, I> {
    fn append(self, i: &mut IterAppend) {
        let z = self.0;
        i.append_container(Self::arg_type(), Some(&T::signature()), |s| for arg in z { arg.clone().append(s) });
    }
    fn arg_type() -> i32 { ffi::DBUS_TYPE_ARRAY }
    fn signature() -> Signature<'static> { Signature::owned(format!("a{}", T::signature().0.to_str().unwrap())) }
}

impl<A: Append, B: Append> Append for (A, B) {
    fn arg_type() -> i32 { ffi::DBUS_TYPE_STRUCT }
    fn signature() -> Signature<'static> {
        Signature::owned(format!("({}{})", A::signature().0.to_str().unwrap(), B::signature().0.to_str().unwrap())) }
    fn append(self, i: &mut IterAppend) {
        let (a, b) = self;
        i.append_container(Self::arg_type(), None, |s| { a.append(s); b.append(s) });
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
pub struct IterAppend<'a>(ffi::DBusMessageIter, &'a Message);

impl<'a> IterAppend<'a> {
    pub fn new(m: &'a Message) -> IterAppend<'a> { 
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init_append(get_message_ptr(m), &mut i) };
        IterAppend(i, m)
    }

    pub fn append<T: Append>(&mut self, a: T) { a.append(self) }

    fn append_container<F: FnOnce(&mut IterAppend<'a>)>(&mut self, arg_type: i32, sig: Option<&Signature>, f: F) {
        let mut s = IterAppend(ffi_iter(), self.1);
        let p = sig.map(|s| s.0.as_ptr()).unwrap_or(ptr::null());
        check("dbus_message_iter_open_container",
            unsafe { ffi::dbus_message_iter_open_container(&mut self.0, arg_type, p, &mut s.0) });
        f(&mut s);
        check("dbus_message_iter_close_container",
            unsafe { ffi::dbus_message_iter_close_container(&mut self.0, &mut s.0) });
    }
}


#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::super::{Connection, ConnectionItem, Message, BusType};
    use super::{Array, Variant, Dict};

    use std::collections::HashMap;

    #[test]
    fn message_types() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        let m = m.append1(2000u16);
        let m = m.append1(Array::new(&vec![129u8, 5, 254]));
        let m = m.append1(Variant(&["Hello", "world"][..]));
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
                    let receiving = format!("{:?}", m.get_items());
                    println!("Receiving {}", receiving);
                    assert_eq!(sending, receiving);
                    break;
                }
                _ => println!("Got {:?}", n),
            }
        }
    }
}
