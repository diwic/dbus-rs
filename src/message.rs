use std::borrow::IntoCow;
use std::{fmt, mem, ptr};
use super::{ffi, Error, MessageType, TypeSig, libc, to_c_str, c_str_to_slice, init_dbus};

fn new_dbus_message_iter() -> ffi::DBusMessageIter {
    ffi::DBusMessageIter {
        dummy1: ptr::null_mut(),
        dummy2: ptr::null_mut(),
        dummy3: 0,
        dummy4: 0,
        dummy5: 0,
        dummy6: 0,
        dummy7: 0,
        dummy8: 0,
        dummy9: 0,
        dummy10: 0,
        dummy11: 0,
        pad1: 0,
        pad2: 0,
        pad3: ptr::null_mut(),
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum MessageItem {
    Array(Vec<MessageItem>, TypeSig<'static>),
    Struct(Vec<MessageItem>),
    Variant(Box<MessageItem>),
    DictEntry(Box<MessageItem>, Box<MessageItem>),
    ObjectPath(String),
    Str(String),
    Bool(bool),
    Byte(u8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Double(f64),
}

fn iter_get_basic(i: &mut ffi::DBusMessageIter) -> i64 {
    let mut c: i64 = 0;
    unsafe {
        let p: *mut libc::c_void = mem::transmute(&mut c);
        ffi::dbus_message_iter_get_basic(i, p);
    }
    c
}

fn iter_get_f64(i: &mut ffi::DBusMessageIter) -> f64 {
    let mut c: f64 = 0.0;
    unsafe {
        let p: *mut libc::c_void = mem::transmute(&mut c);
        ffi::dbus_message_iter_get_basic(i, p);
    }
    c
}

fn iter_append_f64(i: &mut ffi::DBusMessageIter, v: f64) {
    unsafe {
        let p: *const libc::c_void = mem::transmute(&v);
        ffi::dbus_message_iter_append_basic(i, ffi::DBUS_TYPE_DOUBLE, p);
    }
}

fn iter_append_array(i: &mut ffi::DBusMessageIter, a: &[MessageItem], t: TypeSig<'static>) {
    let mut subiter = new_dbus_message_iter();
    let atype = to_c_str(t);

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
    let atype = to_c_str(format!("{}", a.array_type() as u8 as char));
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

    pub fn type_sig(&self) -> TypeSig<'static> {
        match self {
            // TODO: Can we make use of the ffi constants here instead of duplicating them?
            &MessageItem::Str(_) => "s".into_cow(),
            &MessageItem::Bool(_) => "b".into_cow(),
            &MessageItem::Byte(_) => "y".into_cow(),
            &MessageItem::Int16(_) => "n".into_cow(),
            &MessageItem::Int32(_) => "i".into_cow(),
            &MessageItem::Int64(_) => "x".into_cow(),
            &MessageItem::UInt16(_) => "q".into_cow(),
            &MessageItem::UInt32(_) => "u".into_cow(),
            &MessageItem::UInt64(_) => "t".into_cow(),
            &MessageItem::Double(_) => "d".into_cow(),
            &MessageItem::Array(_, ref s) => format!("a{}", s).into_cow(),
            &MessageItem::Struct(_) => "r".into_cow(),
            &MessageItem::Variant(_) => "v".into_cow(),
            &MessageItem::DictEntry(ref k, ref v) => format!("{{{}{}}}", k.type_sig(), v.type_sig()).into_cow(),
            &MessageItem::ObjectPath(_) => "o".into_cow(),
        }
    }

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
        };
        s as i32
    }

    // Creates a Array<String, Variant> from an iterator with Result passthrough (an Err will abort and return that Err)
    pub fn from_dict<E, I: Iterator<Item=Result<(String, MessageItem),E>>>(i: I) -> Result<MessageItem,E> {
        let mut v = Vec::new();
        for r in i {
            let (s, vv) = try!(r);
            v.push(MessageItem::DictEntry(Box::new(MessageItem::Str(s)), Box::new(MessageItem::Variant(
                Box::new(vv)))));
        }
        Ok(MessageItem::Array(v, "{sv}".into_cow()))
    }

    // Note: Will panic if the vec is empty or if there are different types in the array
    pub fn new_array(v: Vec<MessageItem>) -> MessageItem {
        let t = v[0].type_sig();
        for i in &v { debug_assert!(i.type_sig() == t) };
        MessageItem::Array(v, t)
    }

    fn from_iter(i: &mut ffi::DBusMessageIter) -> Vec<MessageItem> {
        let mut v = Vec::new();
        loop {
            let t = unsafe { ffi::dbus_message_iter_get_arg_type(i) };
            match t {
                ffi::DBUS_TYPE_INVALID => { return v },
                ffi::DBUS_TYPE_DICT_ENTRY => {
                    let mut subiter = new_dbus_message_iter();
                    unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                    let a = MessageItem::from_iter(&mut subiter);
                    if a.len() != 2 { panic!("D-Bus dict entry error"); }
                    let mut a = a.into_iter();
                    let key = Box::new(a.next().unwrap());
                    let value = Box::new(a.next().unwrap());
                    v.push(MessageItem::DictEntry(key, value));
                }
                ffi::DBUS_TYPE_VARIANT => {
                    let mut subiter = new_dbus_message_iter();
                    unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                    let a = MessageItem::from_iter(&mut subiter);
                    if a.len() != 1 { panic!("D-Bus variant error"); }
                    v.push(MessageItem::Variant(Box::new(a.into_iter().next().unwrap())));
                }
                ffi::DBUS_TYPE_ARRAY => {
                    let mut subiter = new_dbus_message_iter();
                    unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                    let a = MessageItem::from_iter(&mut subiter);
                    let t = if a.len() > 0 { a[0].type_sig() } else {
                        let c = unsafe { ffi::dbus_message_iter_get_signature(&mut subiter) };
                        let s = c_str_to_slice(&(c as *const libc::c_char)).unwrap().to_string();
                        unsafe { ffi::dbus_free(c as *mut libc::c_void) };
                        s.into_cow()
                    };
                    v.push(MessageItem::Array(a, t));
                },
                ffi::DBUS_TYPE_STRUCT => {
                    let mut subiter = new_dbus_message_iter();
                    unsafe { ffi::dbus_message_iter_recurse(i, &mut subiter) };
                    v.push(MessageItem::Struct(MessageItem::from_iter(&mut subiter)));
                },
                ffi::DBUS_TYPE_STRING => {
                    let mut c: *const libc::c_char = ptr::null();
                    unsafe {
                        let p: *mut libc::c_void = mem::transmute(&mut c);
                        ffi::dbus_message_iter_get_basic(i, p);
                    };
                    v.push(MessageItem::Str(c_str_to_slice(&c).expect("D-Bus string error").to_string()));
                },
                ffi::DBUS_TYPE_OBJECT_PATH => {
                    let mut c: *const libc::c_char = ptr::null();
                    unsafe {
                        let p: *mut libc::c_void = mem::transmute(&mut c);
                        ffi::dbus_message_iter_get_basic(i, p);
                    };
                    v.push(MessageItem::ObjectPath(c_str_to_slice(&c).expect("D-Bus object path error").to_string()));
                },
                ffi::DBUS_TYPE_BOOLEAN => v.push(MessageItem::Bool((iter_get_basic(i) as u32) != 0)),
                ffi::DBUS_TYPE_BYTE => v.push(MessageItem::Byte(iter_get_basic(i) as u8)),
                ffi::DBUS_TYPE_INT16 => v.push(MessageItem::Int16(iter_get_basic(i) as i16)),
                ffi::DBUS_TYPE_INT32 => v.push(MessageItem::Int32(iter_get_basic(i) as i32)),
                ffi::DBUS_TYPE_INT64 => v.push(MessageItem::Int64(iter_get_basic(i) as i64)),
                ffi::DBUS_TYPE_UINT16 => v.push(MessageItem::UInt16(iter_get_basic(i) as u16)),
                ffi::DBUS_TYPE_UINT32 => v.push(MessageItem::UInt32(iter_get_basic(i) as u32)),
                ffi::DBUS_TYPE_UINT64 => v.push(MessageItem::UInt64(iter_get_basic(i) as u64)),
                ffi::DBUS_TYPE_DOUBLE => v.push(MessageItem::Double(iter_get_f64(i))),

                _ => { panic!("D-Bus unsupported message type {} ({})", t, t as u8 as char); }
            }
            unsafe { ffi::dbus_message_iter_next(i) };
        }
    }

    fn iter_append_basic(&self, i: &mut ffi::DBusMessageIter, v: i64) {
        let t = self.array_type();
        unsafe {
            let p: *const libc::c_void = mem::transmute(&v);
            ffi::dbus_message_iter_append_basic(i, t as libc::c_int, p);
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
            &MessageItem::Double(b) => iter_append_f64(i, b),
            &MessageItem::Array(ref b, ref t) => iter_append_array(i, &**b, t.clone()),
            &MessageItem::Struct(ref v) => iter_append_struct(i, &**v),
            &MessageItem::Variant(ref b) => iter_append_variant(i, &**b),
            &MessageItem::DictEntry(ref k, ref v) => iter_append_dict(i, &**k, &**v),
            &MessageItem::ObjectPath(ref s) => unsafe {
                let c = to_c_str(s);
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
}

pub struct Message {
    msg: *mut ffi::DBusMessage,
}

impl Message {
    pub fn new_method_call(destination: &str, path: &str, iface: &str, method: &str) -> Option<Message> {
        init_dbus();
        let (d, p, i, m) = (to_c_str(destination), to_c_str(path), to_c_str(iface), to_c_str(method));
        let ptr = unsafe {
            ffi::dbus_message_new_method_call(d.as_ptr(), p.as_ptr(), i.as_ptr(), m.as_ptr())
        };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    pub fn new_signal(path: &str, iface: &str, method: &str) -> Option<Message> {
        init_dbus();
        let (p, i, m) = (to_c_str(path), to_c_str(iface), to_c_str(method));
        let ptr = unsafe {
            ffi::dbus_message_new_signal(p.as_ptr(), i.as_ptr(), m.as_ptr())
        };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    pub fn new_method_return(m: &Message) -> Option<Message> {
        let ptr = unsafe { ffi::dbus_message_new_method_return(m.msg) };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    pub fn new_error(m: &Message, error_name: &str, error_message: &str) -> Option<Message> {
        let (en, em) = (to_c_str(error_name), to_c_str(error_message));
        let ptr = unsafe { ffi::dbus_message_new_error(m.msg, en.as_ptr(), em.as_ptr()) };
        if ptr == ptr::null_mut() { None } else { Some(Message { msg: ptr} ) }
    }

    pub fn get_items(&mut self) -> Vec<MessageItem> {
        let mut i = new_dbus_message_iter();
        match unsafe { ffi::dbus_message_iter_init(self.msg, &mut i) } {
            0 => Vec::new(),
            _ => MessageItem::from_iter(&mut i)
        }
    }

    pub fn append_items(&mut self, v: &[MessageItem]) {
        let mut i = new_dbus_message_iter();
        unsafe { ffi::dbus_message_iter_init_append(self.msg, &mut i) };
        MessageItem::copy_to_iter(&mut i, v);
    }

    pub fn msg_type(&self) -> MessageType {
        unsafe { mem::transmute(ffi::dbus_message_get_type(self.msg)) }
    }

    pub fn sender(&self) -> Option<String> {
        let s = unsafe { ffi::dbus_message_get_sender(self.msg) };
        c_str_to_slice(&s).map(|s| s.to_string())
    }

    pub fn headers(&self) -> (MessageType, Option<String>, Option<String>, Option<String>) {
        let p = unsafe { ffi::dbus_message_get_path(self.msg) };
        let i = unsafe { ffi::dbus_message_get_interface(self.msg) };
        let m = unsafe { ffi::dbus_message_get_member(self.msg) };
        (self.msg_type(),
         c_str_to_slice(&p).map(|s| s.to_string()),
         c_str_to_slice(&i).map(|s| s.to_string()),
         c_str_to_slice(&m).map(|s| s.to_string()))
    }

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

#[cfg(test)]
mod test {
    use super::super::{Connection, ConnectionItem, Message, BusType, MessageItem};

    #[test]
    fn message_types() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut m = Message::new_method_call(&*c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        m.append_items(&[
            MessageItem::UInt16(2000),
            MessageItem::new_array(vec!(MessageItem::Byte(129))),
            MessageItem::UInt64(987654321),
            MessageItem::Int32(-1),
            MessageItem::Str(format!("Hello world")),
            MessageItem::Double(-3.14),
            MessageItem::new_array(vec!(
                MessageItem::DictEntry(Box::new(MessageItem::UInt32(123543)), Box::new(MessageItem::Bool(true)))
            ))
        ]);
        let sending = format!("{:?}", m.get_items());
        println!("Sending {}", sending);
        c.send(m).unwrap();

        for n in c.iter(1000) {
            match n {
                ConnectionItem::MethodCall(mut m) => {
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
        officethings.insert("pencil", MessageItem::UInt16(2));
        officethings.insert("paper", MessageItem::UInt16(5));
        let mut homethings = BTreeMap::new();
        homethings.insert("apple", MessageItem::UInt16(11));
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
            |(path, ifaces)| MessageItem::DictEntry(Box::new(MessageItem::ObjectPath(path.to_string())), Box::new(
                MessageItem::new_array(ifaces.iter().map(
                    |(iface, props)| MessageItem::DictEntry(Box::new(MessageItem::Str(iface.to_string())), Box::new(
                        MessageItem::from_dict::<(),_>(props.iter().map(|(name, value)| Ok((name.to_string(), value.clone())))).unwrap()
                    ))
                ).collect())
            ))
        ).collect());
        println!("As MessageItem: {:?}", m);
        assert_eq!(m.type_sig(), "a{oa{sa{sv}}}");

        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut msg = Message::new_method_call(&*c.unique_name(), "/hello", "org.freedesktop.DBusObjectManager", "GetManagedObjects").unwrap();
        msg.append_items(&[m]);
        let sending = format!("{:?}", msg.get_items());
        println!("Sending {}", sending);
        c.send(msg).unwrap();

        for n in c.iter(1000) {
            match n {
                ConnectionItem::MethodCall(mut m) => {
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
