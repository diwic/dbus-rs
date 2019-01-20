use crate::{Message, MessageType, Error, to_c_str, c_str_to_slice};
use std::ptr;

/// [Unstable and Experimental] Meant for usage with Connection2.
pub struct MessageDispatcher;

impl MessageDispatcher {

    /// Handles what we need to be a good D-Bus citizen.
    ///
    /// Call this if you have not handled the message yourself:
    /// * It handles calls to org.freedesktop.DBus.Peer.
    /// * For other method calls, it sends an error reply back that the method was unknown.
    pub fn default_dispatch(m: &Message) -> Option<Message> {
        Self::peer(&m)
            .or_else(|| Self::unknown_method(&m))
    }

    /// Replies if this is a call to org.freedesktop.DBus.Peer, otherwise returns None.
    pub fn peer(m: &Message) -> Option<Message> {
        if let Some(intf) = m.interface() {
            if &*intf != "org.freedesktop.DBus.Peer" { return None; }
            if let Some(method) = m.member() {
                if &*method == "Ping" { return Some(m.method_return()) }
                if &*method == "GetMachineId" {
                    let mut r = m.method_return();
                    let mut e = Error::empty();
                    unsafe {
                        let id = ffi::dbus_try_get_local_machine_id(e.get_mut());
                        if id != ptr::null_mut() {
                            r = r.append1(c_str_to_slice(&(id as *const _)).unwrap());
                            ffi::dbus_free(id as *mut _);
                            return Some(r)
                        }
                    }
                    
                }
            }
            Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Method does not exist")))
        } else { None }
    }

    /// For method calls, it replies that the method was unknown, otherwise returns None.
    pub fn unknown_method(m: &Message) -> Option<Message> {
        if m.msg_type() != MessageType::MethodCall { return None; }
        // if m.get_no_reply() { return None; } // The reference implementation does not do this?
        Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Path, Interface, or Method does not exist")))
    }
}


