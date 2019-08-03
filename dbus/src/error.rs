use std::ptr;
use crate::{tree, arg, to_c_str, c_str_to_slice, init_dbus};
use crate::strings::ErrorName;

/// D-Bus Error wrapper.
pub struct Error {
    e: ffi::DBusError,
}

unsafe impl Send for Error {}

// Note! For this Sync impl to be safe, it requires that no functions that take &self,
// actually calls into FFI. All functions that call into FFI with a ffi::DBusError
// must take &mut self.

unsafe impl Sync for Error {}

impl Error {

    /// Create a new custom D-Bus Error.
    pub fn new_custom<'a, N: Into<ErrorName<'a>>>(name: N, message: &str) -> Error {
        let n = to_c_str(&name.into());
        let m = to_c_str(&message.replace("%","%%"));
        let mut e = Error::empty();

        unsafe { ffi::dbus_set_error(e.get_mut(), n.as_ptr(), m.as_ptr()) };
        e
    }

    /// Create a new generic D-Bus Error with "org.freedesktop.DBus.Error.Failed" as the Error name.
    pub fn new_failed(message: &str) -> Error {
        Error::new_custom("org.freedesktop.DBus.Error.Failed", message)
    }

    pub (crate) fn empty() -> Error {
        init_dbus();
        let mut e = ffi::DBusError {
            name: ptr::null(),
            message: ptr::null(),
            dummy: 0,
            padding1: ptr::null()
        };
        unsafe { ffi::dbus_error_init(&mut e); }
        Error{ e: e }
    }

    /// Error name/type, e g 'org.freedesktop.DBus.Error.Failed'
    pub fn name(&self) -> Option<&str> {
        c_str_to_slice(&self.e.name)
    }

    /// Custom message, e g 'Could not find a matching object path'
    pub fn message(&self) -> Option<&str> {
        c_str_to_slice(&self.e.message)
    }

    pub (crate) fn get_mut(&mut self) -> &mut ffi::DBusError { &mut self.e }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { ffi::dbus_error_free(&mut self.e); }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "D-Bus error: {} ({})", self.message().unwrap_or(""),
            self.name().unwrap_or(""))
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str { "D-Bus error" }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(),std::fmt::Error> {
        if let Some(x) = self.message() {
             write!(f, "{:?}", x.to_string())
        } else { Ok(()) }
    }
}

impl From<arg::TypeMismatchError> for Error {
    fn from(t: arg::TypeMismatchError) -> Error {
        Error::new_custom("org.freedesktop.DBus.Error.Failed", &format!("{}", t))
    }
}


impl From<tree::MethodErr> for Error {
    fn from(t: tree::MethodErr) -> Error {
        Error::new_custom(t.errorname(), t.description())
    }
}

