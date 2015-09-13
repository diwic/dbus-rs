#![allow(dead_code)]

use libc::{c_void, c_char, c_uint, c_int};

pub type DBusConnection = c_void;
pub type DBusMessage = c_void;
pub type DBusCallback = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> DBusHandlerResult;
pub type DBusWatch = c_void;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusBusType {
    Session = 0,
    System = 1,
    Starter = 2,
}

pub const DBUS_TYPE_ARRAY: c_int = 'a' as c_int;
pub const DBUS_TYPE_VARIANT: c_int = 'v' as c_int;
pub const DBUS_TYPE_BOOLEAN: c_int = 'b' as c_int;
pub const DBUS_TYPE_INVALID: c_int = 0;
pub const DBUS_TYPE_STRING: c_int = 's' as c_int;
pub const DBUS_TYPE_DICT_ENTRY: c_int = 'e' as c_int;
pub const DBUS_TYPE_BYTE: c_int = 'y' as c_int;
pub const DBUS_TYPE_INT16: c_int = 'n' as c_int;
pub const DBUS_TYPE_UINT16: c_int = 'q' as c_int;
pub const DBUS_TYPE_INT32: c_int = 'i' as c_int;
pub const DBUS_TYPE_UINT32: c_int = 'u' as c_int;
pub const DBUS_TYPE_INT64: c_int = 'x' as c_int;
pub const DBUS_TYPE_UINT64: c_int = 't' as c_int;
pub const DBUS_TYPE_DOUBLE: c_int = 'd' as c_int;
pub const DBUS_TYPE_UNIX_FD: c_int = 'h' as c_int;
pub const DBUS_TYPE_STRUCT: c_int = 'r' as c_int;
pub const DBUS_TYPE_OBJECT_PATH: c_int = 'o' as c_int;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusNameFlag {
    AllowReplacement = 1,
    ReplaceExisting = 2,
    DoNotQueue = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusRequestNameReply {
    PrimaryOwner = 1,
    InQueue = 2,
    Exists = 3,
    AlreadyOwner = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusReleaseNameReply {
    Released = 1,
    NonExistent = 2,
    NotOwner = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusHandlerResult {
    Handled = 0,
    NotYetHandled = 1,
    NeedMemory = 2,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusMessageType {
    Invalid = 0,
    MethodCall = 1,
    MethodReturn = 2,
    Error = 3,
    Signal = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusDispatchStatus {
    DataRemains = 0,
    Complete = 1,
    NeedMemory = 2,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusWatchEvent {
    Readable = 1,
    Writable = 2,
    Error = 4,
    Hangup = 8,
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
    pub dummy1: *mut c_void,
    pub dummy2: *mut c_void,
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
    pub pad3: *mut c_void,
}

pub type DBusHandleMessageFunction = Option<
        extern fn(conn: *mut DBusConnection, msg: *mut DBusMessage, user_data: *mut c_void) -> DBusHandlerResult>;

pub type DBusAddWatchFunction = Option<extern fn(watch: *mut DBusWatch, user_data: *mut c_void) -> u32>;
pub type DBusRemoveWatchFunction = Option<extern fn(watch: *mut DBusWatch, user_data: *mut c_void)>;
pub type DBusWatchToggledFunction = Option<extern fn(watch: *mut DBusWatch, user_data: *mut c_void)>;

#[repr(C)]
pub struct DBusObjectPathVTable {
    pub unregister_function: Option<extern fn(conn: *mut DBusConnection, user_data: *mut c_void)>,
    pub message_function: DBusHandleMessageFunction,
    pub dbus_internal_pad1: Option<extern fn()>,
    pub dbus_internal_pad2: Option<extern fn()>,
    pub dbus_internal_pad3: Option<extern fn()>,
    pub dbus_internal_pad4: Option<extern fn()>,
}

#[link(name = "dbus-1")]
extern "C" {
    pub fn dbus_bus_get_private(t: DBusBusType, error: *mut DBusError) -> *mut DBusConnection;
    pub fn dbus_bus_get_unique_name(conn: *mut DBusConnection) -> *const c_char;
    pub fn dbus_bus_request_name(conn: *mut DBusConnection, name: *const c_char,
        flags: c_uint, error: *mut DBusError) -> c_int;
    pub fn dbus_bus_release_name(conn: *mut DBusConnection, name: *const c_char,
        error: *mut DBusError) -> c_int;
    pub fn dbus_bus_add_match(conn: *mut DBusConnection, rule: *const c_char,
        error: *mut DBusError);
    pub fn dbus_bus_remove_match(conn: *mut DBusConnection, rule: *const c_char,
        error: *mut DBusError);

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
    pub fn dbus_connection_list_registered(conn: *mut DBusConnection,
        parent_path: *const c_char, child_entries: *mut *mut *mut c_char) -> u32;
    pub fn dbus_connection_add_filter(conn: *mut DBusConnection, function: DBusHandleMessageFunction,
        user_data: *mut c_void, free_data_function: Option<extern fn(memory: *mut c_void)>) -> u32;
    pub fn dbus_connection_remove_filter(conn: *mut DBusConnection, function: DBusHandleMessageFunction,
        user_data: *mut c_void) -> u32;
    pub fn dbus_connection_set_watch_functions(conn: *mut DBusConnection, add_function: DBusAddWatchFunction,
        remove_function: DBusRemoveWatchFunction, toggled_function: DBusWatchToggledFunction,
        data: *mut c_void, free_data_function: Option<extern fn(memory: *mut c_void)>) -> u32;

    pub fn dbus_error_init(error: *mut DBusError);
    pub fn dbus_error_free(error: *mut DBusError);
    pub fn dbus_set_error(error: *mut DBusError, name: *const c_char, message: *const c_char, ...);
    pub fn dbus_set_error_from_message(error: *mut DBusError, message: *mut DBusMessage) -> u32;

    pub fn dbus_message_new_method_call(destination: *const c_char, path: *const c_char,
        iface: *const c_char, method: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_new_method_return(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_new_error(message: *mut DBusMessage,
        error_name: *const c_char, error_message: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_new_signal(path: *const c_char,
        iface: *const c_char, name: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_ref(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_unref(message: *mut DBusMessage);
    pub fn dbus_message_get_type(message: *mut DBusMessage) -> c_int;
    pub fn dbus_message_is_method_call(message: *mut DBusMessage, iface: *const c_char, method: *const c_char) -> u32;
    pub fn dbus_message_is_signal(message: *mut DBusMessage, iface: *const c_char, signal_name: *const c_char) -> u32;
    pub fn dbus_message_get_reply_serial(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_get_serial(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_get_path(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_interface(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_member(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_sender(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_set_serial(message: *mut DBusMessage, serial: u32);

    pub fn dbus_message_iter_append_basic(iter: *mut DBusMessageIter, t: c_int, value: *const c_void) -> u32;
    pub fn dbus_message_iter_init(message: *mut DBusMessage, iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_init_append(message: *mut DBusMessage, iter: *mut DBusMessageIter);
    pub fn dbus_message_iter_get_arg_type(iter: *mut DBusMessageIter) -> c_int;
    pub fn dbus_message_iter_get_basic(iter: *mut DBusMessageIter, value: *mut c_void);
    pub fn dbus_message_iter_get_signature(iter: *mut DBusMessageIter) -> *mut c_char;
    pub fn dbus_message_iter_next(iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_recurse(iter: *mut DBusMessageIter, subiter: *mut DBusMessageIter);
    pub fn dbus_message_iter_open_container(iter: *mut DBusMessageIter, _type: c_int,
        contained_signature: *const c_char, sub: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_close_container(iter: *mut DBusMessageIter, sub: *mut DBusMessageIter) -> u32;

    pub fn dbus_free(memory: *mut c_void);
    pub fn dbus_free_string_array(str_array: *mut *mut c_char) -> c_void;

    pub fn dbus_signature_validate_single(signature: *const c_char, error: *mut DBusError) -> u32;

    pub fn dbus_threads_init_default() -> c_int;

    pub fn dbus_validate_bus_name(busname: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_error_name(errorname: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_interface(interface: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_member(member: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_path(path: *const c_char, error: *mut DBusError) -> u32;

    pub fn dbus_watch_get_enabled(watch: *mut DBusWatch) -> u32;
    pub fn dbus_watch_get_flags(watch: *mut DBusWatch) -> c_uint;
    pub fn dbus_watch_get_unix_fd(watch: *mut DBusWatch) -> c_int;
    pub fn dbus_watch_handle(watch: *mut DBusWatch, flags: c_uint) -> u32;
}
