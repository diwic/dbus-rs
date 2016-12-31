use super::*;
use {Signature, Path, Message, ffi};
use std::marker::PhantomData;
use std::{ptr, mem, any, fmt};
use super::check;
use std::ffi::{CString};
use std::os::raw::{c_void, c_int};

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

fn array_append<T: Arg, F: FnMut(&T, &mut IterAppend)>(z: &[T], i: &mut IterAppend, mut f: F) {
    let zptr = z.as_ptr();
    let zlen = z.len() as i32;

    // Can we do append_fixed_array?
    let a = (T::arg_type(), mem::size_of::<T>());
    let can_fixed_array = (zlen > 1) && (z.len() == zlen as usize) && FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == a);

    i.append_container(ArgType::Array, Some(T::signature().as_cstr()), |s|
        if can_fixed_array { unsafe { check("dbus_message_iter_append_fixed_array",
            ffi::dbus_message_iter_append_fixed_array(&mut s.0, a.0 as c_int, &zptr as *const _ as *const c_void, zlen)) }}
        else { for arg in z { f(arg, s); }}
    );
}

/// Appends a D-Bus array. Note: In case you have a large array of a type that implements FixedArray,
/// using this method will be more efficient than using an Array.
impl<'a, T: Arg + Append + Clone> Append for &'a [T] {
    fn append(self, i: &mut IterAppend) {
        array_append(self, i, |arg, s| arg.clone().append(s));
    }
}

impl<'a, T: Arg + RefArg> RefArg for &'a [T] {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        array_append(self, i, |arg, s| (arg as &RefArg).append(s));
    }
    fn as_any(&self) -> &any::Any where Self: 'static { self }
}

impl<T: Arg + RefArg> RefArg for Vec<T> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        array_append(&self, i, |arg, s| (arg as &RefArg).append(s));
    }
    fn as_any(&self) -> &any::Any where Self: 'static { self }
}


impl<'a, T: FixedArray> Get<'a> for &'a [T] {
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

#[derive(Copy, Clone, Debug)]
/// Represents a D-Bus Array. Maximum flexibility (wraps an iterator of items to append). 
/// Note: Slices of FixedArray can be faster.
pub struct Array<'a, T, I>(I, PhantomData<(*const T, &'a Message)>);

impl<'a, T: 'a, I: Iterator<Item=T>> Array<'a, T, I> {
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
        i.append_container(ArgType::Array, Some(T::signature().as_cstr()), |s| for arg in z { arg.append(s) });
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

// Due to the strong typing here; RefArg is implemented only for T's that are both Arg and RefArg.
// We need Arg for this to work for empty arrays (we can't get signature from first element if there is no elements).
// We need RefArg for non-consuming append.
impl<'a, T: 'a + Arg + fmt::Debug + RefArg, I: fmt::Debug + Clone + Iterator<Item=&'a T>> RefArg for Array<'static, T, I> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        let z = self.0.clone();
        i.append_container(ArgType::Array, Some(<T as Arg>::signature().as_cstr()), |s|
            for arg in z { (arg as &RefArg).append(s) }
        );
    }
    fn as_any(&self) -> &any::Any where Self: 'static { self }
}

fn get_fixed_array_refarg<'a, T: FixedArray + Clone + RefArg>(i: &mut Iter<'a>) -> Box<RefArg> {
    let s = <&[T]>::get(i).unwrap();
    Box::new(s.to_vec())
}

fn get_var_array_refarg<'a, T: 'static + RefArg + Arg, F: FnMut(&mut Iter<'a>) -> Option<T>>
    (i: &mut Iter<'a>, mut f: F) -> Box<RefArg> {
    let mut v: Vec<T> = vec!(); // dbus_message_iter_get_element_count might be O(n), better not use it
    let mut si = i.recurse(ArgType::Array).unwrap();
    while let Some(q) = f(&mut si) { v.push(q); si.next(); }
    Box::new(v)
}

pub fn get_array_refarg<'a>(i: &mut Iter<'a>) -> Box<RefArg> {
    debug_assert!(i.arg_type() == ArgType::Array);
    let etype = ArgType::from_i32(unsafe { ffi::dbus_message_iter_get_element_type(&mut i.0) } as i32).unwrap();
    match etype {
        ArgType::Byte => get_fixed_array_refarg::<u8>(i),
        ArgType::Int16 => get_fixed_array_refarg::<i16>(i),
        ArgType::UInt16 => get_fixed_array_refarg::<u16>(i),
        ArgType::Int32 => get_fixed_array_refarg::<i32>(i),
        ArgType::UInt32 => get_fixed_array_refarg::<u32>(i),
        ArgType::Int64 => get_fixed_array_refarg::<i64>(i),
        ArgType::UInt64 => get_fixed_array_refarg::<u64>(i),
        ArgType::Double => get_fixed_array_refarg::<f64>(i),
	ArgType::String => get_var_array_refarg::<String, _>(i, |si| si.get()),
	ArgType::ObjectPath => get_var_array_refarg::<Path<'static>, _>(i, |si| si.get::<Path>().map(|s| s.into_static())),
	ArgType::Signature => get_var_array_refarg::<Signature<'static>, _>(i, |si| si.get::<Signature>().map(|s| s.into_static())),
	ArgType::Variant => get_var_array_refarg::<Variant<Box<RefArg>>, _>(i, |si| Variant::new_refarg(si)),
	ArgType::Boolean => get_var_array_refarg::<bool, _>(i, |si| si.get()),
	ArgType::Invalid => panic!("Array with Invalid ArgType"),
        _ => unimplemented!(),
    }
}


