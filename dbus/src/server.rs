//! DBus server for hosting buses
use std::{ffi::{CString, CStr, c_char}, mem::{MaybeUninit, ManuallyDrop}};

use libc::c_void;

use crate::{channel::Channel, watch};

use super::{ffi, Error};

#[derive(Clone, Debug)]
struct ServerHandle(*mut ffi::DBusServer);
unsafe impl Send for ServerHandle {}
unsafe impl Sync for ServerHandle {}

impl super::watch::SetWatchFunctions for ServerHandle {
    unsafe fn set_watch_functions(&self, add_function: ffi::DBusAddWatchFunction,
        remove_function: ffi::DBusRemoveWatchFunction, toggled_function: ffi::DBusWatchToggledFunction,
        data: *mut std::os::raw::c_void, free_data_function: ffi::DBusFreeFunction) -> u32
    {
        ffi::dbus_server_set_watch_functions(self.0, add_function, remove_function, toggled_function, data, free_data_function)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        unsafe { 
            if ffi::dbus_server_get_is_connected(self.0) != 0 {
                ffi::dbus_server_disconnect(self.0);
            }
            ffi::dbus_server_unref(self.0);
        }
    }
}

/// DBus server
pub struct Server {
    handle: watch::WithWatchMap<ServerHandle>,
    allow_watch_funcs: bool,
}

impl Server {
    /// Listens for new connections on the given address.
    /// 
    /// # Panics
    /// Panics if address has a null byte
    pub fn listen(address: &str) -> Result<Self, Error> {
        let mut error = Error::empty();
        let address = CString::new(address).unwrap();
        let server = unsafe { ffi::dbus_server_listen(address.as_ptr(), error.get_mut()) };
        if server.is_null() {
            Err(error)
        } else {
            Ok(Self {
                handle: watch::WithWatchMap::new(ServerHandle(server)),
                allow_watch_funcs: true,
            })
        }
    }
    /// Check whether the server is still listening for new connections.
    pub fn is_connected(&mut self) -> bool {
        0 != unsafe { ffi::dbus_server_get_is_connected(self.handle.inner().0) }
    }
    /// Releases the server's address and stops listening for new clients.
    pub fn disconnect(&mut self) {
        unsafe { ffi::dbus_server_disconnect(self.handle.inner().0); }
    }
    /// Returns the address of the server.
    pub fn address(&mut self) -> String {
        let adr_ptr = unsafe { ffi::dbus_server_get_address(self.handle.inner().0) };
        let adr = unsafe { CStr::from_ptr(adr_ptr) };
        let ret = std::str::from_utf8(adr.to_bytes()).unwrap().to_owned();
        unsafe { ffi::dbus_free(adr_ptr as *mut c_void); }
        ret
    }
    /// Returns the unique ID of the server.
    pub fn id(&mut self) -> String {
        let id_ptr = unsafe { ffi::dbus_server_get_id(self.handle.inner().0) };
        let id = unsafe { CStr::from_ptr(id_ptr) };
        let ret = std::str::from_utf8(id.to_bytes()).unwrap().to_owned();
        unsafe { ffi::dbus_free(id_ptr as *mut c_void); }
        ret
    }
    /// Enables watch tracking, a prequisite for calling watch.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn set_watch_enabled(&mut self, enable: bool) {
        assert!(self.allow_watch_funcs);
        self.handle.set_watch_enabled(enable)
    }
    /// Gets the file descriptor to listen for read/write.
    ///
    /// Panics: if set_watch_enabled is false.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn watch(&self) -> watch::Watch {
        assert!(self.allow_watch_funcs);
        self.handle.watch()
    }
    /// Sets the authentication mechanisms that this server offers to clients.
    ///
    /// # Panics
    /// Panics if at least one of the mechanisms has a null byte.
    pub fn set_auth_mechanisms<S: AsRef<str>>(&self, mechanisms: &[S]) {
        let mut mechanisms = mechanisms.iter().map(|mech| {
            MaybeUninit::new(CString::new(mech.as_ref()).unwrap())
        }).collect::<Vec<_>>();
        mechanisms.push(MaybeUninit::zeroed());
        assert!(
            0 != unsafe { ffi::dbus_server_set_auth_mechanisms(self.handle.inner().0, mechanisms.as_mut_ptr() as *mut *const c_char) }
        );
        mechanisms.pop();
        mechanisms.into_iter().for_each(|mut x| unsafe { x.assume_init_drop() });
    }
    /// Sets a function to be used for handling new connections.
    pub fn set_new_connection_callback<F: Fn(&Server, Channel) -> bool + 'static>(&self, callback: F) {
        let callback_ptr = Box::into_raw(Box::new(callback)) as *mut c_void;
        extern "C" fn free_data_function<F>(data: *mut c_void) {
            drop(unsafe { Box::from_raw(data as *mut F) });
        }
        extern "C" fn function<F: Fn(&Server, Channel) -> bool + 'static>(server: *mut ffi::DBusServer, new_connection: *mut ffi::DBusConnection, data: *mut c_void) {
            let closure = ManuallyDrop::new(unsafe { Box::from_raw(data as *mut F) });
            let server = ManuallyDrop::new(Server {
                allow_watch_funcs: false,
                handle: watch::WithWatchMap::new(ServerHandle(server)),
            });
            let new_connection = Channel::conn_from_ptr(new_connection).unwrap();
            closure(&server, new_connection);
        }
        unsafe {
            ffi::dbus_server_set_new_connection_function(self.handle.inner().0, Some(function::<F>), callback_ptr, Some(free_data_function::<F>))
        }
    }
}

