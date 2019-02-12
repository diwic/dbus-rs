use crate::{Message, MessageType, Error, to_c_str, c_str_to_slice};
use std::ptr;

use std::collections::HashMap;

/// [Unstable and Experimental]
pub trait MessageDispatcherConfig {
    /// The type of method reply stored in the hashmap
    type Reply;
    /// Call the reply
    fn call_reply(_: Self::Reply, _: Message);
}

/// [Unstable and Experimental] Meant for usage with RxTx.
pub struct MessageDispatcher<C: MessageDispatcherConfig> {
    waiting_replies: HashMap<u32, C::Reply>
}

impl<C: MessageDispatcherConfig> MessageDispatcher<C> {

    pub fn new() -> Self { MessageDispatcher { waiting_replies: HashMap::new() } }

    /// Adds a waiting reply to a method call. func will be called when a method reply is dispatched.
    pub fn add_reply(&mut self, serial: u32, func: C::Reply) {
        if let Some(_) = self.waiting_replies.insert(serial, func) {
            // panic because we're overwriting something else, or just ignore?
        }
    }

    /// Cancels a waiting reply.
    pub fn cancel_reply(&mut self, serial: u32) -> Option<C::Reply> {
        self.waiting_replies.remove(&serial)
    }

    /// Dispatch an incoming message.
    pub fn dispatch(&mut self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            if let Some(sender) = self.waiting_replies.remove(&serial) {
                C::call_reply(sender, msg);
                return;
            }
        }
        Self::default_dispatch(&msg);
    }

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


