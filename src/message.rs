use std::borrow::Cow;
use std::{fmt, mem, ptr};
use super::{ffi, Error, MessageType, TypeSig, libc, to_c_str, c_str_to_slice, init_dbus};
use super::{BusName, Path, Interface, Member, ErrorName, Connection};
use std::os::unix::io::{RawFd, AsRawFd};
use std::ffi::CStr;
use std::os::raw::{c_void, c_char, c_int};

use super::arg::{Append, IterAppend, Get, Iter, Arg, RefArg, TypeMismatchError};

#[derive(Debug,Copy,Clone)]
/// Errors that can happen when creating a MessageItem::Array.
pub enum ArrayError {
    /// The array is empty.
    EmptyArray,
    /// The array is composed of different element types.
    DifferentElementTypes,
}

fn new_dbus_message_iter() -> ffi::DBusMessageIter { unsafe { mem::zeroed() }}


/// An RAII wrapper around Fd to ensure that file descriptor is closed
/// when the scope ends.
#[derive(Debug, PartialEq, PartialOrd)]
pub struct OwnedFd {
    fd: RawFd
}

impl OwnedFd {
    /// Create a new OwnedFd from a RawFd.
    pub fn new(fd: RawFd) -> OwnedFd {
        OwnedFd { fd: fd }
    }

