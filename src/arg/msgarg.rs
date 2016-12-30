#![allow(dead_code)]

use {ffi, Message, message, Signature};
use std::{mem, ptr};
use std::marker::PhantomData;

use super::{Iter, IterAppend, check, ArgType};

use std::ffi::CString;
use std::os::raw::{c_void, c_int};

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

/// Object safe version of Arg + Append + Get.
pub trait RefArg {
    /// The corresponding D-Bus argument type code. 
    fn arg_type(&self) -> ArgType;
    /// The corresponding D-Bus type signature for this type. 
    fn signature(&self) -> Signature<'static>;
    /// Performs the append operation.
    fn append(&self, &mut IterAppend);
    /// Performs the get operation.
    ///
    /// If successful, replaces self and returns Ok, otherwise self remains unchanged and Err is returned.
    fn get<'a>(&mut self, i: &mut Iter<'a>) -> Result<(), ()>;   
}

/// If a type implements this trait, it means the size and alignment is the same
/// as in D-Bus. This means that you can quickly append and get slices of this type.
///
/// Note: Booleans do not implement this trait because D-Bus booleans are 4 bytes and Rust booleans are 1 byte.
pub unsafe trait FixedArray: Arg {}

/// Types that can be used as keys in a dict type implement this trait. 
pub trait DictKey: Arg {}



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

impl Append for Variant<message::MessageItem> {
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

impl<$($t: Append),*> Append for ($($t,)*) {
    fn append(self, i: &mut IterAppend) {
        let ( $($n,)*) = self;
        i.append_container(ArgType::Struct, None, |s| { $( $n.append(s); )* });
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

impl Append for message::MessageItem {
    fn append(self, i: &mut IterAppend) {
        message::append_messageitem(&mut i.0, &self)
    }
}

impl<'a> Get<'a> for message::MessageItem {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        message::get_messageitem(&mut i.0)
    }
}


fn test_compile() {
    let mut q = IterAppend::new(unsafe { mem::transmute(0usize) });

    q.append(5u8);
    q.append(Array::new(&[5u8, 6, 7]));
    q.append((8u8, &[9u8, 6, 7][..]));
    q.append(Variant((6u8, 7u8)));
}


#[cfg(test)]
mod test {
    extern crate tempdir;

    use {Connection, ConnectionItem, Message, BusType, Path, Signature};
    use arg::{Array, Variant, Dict, Iter, ArgType, TypeMismatchError};

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
