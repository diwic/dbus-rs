#![allow(dead_code)]

use libc::{c_void, c_char, c_uint, c_int};

pub type DBusConnection = c_void;
pub type DBusMessage = c_void;

#[repr(C)]
pub enum DBusBusType {
    Session = 0i,
    System = 1i,
    Starter = 2i,
}

pub const DBUS_TYPE_ARRAY: c_int = 'a' as c_int;
pub const DBUS_TYPE_BOOLEAN: c_int = 'b' as c_int;
pub const DBUS_TYPE_INVALID: c_int = 0;
pub const DBUS_TYPE_STRING: c_int = 's' as c_int;
pub const DBUS_TYPE_BYTE: c_int = 'y' as c_int;
pub const DBUS_TYPE_INT16: c_int = 'n' as c_int;
pub const DBUS_TYPE_UINT16: c_int = 'q' as c_int;
pub const DBUS_TYPE_INT32: c_int = 'i' as c_int;
pub const DBUS_TYPE_UINT32: c_int = 'u' as c_int;
pub const DBUS_TYPE_INT64: c_int = 'x' as c_int;
pub const DBUS_TYPE_UINT64: c_int = 't' as c_int;


#[repr(C)]
pub enum DBusHandlerResult {
    Handled = 0i,
    NotYetHandled = 1i,
    NeedMemory = 2i,
}

#[deriving(Show)]
#[repr(C)]
pub enum DBusMessageType {
    Invalid = 0i,
    MethodCall = 1i,
    MethodReturn = 2i,
    Error = 3i,
    Signal = 4i,
}

#[deriving(Show, PartialEq)]
#[repr(C)]
pub enum DBusDispatchStatus {
    DataRemains = 0i,
    Complete = 1i,
    NeedMemory_ = 2i,
}

#[repr(C)]
pub struct DBusError {
    pub name: *const c_char,
    pub message: *const c_char,
    pub dummy: c_uint,
    pub padding1: *const c_void
}

#[repr(C)]
pub struct DBusMessageIter {
    pub dummy1: *mut (),
    pub dummy2: *mut (),
    pub dummy3: u32,
    pub dummy4: c_int,
    pub dummy5: c_int,
    pub dummy6: c_int,
    pub dummy7: c_int,
    pub dummy8: c_int,
    pub dummy9: c_int,
    pub dummy10: c_int,
    pub dummy11: c_int,
    pub pad1: c_int,
    pub pad2: c_int,
    pub pad3: *mut (),
}

#[repr(C)]
pub struct DBusObjectPathVTable {
    pub unregister_function: Option<extern fn(conn: *mut DBusConnection, user_data: *mut c_void)>,
    pub message_function: Option<
        extern fn(conn: *mut DBusConnection, msg: *mut DBusMessage, user_data: *mut c_void) -> DBusHandlerResult>,
    pub dbus_internal_pad1: Option<extern fn()>,
    pub dbus_internal_pad2: Option<extern fn()>,
    pub dbus_internal_pad3: Option<extern fn()>,
    pub dbus_internal_pad4: Option<extern fn()>,
}

#[link(name = "dbus-1")]
extern "C" {
    pub fn dbus_bus_get_private(t: DBusBusType, error: *mut DBusError) -> *mut DBusConnection;
    pub fn dbus_bus_get_unique_name(conn: *mut DBusConnection) -> *const c_char;

    pub fn dbus_connection_close(conn: *mut DBusConnection);
    pub fn dbus_connection_dispatch(conn: *mut DBusConnection) -> DBusDispatchStatus;
    pub fn dbus_connection_flush(conn: *mut DBusConnection);
    pub fn dbus_connection_unref(conn: *mut DBusConnection);
    pub fn dbus_connection_set_exit_on_disconnect(conn: *mut DBusConnection, enable: u32);
    pub fn dbus_connection_send_with_reply_and_block(conn: *mut DBusConnection,
        message: *mut DBusMessage, timeout_milliseconds: c_int, error: *mut DBusError) -> *mut DBusMessage;
    pub fn dbus_connection_send(conn: *mut DBusConnection,
        message: *mut DBusMessage, serial: *mut u32) -> u32;
    pub fn dbus_connection_read_write_dispatch(conn: *mut DBusConnection,
        timeout_milliseconds: c_int) -> u32;
    pub fn dbus_connection_try_register_object_path(conn: *mut DBusConnection,
        path: *const c_char, vtable: *const DBusObjectPathVTable, user_data: *mut c_void,
        error: *mut DBusError) -> u32;
    pub fn dbus_connection_unregister_object_path(conn: *mut DBusConnection,
        path: *const c_char) -> u32;

    pub fn dbus_error_init(error: *mut DBusError);
    pub fn dbus_error_free(error: *mut DBusError);

    pub fn dbus_message_new_method_call(destination: *const c_char, path: *const c_char,
        iface: *const c_char, method: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_new_method_return(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_ref(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_unref(message: *mut DBusMessage);
    pub fn dbus_message_get_type(message: *mut DBusMessage) -> c_int;

    pub fn dbus_message_iter_append_basic(iter: *mut DBusMessageIter, t: c_int, value: *const c_void) -> u32;
    pub fn dbus_message_iter_init(message: *mut DBusMessage, iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_init_append(message: *mut DBusMessage, iter: *mut DBusMessageIter);
    pub fn dbus_message_iter_get_arg_type(iter: *mut DBusMessageIter) -> c_int;
    pub fn dbus_message_iter_get_basic(iter: *mut DBusMessageIter, value: *mut c_void);
    pub fn dbus_message_iter_next(iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_recurse(iter: *mut DBusMessageIter, subiter: *mut DBusMessageIter);
    pub fn dbus_message_iter_open_container(iter: *mut DBusMessageIter, _type: c_int,
        contained_signature: *const c_char, sub: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_close_container(iter: *mut DBusMessageIter, sub: *mut DBusMessageIter) -> u32;

    pub fn dbus_threads_init_default() -> c_int;
}