    /// Convert an OwnedFD back into a RawFd.
    pub fn into_fd(self) -> RawFd {
        let s = self.fd;
        ::std::mem::forget(self);
        s
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}

impl Clone for OwnedFd {
    fn clone(&self) -> OwnedFd {
        OwnedFd::new(unsafe { libc::dup(self.fd) } ) // FIXME: handle errors
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

/// MessageItem - used as parameters and return values from
/// method calls, or as data added to a signal (old, enum version).
///
/// Note that the newer generic design (see `arg` module) is both faster
/// and less error prone than MessageItem, and should be your first hand choice
/// whenever applicable.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum MessageItem {
    /// A D-Bus array requires all elements to be of the same type.
    /// All elements must match the TypeSig.
    Array(Vec<MessageItem>, TypeSig<'static>),
    /// A D-Bus struct allows for values of different types.
    Struct(Vec<MessageItem>),
    /// A D-Bus variant is a wrapper around another `MessageItem`, which
    /// can be of any type.
    Variant(Box<MessageItem>),
    /// A D-Bus dictionary is an Array of DictEntry items.
    DictEntry(Box<MessageItem>, Box<MessageItem>),
    /// A D-Bus objectpath requires its content to be a valid objectpath,
    /// so this cannot be any string.
    ObjectPath(Path<'static>),
    /// A D-Bus String is zero terminated, so no \0 s in the String, please.
    /// (D-Bus strings are also - like Rust strings - required to be valid UTF-8.)
    Str(String),
    /// A D-Bus boolean type.
    Bool(bool),
    /// A D-Bus unsigned 8 bit type.
    Byte(u8),
    /// A D-Bus signed 16 bit type.
    Int16(i16),
    /// A D-Bus signed 32 bit type.
    Int32(i32),
    /// A D-Bus signed 64 bit type.
    Int64(i64),
    /// A D-Bus unsigned 16 bit type.
    UInt16(u16),
    /// A D-Bus unsigned 32 bit type.
    UInt32(u32),
    /// A D-Bus unsigned 64 bit type.
    UInt64(u64),
    /// A D-Bus IEEE-754 double-precision floating point type.
    Double(f64),
    /// D-Bus allows for sending file descriptors, which can be used to
    /// set up SHM, unix pipes, or other communication channels.
    UnixFd(OwnedFd),
}

fn iter_get_basic(i: &mut ffi::DBusMessageIter) -> i64 {
    let mut c: i64 = 0;
    unsafe {
        let p: *mut c_void = mem::transmute(&mut c);
        ffi::dbus_message_iter_get_basic(i, p);
    }
    c
}

fn iter_get_f64(i: &mut ffi::DBusMessageIter) -> f64 {
    let mut c: f64 = 0.0;
    unsafe {
        let p: *mut c_void = mem::transmute(&mut c);
        ffi::dbus_message_iter_get_basic(i, p);
    }
    c
}

fn iter_append_f64(i: &mut ffi::DBusMessageIter, v: f64) {
    unsafe {
        let p: *const c_void = mem::transmute(&v);
        ffi::dbus_message_iter_append_basic(i, ffi::DBUS_TYPE_DOUBLE, p);
    }
}

fn iter_append_array(i: &mut ffi::DBusMessageIter, a: &[MessageItem], t: TypeSig<'static>) {
    let mut subiter = new_dbus_message_iter();
    let atype = to_c_str(&t);

    assert!(unsafe { ffi::dbus_message_iter_open_container(i, ffi::DBUS_TYPE_ARRAY, atype.as_ptr(), &mut subiter) } != 0);
    for item in a.iter() {
//        assert!(item.type_sig() == t);
        item.iter_append(&mut subiter);
    }
    assert!(unsafe { ffi::dbus_message_iter_close_container(i, &mut subiter) } != 0);
}

fn iter_append_struct(i: &mut ffi::DBusMessageIter, a: &[MessageItem]) {
    let mut subiter = new_dbus_message_iter();
    let res = unsafe { ffi::dbus_message_iter_open_container(i, ffi::DBUS_TYPE_STRUCT, ptr::null(), &mut subiter) };
    assert!(res != 0);
    for item in a.iter() {
        item.iter_append(&mut subiter);
    }
    let res2 = unsafe { ffi::dbus_message_iter_close_container(i, &mut subiter) };
    assert!(res2 != 0);
}

fn iter_append_variant(i: &mut ffi::DBusMessageIter, a: &MessageItem) {
    let mut subiter = new_dbus_message_iter();
    let atype = to_c_str(&a.type_sig());
    assert!(unsafe { ffi::dbus_message_iter_open_container(i, ffi::DBUS_TYPE_VARIANT, atype.as_ptr(), &mut subiter) } != 0);
    a.iter_append(&mut subiter);
    assert!(unsafe { ffi::dbus_message_iter_close_container(i, &mut subiter) } != 0);
}

fn iter_append_dict(i: &mut ffi::DBusMessageIter, k: &MessageItem, v: &MessageItem) {
    let mut subiter = new_dbus_message_iter();
    assert!(unsafe { ffi::dbus_message_iter_open_container(i, ffi::DBUS_TYPE_DICT_ENTRY, ptr::null(), &mut subiter) } != 0);
    k.iter_append(&mut subiter);
    v.iter_append(&mut subiter);
    assert!(unsafe { ffi::dbus_message_iter_close_container(i, &mut subiter) } != 0);
}

impl MessageItem {
    /// Get the D-Bus ASCII type-code for this MessageItem.
    pub fn type_sig(&self) -> TypeSig<'static> {
        match self {
            // TODO: Can we make use of the ffi constants here instead of duplicating them?
            &MessageItem::Str(_) => Cow::Borrowed("s"),
            &MessageItem::Bool(_) => Cow::Borrowed("b"),
            &MessageItem::Byte(_) => Cow::Borrowed("y"),
            &MessageItem::Int16(_) => Cow::Borrowed("n"),
            &MessageItem::Int32(_) => Cow::Borrowed("i"),
            &MessageItem::Int64(_) => Cow::Borrowed("x"),
            &MessageItem::UInt16(_) => Cow::Borrowed("q"),
            &MessageItem::UInt32(_) => Cow::Borrowed("u"),
            &MessageItem::UInt64(_) => Cow::Borrowed("t"),
            &MessageItem::Double(_) => Cow::Borrowed("d"),
            &MessageItem::Array(_, ref s) => Cow::Owned(format!("a{}", s)),
            &MessageItem::Struct(ref s) => Cow::Owned(format!("({})", s.iter().fold(String::new(), |s, i| s + &*i.type_sig()))),
            &MessageItem::Variant(_) => Cow::Borrowed("v"),
            &MessageItem::DictEntry(ref k, ref v) => Cow::Owned(format!("{{{}{}}}", k.type_sig(), v.type_sig())),
            &MessageItem::ObjectPath(_) => Cow::Borrowed("o"),
            &MessageItem::UnixFd(_) => Cow::Borrowed("h"),
        }
    }

    /// Get the integer value for this MessageItem's type-code.
    pub fn array_type(&self) -> i32 {
        let s = match self {
            &MessageItem::Str(_) => ffi::DBUS_TYPE_STRING,
            &MessageItem::Bool(_) => ffi::DBUS_TYPE_BOOLEAN,
            &MessageItem::Byte(_) => ffi::DBUS_TYPE_BYTE,
            &MessageItem::Int16(_) => ffi::DBUS_TYPE_INT16,
            &MessageItem::Int32(_) => ffi::DBUS_TYPE_INT32,
            &MessageItem::Int64(_) => ffi::DBUS_TYPE_INT64,
            &MessageItem::UInt16(_) => ffi::DBUS_TYPE_UINT16,
            &MessageItem::UInt32(_) => ffi::DBUS_TYPE_UINT32,
            &MessageItem::UInt64(_) => ffi::DBUS_TYPE_UINT64,
            &MessageItem::Double(_) => ffi::DBUS_TYPE_DOUBLE,
            &MessageItem::Array(_,_) => ffi::DBUS_TYPE_ARRAY,
            &MessageItem::Struct(_) => ffi::DBUS_TYPE_STRUCT,
            &MessageItem::Variant(_) => ffi::DBUS_TYPE_VARIANT,
            &MessageItem::DictEntry(_,_) => ffi::DBUS_TYPE_DICT_ENTRY,
            &MessageItem::ObjectPath(_) => ffi::DBUS_TYPE_OBJECT_PATH,
            &MessageItem::UnixFd(_) => ffi::DBUS_TYPE_UNIX_FD,
        };
        s as i32
    }

    /// Creates a (String, Variant) dictionary from an iterator with Result passthrough (an Err will abort and return that Err)
    pub fn from_dict<E, I: Iterator<Item=Result<(String, MessageItem),E>>>(i: I) -> Result<MessageItem,E> {
        let mut v = Vec::new();
        for r in i {
            let (s, vv) = try!(r);
            v.push((s.into(), Box::new(vv).into()).into());
        }
        Ok(MessageItem::Array(v, Cow::Borrowed("{sv}")))
    }

    /// Creates an MessageItem::Array from a list of MessageItems.
    ///
    /// Note: This requires `v` to be non-empty. See also
    /// `MessageItem::from(&[T])`, which can handle empty arrays as well.
    pub fn new_array(v: Vec<MessageItem>) -> Result<MessageItem,ArrayError> {
        if v.len() == 0 {
            return Err(ArrayError::EmptyArray);
        }

        let t = v[0].type_sig();
        for i in &v {
            if i.type_sig() != t {
                return Err(ArrayError::DifferentElementTypes);
            }
        }

        Ok(MessageItem::Array(v, t))
    }


    fn new_array2<D, I>(i: I) -> MessageItem
    where D: Into<MessageItem>, D: Default, I: Iterator<Item=D> {
        let v: Vec<MessageItem> = i.map(|ii| ii.into()).collect();
        let t = if v.len() == 0 { D::default().into().type_sig() } else { v[0].type_sig() };
        MessageItem::Array(v, t)
    }

    fn new_array3<'b, D: 'b, I>(i: I) -> MessageItem
    where D: Into<MessageItem> + Default + Clone, I: Iterator<Item=&'b D> {
        MessageItem::new_array2(i.map(|ii| ii.clone()))
    }

    fn from_iter_single(i: &mut ffi::DBusMessageIter) -> Option<MessageItem> {
        let t = unsafe { ffi::dbus_message_iter_get_arg_type(i) };
        match t {
            ffi::DBUS_TYPE_INVALID => { None },
            ffi::DBUS_TYPE_DICT_ENTRY => {
                let mut subiter = new_dbus_message_iter();
                unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                let a = MessageItem::from_iter(&mut subiter);
                if a.len() != 2 { panic!("D-Bus dict entry error"); }
                let mut a = a.into_iter();
                let key = Box::new(a.next().unwrap());
                let value = Box::new(a.next().unwrap());
                Some(MessageItem::DictEntry(key, value))
            }
            ffi::DBUS_TYPE_VARIANT => {
                let mut subiter = new_dbus_message_iter();
                unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                let a = MessageItem::from_iter(&mut subiter);
                if a.len() != 1 { panic!("D-Bus variant error"); }
                Some(MessageItem::Variant(Box::new(a.into_iter().next().unwrap())))
            }
            ffi::DBUS_TYPE_ARRAY => {
                let mut subiter = new_dbus_message_iter();
                unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                let a = MessageItem::from_iter(&mut subiter);
                let t = if a.len() > 0 { a[0].type_sig() } else {
                    let c = unsafe { ffi::dbus_message_iter_get_signature(&mut subiter) };
                    let s = c_str_to_slice(&(c as *const c_char)).unwrap().to_string();
                    unsafe { ffi::dbus_free(c as *mut c_void) };
                    Cow::Owned(s)
                };
                Some(MessageItem::Array(a, t))
            },
            ffi::DBUS_TYPE_STRUCT => {
                let mut subiter = new_dbus_message_iter();
                unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                Some(MessageItem::Struct(MessageItem::from_iter(&mut subiter)))
            },
            ffi::DBUS_TYPE_STRING => {
                let mut c: *const c_char = ptr::null();
                unsafe {
                    let p: *mut c_void = mem::transmute(&mut c);
                    ffi::dbus_message_iter_get_basic(i, p);
                };
                Some(MessageItem::Str(c_str_to_slice(&c).expect("D-Bus string error").to_string()))
            },
            ffi::DBUS_TYPE_OBJECT_PATH => {
                let mut c: *const c_char = ptr::null();
                unsafe {
                    let p: *mut c_void = mem::transmute(&mut c);
                    ffi::dbus_message_iter_get_basic(i, p);
                };
                let o = Path::new(c_str_to_slice(&c).expect("D-Bus object path error")).ok().expect("D-Bus object path error");
                Some(MessageItem::ObjectPath(o))
            },
            ffi::DBUS_TYPE_UNIX_FD => Some(MessageItem::UnixFd(OwnedFd::new(iter_get_basic(i) as libc::c_int))),
            ffi::DBUS_TYPE_BOOLEAN => Some(MessageItem::Bool((iter_get_basic(i) as u32) != 0)),
            ffi::DBUS_TYPE_BYTE => Some(MessageItem::Byte(iter_get_basic(i) as u8)),
            ffi::DBUS_TYPE_INT16 => Some(MessageItem::Int16(iter_get_basic(i) as i16)),
            ffi::DBUS_TYPE_INT32 => Some(MessageItem::Int32(iter_get_basic(i) as i32)),
            ffi::DBUS_TYPE_INT64 => Some(MessageItem::Int64(iter_get_basic(i) as i64)),
            ffi::DBUS_TYPE_UINT16 => Some(MessageItem::UInt16(iter_get_basic(i) as u16)),
            ffi::DBUS_TYPE_UINT32 => Some(MessageItem::UInt32(iter_get_basic(i) as u32)),
            ffi::DBUS_TYPE_UINT64 => Some(MessageItem::UInt64(iter_get_basic(i) as u64)),
            ffi::DBUS_TYPE_DOUBLE => Some(MessageItem::Double(iter_get_f64(i))),
            _ => { None /* Only the new msgarg module supports signatures */ }
        }
    }

    fn from_iter(i: &mut ffi::DBusMessageIter) -> Vec<MessageItem> {
        let mut v = Vec::new();
        while let Some(m) = Self::from_iter_single(i) {
            v.push(m);
            unsafe { ffi::dbus_message_iter_next(i) };
        }
        v
    }

    fn iter_append_basic(&self, i: &mut ffi::DBusMessageIter, v: i64) {
        let t = self.array_type();
        unsafe {
            let p: *const c_void = mem::transmute(&v);
            ffi::dbus_message_iter_append_basic(i, t as c_int, p);
        }
    }

    fn iter_append(&self, i: &mut ffi::DBusMessageIter) {
        match self {
            &MessageItem::Str(ref s) => unsafe {
                let c = to_c_str(s);
                let p = mem::transmute(&c);
                ffi::dbus_message_iter_append_basic(i, ffi::DBUS_TYPE_STRING, p);
            },
            &MessageItem::Bool(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::Byte(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::Int16(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::Int32(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::Int64(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::UInt16(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::UInt32(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::UInt64(b) => self.iter_append_basic(i, b as i64),
            &MessageItem::UnixFd(ref b) => self.iter_append_basic(i, b.as_raw_fd() as i64),
            &MessageItem::Double(b) => iter_append_f64(i, b),
            &MessageItem::Array(ref b, ref t) => iter_append_array(i, &**b, t.clone()),
            &MessageItem::Struct(ref v) => iter_append_struct(i, &**v),
            &MessageItem::Variant(ref b) => iter_append_variant(i, &**b),
            &MessageItem::DictEntry(ref k, ref v) => iter_append_dict(i, &**k, &**v),
            &MessageItem::ObjectPath(ref s) => unsafe {
                let c: *const libc::c_char = s.as_ref().as_ptr();
                let p = mem::transmute(&c);
                ffi::dbus_message_iter_append_basic(i, ffi::DBUS_TYPE_OBJECT_PATH, p);
            }
        }
    }

    fn copy_to_iter(i: &mut ffi::DBusMessageIter, v: &[MessageItem]) {
        for item in v.iter() {
            item.iter_append(i);
        }
    }

    /// Conveniently get the inner value of a `MessageItem`
    ///
    /// # Example
    /// ```
    /// use dbus::MessageItem;
    /// let m: MessageItem = 5i64.into();
    /// let s: i64 = m.inner().unwrap();
    /// assert_eq!(s, 5i64);
    /// ```
    pub fn inner<'a, T: FromMessageItem<'a>>(&'a self) -> Result<T, ()> {
        T::from(self)
    }
}


// For use by the msgarg module
pub fn append_messageitem(i: &mut ffi::DBusMessageIter, m: &MessageItem) {
    m.iter_append(i)
}

// For use by the msgarg module
pub fn get_messageitem(i: &mut ffi::DBusMessageIter) -> Option<MessageItem> {
    MessageItem::from_iter_single(i)
}


macro_rules! msgitem_convert {
    ($t: ty, $s: ident) => {
        impl From<$t> for MessageItem { fn from(i: $t) -> MessageItem { MessageItem::$s(i) } }

        impl<'a> FromMessageItem<'a> for $t {
            fn from(i: &'a MessageItem) -> Result<$t,()> {
                if let &MessageItem::$s(ref b) = i { Ok(*b) } else { Err(()) }
            }
        }
    }
}

msgitem_convert!(u8, Byte);
msgitem_convert!(u64, UInt64);
msgitem_convert!(u32, UInt32);
msgitem_convert!(u16, UInt16);
msgitem_convert!(i16, Int16);
msgitem_convert!(i32, Int32);
msgitem_convert!(i64, Int64);
msgitem_convert!(f64, Double);
msgitem_convert!(bool, Bool);


/// Create a `MessageItem::Array`.
impl<'a, T> From<&'a [T]> for MessageItem
where T: Into<MessageItem> + Clone + Default {
    fn from(i: &'a [T]) -> MessageItem {
        MessageItem::new_array3(i.iter())
    }
}

impl<'a> From<&'a str> for MessageItem { fn from(i: &str) -> MessageItem { MessageItem::Str(i.to_string()) } }

impl From<String> for MessageItem { fn from(i: String) -> MessageItem { MessageItem::Str(i) } }

impl From<Path<'static>> for MessageItem { fn from(i: Path<'static>) -> MessageItem { MessageItem::ObjectPath(i) } }

impl From<OwnedFd> for MessageItem { fn from(i: OwnedFd) -> MessageItem { MessageItem::UnixFd(i) } }

/// Create a `MessageItem::Variant`
impl From<Box<MessageItem>> for MessageItem {
    fn from(i: Box<MessageItem>) -> MessageItem { MessageItem::Variant(i) }
}

/// Create a `MessageItem::DictEntry`
impl From<(MessageItem, MessageItem)> for MessageItem {
    fn from(i: (MessageItem, MessageItem)) -> MessageItem {
        MessageItem::DictEntry(Box::new(i.0), Box::new(i.1))
    }
}

/// Helper trait for `MessageItem::inner()`
pub trait FromMessageItem<'a> :Sized {
    /// Allows converting from a MessageItem into the type it contains.
    fn from(i: &'a MessageItem) -> Result<Self, ()>;
}

impl<'a> FromMessageItem<'a> for &'a str {
    fn from(i: &'a MessageItem) -> Result<&'a str,()> {
        match i {
            &MessageItem::Str(ref b) => Ok(&b),
            &MessageItem::ObjectPath(ref b) => Ok(&b),
            _ => Err(()),
        }
    }
}

impl<'a> FromMessageItem<'a> for &'a String {
    fn from(i: &'a MessageItem) -> Result<&'a String,()> { if let &MessageItem::Str(ref b) = i { Ok(&b) } else { Err(()) } }
}

impl<'a> FromMessageItem<'a> for &'a Path<'static> {
    fn from(i: &'a MessageItem) -> Result<&'a Path<'static>,()> { if let &MessageItem::ObjectPath(ref b) = i { Ok(&b) } else { Err(()) } }
}

impl<'a> FromMessageItem<'a> for &'a MessageItem {
    fn from(i: &'a MessageItem) -> Result<&'a MessageItem,()> { if let &MessageItem::Variant(ref b) = i { Ok(&**b) } else { Err(()) } }
}

impl<'a> FromMessageItem<'a> for &'a Vec<MessageItem> {
    fn from(i: &'a MessageItem) -> Result<&'a Vec<MessageItem>,()> {
        match i {
            &MessageItem::Array(ref b, _) => Ok(&b),
            &MessageItem::Struct(ref b) => Ok(&b),
            _ => Err(()),
        }
    }
}

impl<'a> FromMessageItem<'a> for &'a [MessageItem] {
    fn from(i: &'a MessageItem) -> Result<&'a [MessageItem],()> { i.inner::<&Vec<MessageItem>>().map(|s| &**s) }
}

impl<'a> FromMessageItem<'a> for &'a OwnedFd {
    fn from(i: &'a MessageItem) -> Result<&'a OwnedFd,()> { if let &MessageItem::UnixFd(ref b) = i { Ok(b) } else { Err(()) } }
}

impl<'a> FromMessageItem<'a> for (&'a MessageItem, &'a MessageItem) {
    fn from(i: &'a MessageItem) -> Result<(&'a MessageItem, &'a MessageItem),()> {
        if let &MessageItem::DictEntry(ref k, ref v) = i { Ok((&**k, &**v)) } else { Err(()) }
    }
}


/// A D-Bus message. A message contains some headers (e g sender and destination address)
/// and a list of MessageItems.
pub struct Message {
    msg: *mut ffi::DBusMessage,
}

unsafe impl Send for Message {}

impl Message {
    /// Creates a new method call message.
    pub fn new_method_call<'d, 'p, 'i, 'm, D, P, I, M>(destination: D, path: P, iface: I, method: M) -> Result<Message, String>
    where D: Into<BusName<'d>>, P: Into<Path<'p>>, I: Into<Interface<'i>>, M: Into<Member<'m>> {
        init_dbus();
        let (d, p, i, m) = (destination.into(), path.into(), iface.into(), method.into());
        let ptr = unsafe {
            ffi::dbus_message_new_method_call(d.as_ref().as_ptr(), p.as_ref().as_ptr(), i.as_ref().as_ptr(), m.as_ref().as_ptr())
        };
        if ptr == ptr::null_mut() { Err("D-Bus error: dbus_message_new_method_call failed".into()) }
        else { Ok(Message { msg: ptr}) }
    }

    /// Creates a new method call message.
    pub fn method_call(destination: &BusName, path: &Path, iface: &Interface, name: &Member) -> Message {
        init_dbus();
        let ptr = unsafe {
            ffi::dbus_message_new_method_call(destination.as_ref().as_ptr(), path.as_ref().as_ptr(),
                iface.as_ref().as_ptr(), name.as_ref().as_ptr())
        };
        if ptr == ptr::null_mut() { panic!("D-Bus error: dbus_message_new_signal failed") }
        Message { msg: ptr}
    }

    /// Creates a new signal message.
    pub fn new_signal<P, I, M>(path: P, iface: I, name: M) -> Result<Message, String>
    where P: Into<Vec<u8>>, I: Into<Vec<u8>>, M: Into<Vec<u8>> {
        init_dbus();

        let p = try!(Path::new(path));
        let i = try!(Interface::new(iface));
        let m = try!(Member::new(name));

        let ptr = unsafe {
            ffi::dbus_message_new_signal(p.as_ref().as_ptr(), i.as_ref().as_ptr(), m.as_ref().as_ptr())
        };
        if ptr == ptr::null_mut() { Err("D-Bus error: dbus_message_new_signal failed".into()) }
        else { Ok(Message { msg: ptr}) }
    }

    /// Creates a new signal message.
    pub fn signal(path: &Path, iface: &Interface, name: &Member) -> Message {
        init_dbus();
        let ptr = unsafe {
            ffi::dbus_message_new_signal(path.as_ref().as_ptr(), iface.as_ref().as_ptr(), name.as_ref().as_ptr())
        };
        if ptr == ptr::null_mut() { panic!("D-Bus error: dbus_message_new_signal failed") }
        Message { msg: ptr}
    }

    /// Creates a method reply for this method call.
    pub fn new_method_return(m: &Message) -> Option<Message> {
        let ptr = unsafe { ffi::dbus_message_new_method_return(m.msg) };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    /// Creates a method return (reply) for this method call.
    pub fn method_return(&self) -> Message {
        let ptr = unsafe { ffi::dbus_message_new_method_return(self.msg) };
        if ptr == ptr::null_mut() { panic!("D-Bus error: dbus_message_new_method_return failed") }
        Message {msg: ptr}
    }

    /// The old way to create a new error reply
    pub fn new_error(m: &Message, error_name: &str, error_message: &str) -> Option<Message> {
        let (en, em) = (to_c_str(error_name), to_c_str(error_message));
        let ptr = unsafe { ffi::dbus_message_new_error(m.msg, en.as_ptr(), em.as_ptr()) };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    /// Creates a new error reply
    pub fn error(&self, error_name: &ErrorName, error_message: &CStr) -> Message {
        let ptr = unsafe { ffi::dbus_message_new_error(self.msg, error_name.as_ref().as_ptr(), error_message.as_ptr()) };
        if ptr == ptr::null_mut() { panic!("D-Bus error: dbus_message_new_error failed") }
        Message { msg: ptr}
    }

    /// Get the MessageItems that make up the message.
    ///
    /// Note: use `iter_init` or `get1`/`get2`/etc instead for faster access to the arguments.
    /// This method is provided for backwards compatibility.
    pub fn get_items(&self) -> Vec<MessageItem> {
        let mut i = new_dbus_message_iter();
        match unsafe { ffi::dbus_message_iter_init(self.msg, &mut i) } {
            0 => Vec::new(),
            _ => MessageItem::from_iter(&mut i)
        }
    }

    /// Get the D-Bus serial of a message, if one was specified.
    pub fn get_serial(&self) -> u32 {
        unsafe { ffi::dbus_message_get_serial(self.msg) }
    }

    /// Get the serial of the message this message is a reply to, if present.
    pub fn get_reply_serial(&self) -> Option<u32> {
        let s = unsafe { ffi::dbus_message_get_reply_serial(self.msg) };
        if s == 0 { None } else { Some(s) }
    }

    /// Add one or more MessageItems to this Message.
    ///
    /// Note: using `append1`, `append2` or `append3` might be faster, especially for large arrays.
    /// This method is provided for backwards compatibility.
    pub fn append_items(&mut self, v: &[MessageItem]) {
        let mut i = new_dbus_message_iter();
        unsafe { ffi::dbus_message_iter_init_append(self.msg, &mut i) };
        MessageItem::copy_to_iter(&mut i, v);
    }

    /// Appends one MessageItem to a message.
    /// Use in builder style: e g `m.method_return().append(7i32)`
    ///
    /// Note: using `append1`, `append2` or `append3` might be faster, especially for large arrays.
    /// This method is provided for backwards compatibility.
    pub fn append<I: Into<MessageItem>>(self, v: I) -> Self {
        let mut i = new_dbus_message_iter();
        unsafe { ffi::dbus_message_iter_init_append(self.msg, &mut i) };
        MessageItem::copy_to_iter(&mut i, &[v.into()]);
        self
    }

    /// Appends one argument to this message.
    /// Use in builder style: e g `m.method_return().append1(7i32)`
    pub fn append1<A: Append>(mut self, a: A) -> Self {
        {
            let mut m = IterAppend::new(&mut self);
            m.append(a);
        }
        self
    }

    /// Appends two arguments to this message.
    /// Use in builder style: e g `m.method_return().append2(7i32, 6u8)`
    pub fn append2<A1: Append, A2: Append>(mut self, a1: A1, a2: A2) -> Self {
        {
            let mut m = IterAppend::new(&mut self);
            m.append(a1); m.append(a2);
        }
        self
    }

    /// Appends three arguments to this message.
    /// Use in builder style: e g `m.method_return().append3(7i32, 6u8, true)`
    pub fn append3<A1: Append, A2: Append, A3: Append>(mut self, a1: A1, a2: A2, a3: A3) -> Self {
        {
            let mut m = IterAppend::new(&mut self);
            m.append(a1); m.append(a2); m.append(a3);
        }
        self
    }

    /// Appends RefArgs to this message.
    /// Use in builder style: e g `m.method_return().append_ref(&[7i32, 6u8, true])`
    pub fn append_ref<A: RefArg>(mut self, r: &[A]) -> Self {
        {
            let mut m = IterAppend::new(&mut self);
            for rr in r {
                rr.append(&mut m);
            }
        }
        self
    } 

    /// Gets the first argument from the message, if that argument is of type G1.
    /// Returns None if there are not enough arguments, or if types don't match.
    pub fn get1<'a, G1: Get<'a>>(&'a self) -> Option<G1> {
        let mut i = Iter::new(&self);
        i.get()
    }

    /// Gets the first two arguments from the message, if those arguments are of type G1 and G2.
    /// Returns None if there are not enough arguments, or if types don't match.
    pub fn get2<'a, G1: Get<'a>, G2: Get<'a>>(&'a self) -> (Option<G1>, Option<G2>) {
        let mut i = Iter::new(&self);
        let g1 = i.get();
        if !i.next() { return (g1, None); }
        (g1, i.get())
    }

    /// Gets the first three arguments from the message, if those arguments are of type G1, G2 and G3.
    /// Returns None if there are not enough arguments, or if types don't match.
    pub fn get3<'a, G1: Get<'a>, G2: Get<'a>, G3: Get<'a>>(&'a self) -> (Option<G1>, Option<G2>, Option<G3>) {
        let mut i = Iter::new(&self);
        let g1 = i.get();
        if !i.next() { return (g1, None, None) }
        let g2 = i.get();
        if !i.next() { return (g1, g2, None) }
        (g1, g2, i.get())
    }

    /// Gets the first four arguments from the message, if those arguments are of type G1, G2, G3 and G4.
    /// Returns None if there are not enough arguments, or if types don't match.
    pub fn get4<'a, G1: Get<'a>, G2: Get<'a>, G3: Get<'a>, G4: Get<'a>>(&'a self) -> (Option<G1>, Option<G2>, Option<G3>, Option<G4>) {
        let mut i = Iter::new(&self);
        let g1 = i.get();
        if !i.next() { return (g1, None, None, None) }
        let g2 = i.get();
        if !i.next() { return (g1, g2, None, None) }
        let g3 = i.get();
        if !i.next() { return (g1, g2, g3, None) }
        (g1, g2, g3, i.get())
    }

    /// Gets the first five arguments from the message, if those arguments are of type G1, G2, G3 and G4.
    /// Returns None if there are not enough arguments, or if types don't match.
    /// Note: If you need more than five arguments, use `iter_init` instead.
    pub fn get5<'a, G1: Get<'a>, G2: Get<'a>, G3: Get<'a>, G4: Get<'a>, G5: Get<'a>>(&'a self) -> (Option<G1>, Option<G2>, Option<G3>, Option<G4>, Option<G5>) {
        let mut i = Iter::new(&self);
        let g1 = i.get();
        if !i.next() { return (g1, None, None, None, None) }
        let g2 = i.get();
        if !i.next() { return (g1, g2, None, None, None) }
        let g3 = i.get();
        if !i.next() { return (g1, g2, g3, None, None) }
        let g4 = i.get();
        if !i.next() { return (g1, g2, g3, g4, None) }
        (g1, g2, g3, g4, i.get())
    }

    /// Gets the first argument from the message, if that argument is of type G1.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read1<'a, G1: Arg + Get<'a>>(&'a self) -> Result<G1, TypeMismatchError> {
        let mut i = Iter::new(&self);
        i.read()
    }

    /// Gets the first two arguments from the message, if those arguments are of type G1 and G2.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read2<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>>(&'a self) -> Result<(G1, G2), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((try!(i.read()), try!(i.read())))
    }

    /// Gets the first three arguments from the message, if those arguments are of type G1, G2 and G3.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read3<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>>(&'a self) -> 
        Result<(G1, G2, G3), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((try!(i.read()), try!(i.read()), try!(i.read())))
    }

    /// Gets the first four arguments from the message, if those arguments are of type G1, G2, G3 and G4.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read4<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>, G4: Arg + Get<'a>>(&'a self) ->
        Result<(G1, G2, G3, G4), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((try!(i.read()), try!(i.read()), try!(i.read()), try!(i.read())))
    }

    /// Gets the first five arguments from the message, if those arguments are of type G1, G2, G3, G4 and G5.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    /// Note: If you need more than five arguments, use `iter_init` instead.
    pub fn read5<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>, G4: Arg + Get<'a>, G5: Arg + Get<'a>>(&'a self) ->
        Result<(G1, G2, G3, G4, G5), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((try!(i.read()), try!(i.read()), try!(i.read()), try!(i.read()), try!(i.read())))
    }

    /// Returns a struct for retreiving the arguments from a message. Supersedes get_items().
    pub fn iter_init<'a>(&'a self) -> Iter<'a> { Iter::new(&self) }

    /// Gets the MessageType of the Message.
    pub fn msg_type(&self) -> MessageType {
        unsafe { mem::transmute(ffi::dbus_message_get_type(self.msg)) }
    }

    fn msg_internal_str<'a>(&'a self, c: *const libc::c_char) -> Option<&'a [u8]> {
        if c == ptr::null() { None }
        else { Some( unsafe { CStr::from_ptr(c) }.to_bytes_with_nul()) }
    }

