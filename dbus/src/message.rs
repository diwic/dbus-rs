use std::{fmt, mem, ptr};
use super::{ffi, Error, MessageType, libc, to_c_str, c_str_to_slice, init_dbus};
use super::{BusName, Path, Interface, Member, ErrorName, Connection, SignalArgs};
use std::os::unix::io::{RawFd, AsRawFd};
use std::ffi::CStr;

use super::arg::{Append, IterAppend, Get, Iter, Arg, RefArg, TypeMismatchError};

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
        let x = unsafe { libc::dup(self.fd) };
        if x == -1 { panic!("Duplicating file descriptor failed") }
        OwnedFd::new(x)
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
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

        let p = Path::new(path)?;
        let i = Interface::new(iface)?;
        let m = Member::new(name)?;

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
    pub fn get_items(&self) -> Vec<crate::arg::messageitem::MessageItem> {
        let mut i = self.iter_init();
        let mut v = vec!();
        while let Some(z) = crate::arg::messageitem::MessageItem::get(&mut i) { v.push(z); i.next(); }
        v
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

    /// Returns true if the message does not expect a reply.
    pub fn get_no_reply(&self) -> bool { unsafe { ffi::dbus_message_get_no_reply(self.msg) != 0 } }

    /// Set whether or not the message expects a reply.
    ///
    /// Set to true if you send a method call and do not want a reply.
    pub fn set_no_reply(&self, v: bool) {
        unsafe { ffi::dbus_message_set_no_reply(self.msg, if v { 1 } else { 0 }) }
    }

    /// Returns true if the message can cause a service to be auto-started.
    pub fn get_auto_start(&self) -> bool { unsafe { ffi::dbus_message_get_auto_start(self.msg) != 0 } }

    /// Sets whether or not the message can cause a service to be auto-started.
    ///
    /// Defaults to true.
    pub fn set_auto_start(&self, v: bool) {
        unsafe { ffi::dbus_message_set_auto_start(self.msg, if v { 1 } else { 0 }) }
    }

    /// Add one or more MessageItems to this Message.
    ///
    /// Note: using `append1`, `append2` or `append3` might be faster, especially for large arrays.
    /// This method is provided for backwards compatibility.
    pub fn append_items(&mut self, v: &[crate::arg::messageitem::MessageItem]) {
        let mut ia = IterAppend::new(self);
        for a in v { a.append_by_ref(&mut ia); }
    }

    /// Appends one MessageItem to a message.
    /// Use in builder style: e g `m.method_return().append(7i32)`
    ///
    /// Note: using `append1`, `append2` or `append3` might be faster, especially for large arrays.
    /// This method is provided for backwards compatibility.
    #[deprecated]
    pub fn append<I: Into<crate::arg::messageitem::MessageItem>>(self, v: I) -> Self {
        self.append1(v.into())
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
        Ok((i.read()?, i.read()?))
    }

    /// Gets the first three arguments from the message, if those arguments are of type G1, G2 and G3.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read3<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>>(&'a self) -> 
        Result<(G1, G2, G3), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((i.read()?, i.read()?, i.read()?))
    }

    /// Gets the first four arguments from the message, if those arguments are of type G1, G2, G3 and G4.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    pub fn read4<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>, G4: Arg + Get<'a>>(&'a self) ->
        Result<(G1, G2, G3, G4), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((i.read()?, i.read()?, i.read()?, i.read()?))
    }

    /// Gets the first five arguments from the message, if those arguments are of type G1, G2, G3, G4 and G5.
    ///
    /// Returns a TypeMismatchError if there are not enough arguments, or if types don't match.
    /// Note: If you need more than five arguments, use `iter_init` instead.
    pub fn read5<'a, G1: Arg + Get<'a>, G2: Arg + Get<'a>, G3: Arg + Get<'a>, G4: Arg + Get<'a>, G5: Arg + Get<'a>>(&'a self) ->
        Result<(G1, G2, G3, G4, G5), TypeMismatchError> {
        let mut i = Iter::new(&self);
        Ok((i.read()?, i.read()?, i.read()?, i.read()?, i.read()?))
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

    /// Gets the destination this Message is being sent to.
    pub fn destination<'a>(&'a self) -> Option<BusName<'a>> {
        self.msg_internal_str(unsafe { ffi::dbus_message_get_destination(self.msg) })
            .map(|s| unsafe { BusName::from_slice_unchecked(s) })
    }

    /// Sets the destination of this Message
    ///
    /// If dest is none, that means broadcast to all relevant destinations.
    pub fn set_destination(&mut self, dest: Option<BusName>) {
        let c_dest = dest.as_ref().map(|d| d.as_cstr().as_ptr()).unwrap_or(ptr::null());
        assert!(unsafe { ffi::dbus_message_set_destination(self.msg, c_dest) } != 0);
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
        self.set_error_from_msg().map(|_| self)
    }

    pub (super) fn set_error_from_msg(&self) -> Result<(), Error> {
        let mut e = Error::empty();
        if unsafe { ffi::dbus_set_error_from_message(e.get_mut(), self.msg) } != 0 { Err(e) }
        else { Ok(()) }
    }

    pub (crate) fn ptr(&self) -> *mut ffi::DBusMessage { self.msg }

    pub (crate) fn from_ptr(ptr: *mut ffi::DBusMessage, add_ref: bool) -> Message {
        if add_ref {
            unsafe { ffi::dbus_message_ref(ptr) };
        }
        Message { msg: ptr }
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

    /// Emit a D-Bus signal, where the arguments are in a struct.
    pub fn emit<S: SignalArgs>(&self, signal: &S) -> Result<u32, Error> {
        let msg = signal.to_emit_message(&self.path);
        self.conn.send(msg).map_err(|_| Error::new_custom("org.freedesktop.DBus.Error.Failed", "Sending signal failed"))
    }
}

// For purpose of testing the library only.
#[cfg(test)]
pub (crate) fn message_set_serial(m: &mut Message, s: u32) {
    unsafe { ffi::dbus_message_set_serial(m.msg, s) };
}

#[cfg(test)]
mod test {
    use crate::{Message, BusName};

    #[test]
    fn set_valid_destination() {
        let mut m = Message::new_method_call("org.test.rust", "/", "org.test.rust", "Test").unwrap();
        let d = Some(BusName::new(":1.14").unwrap());
        m.set_destination(d);

        assert!(!m.get_no_reply());
        m.set_no_reply(true);
        assert!(m.get_no_reply());
    }
}
