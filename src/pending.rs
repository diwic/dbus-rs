use super::ffi;

pub struct PendingCall {
    pc: *mut ffi::DBusPendingCall,
}

impl Drop for PendingCall {
    fn drop(&mut self) {
        unsafe { ffi::dbus_pending_call_unref(self.pc) };
    }
}

pub fn pending_call_from_ptr(ptr: *mut ffi::DBusMessage, add_ref: bool) -> PendingCall {
    if add_ref {
        unsafe { ffi::dbus_pending_call_ref(ptr) };
    }
    PendingCall { pc: ptr }
}

pub fn get_pending_call_ptr<'a>(pc: &PendingCall) -> *mut ffi::DBusPendingCall {
    pc.pc
}