    /// Gets the name of the connection that originated this message.
    pub fn sender<'a>(&'a self) -> Option<BusName<'a>> {
        self.msg_internal_str(unsafe { ffi::dbus_message_get_sender(self.msg) })
            .map(|s| unsafe { BusName::from_slice_unchecked(s) })
    }

    /// Returns a tuple of (Message type, Path, Interface, Member) of the current message.
    pub fn headers(&self) -> (MessageType, Option<String>, Option<String>, Option<String>) {
        let p = unsafe { ffi::dbus_message_get_path(self.msg) };
        let i = unsafe { ffi::dbus_message_get_interface(self.msg) };
        let m = unsafe { ffi::dbus_message_get_member(self.msg) };
        (self.msg_type(),
         c_str_to_slice(&p).map(|s| s.to_string()),
         c_str_to_slice(&i).map(|s| s.to_string()),
         c_str_to_slice(&m).map(|s| s.to_string()))
    }

    /// Gets the object path this Message is being sent to.
    pub fn path<'a>(&'a self) -> Option<Path<'a>> {
        self.msg_internal_str(unsafe { ffi::dbus_message_get_path(self.msg) })
            .map(|s| unsafe { Path::from_slice_unchecked(s) })
    }

    /// Gets the interface this Message is being sent to.
    pub fn interface<'a>(&'a self) -> Option<Interface<'a>> {
        self.msg_internal_str(unsafe { ffi::dbus_message_get_interface(self.msg) })
            .map(|s| unsafe { Interface::from_slice_unchecked(s) })
    }

    /// Gets the interface member being called.
    pub fn member<'a>(&'a self) -> Option<Member<'a>> {
        self.msg_internal_str(unsafe { ffi::dbus_message_get_member(self.msg) })
            .map(|s| unsafe { Member::from_slice_unchecked(s) })
    }

    /// When the remote end returns an error, the message itself is
    /// correct but its contents is an error. This method will
    /// transform such an error to a D-Bus Error or otherwise return
    /// the original message.
    pub fn as_result(&mut self) -> Result<&mut Message, Error> {
        let mut e = Error::empty();
        if unsafe { ffi::dbus_set_error_from_message(e.get_mut(), self.msg) } != 0 { Err(e) }
        else { Ok(self) }
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        unsafe {
            ffi::dbus_message_unref(self.msg);
        }
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self.headers())
    }
}

pub fn message_from_ptr(ptr: *mut ffi::DBusMessage, add_ref: bool) -> Message {
    if add_ref {
        unsafe { ffi::dbus_message_ref(ptr) };
    }
    Message { msg: ptr }
}

pub fn get_message_ptr<'a>(m: &Message) -> *mut ffi::DBusMessage {
    m.msg
}


/// A convenience struct that wraps connection, destination and path.
///
/// Useful if you want to make many method calls to the same destination path.
#[derive(Clone, Debug)]
pub struct ConnPath<'a, C> {
    /// Some way to access the connection, e g a &Connection or Rc<Connection>
    pub conn: C,
    /// Destination, i e what D-Bus service you're communicating with
    pub dest: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Timeout in milliseconds for blocking method calls
    pub timeout: i32,
}

impl<'a, C: ::std::ops::Deref<Target=Connection>> ConnPath<'a, C> {
   // More methods to come.

    /// Make a D-Bus method call, where you can append arguments inside the closure.
    pub fn method_call_with_args<F: FnOnce(&mut Message)>(&self, i: &Interface, m: &Member, f: F) -> Result<Message, Error> {
        let mut msg = Message::method_call(&self.dest, &self.path, i, m);
        f(&mut msg);
        self.conn.send_with_reply_and_block(msg, self.timeout)
    }

    /// Emit a D-Bus signal, where you can append arguments inside the closure.
    pub fn signal_with_args<F: FnOnce(&mut Message)>(&self, i: &Interface, m: &Member, f: F) -> Result<u32, Error> {
        let mut msg = Message::signal(&self.path, i, m);
        f(&mut msg);
        self.conn.send(msg).map_err(|_| Error::new_custom("org.freedesktop.DBus.Error.Failed", "Sending signal failed"))
    }
}

pub fn message_set_serial(m: &mut Message, s: u32) {
    unsafe { ffi::dbus_message_set_serial(m.msg, s) };
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::super::{Connection, ConnectionItem, Message, BusType, MessageItem, OwnedFd, libc, Path};

    #[test]
    fn unix_fd() {
        use std::io::prelude::*;
        use std::io::SeekFrom;
        use std::fs::OpenOptions;
        use std::os::unix::io::AsRawFd;

        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        let tempdir = tempdir::TempDir::new("dbus-rs-test").unwrap();
        let mut filename = tempdir.path().to_path_buf();
        filename.push("test");
        println!("Creating file {:?}", filename);
        let mut file = OpenOptions::new().create(true).read(true).write(true).open(&filename).unwrap();
        file.write_all(b"z").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let ofd = OwnedFd::new(file.as_raw_fd());
        m.append_items(&[MessageItem::UnixFd(ofd.clone())]);
        println!("Sending {:?}", m.get_items());
        c.send(m).unwrap();

        for n in c.iter(1000) {
            match n {
                ConnectionItem::MethodCall(m) => {
                    if let Some(&MessageItem::UnixFd(ref z)) = m.get_items().get(0) {
                        println!("Got {:?}", m.get_items());
                        let mut q: libc::c_char = 100;
                        assert_eq!(1, unsafe { libc::read(z.as_raw_fd(), &mut q as *mut _ as *mut libc::c_void, 1) });
                        assert_eq!(q, 'z' as libc::c_char);
                        break;
                    }
                    else {
                        panic!("Expected UnixFd, got {:?}", m.get_items());
                    }
                }
                _ => println!("Got {:?}", n),
            }
        }
    }

    #[test]
    fn message_types() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        m.append_items(&[
            2000u16.into(),
            MessageItem::new_array(vec!(129u8.into())).unwrap(),
            ["Hello", "world"][..].into(),
            987654321u64.into(),
            (-1i32).into(),
            format!("Hello world").into(),
            (-3.14f64).into(),
            MessageItem::Struct(vec!(256i16.into())),
            Path::new("/some/path").unwrap().into(),
            MessageItem::new_array(vec!((123543u32.into(), true.into()).into())).unwrap()
        ]);
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

    #[test]
    fn dict_of_dicts() {
        use std::collections::BTreeMap;

        let officeactions: BTreeMap<&'static str, MessageItem> = BTreeMap::new();
        let mut officethings = BTreeMap::new();
        officethings.insert("pencil", 2u16.into());
        officethings.insert("paper", 5u16.into());
        let mut homethings = BTreeMap::new();
        homethings.insert("apple", 11u16.into());
        let mut homeifaces = BTreeMap::new();
        homeifaces.insert("getThings", homethings);
        let mut officeifaces = BTreeMap::new();
        officeifaces.insert("getThings", officethings);
        officeifaces.insert("getActions", officeactions);
        let mut paths = BTreeMap::new();
        paths.insert("/hello/office", officeifaces);
        paths.insert("/hello/home", homeifaces);

        println!("Original treemap: {:?}", paths);
        let m = MessageItem::new_array(paths.iter().map(
            |(path, ifaces)| (MessageItem::ObjectPath(Path::new(*path).unwrap()),
                MessageItem::new_array(ifaces.iter().map(
                    |(iface, props)| (iface.to_string().into(),
                        MessageItem::from_dict::<(),_>(props.iter().map(
                            |(name, value)| Ok((name.to_string(), value.clone()))
                        )).unwrap()
                    ).into()
                ).collect()).unwrap()
            ).into()
        ).collect()).unwrap();
        println!("As MessageItem: {:?}", m);
        assert_eq!(m.type_sig(), "a{oa{sa{sv}}}");

        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut msg = Message::new_method_call(&c.unique_name(), "/hello", "org.freedesktop.DBusObjectManager", "GetManagedObjects").unwrap();
        msg.append_items(&[m]);
        let sending = format!("{:?}", msg.get_items());
        println!("Sending {}", sending);
        c.send(msg).unwrap();

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

    #[test]
    fn issue24() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut m = Message::new_method_call("org.test.rust", "/", "org.test.rust", "Test").unwrap();

        let a = MessageItem::from("test".to_string());
        let b = MessageItem::from("test".to_string());
        let foo = MessageItem::Struct(vec!(a, b));
        let bar = foo.clone();

        let args = [MessageItem::new_array(vec!(foo, bar)).unwrap()];
        println!("{:?}", args);

        m.append_items(&args);
        c.send(m).unwrap();
    }
}
