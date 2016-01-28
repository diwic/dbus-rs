use ffi;
use libc;
use super::Connection;

use std::mem;
use std::cell::RefCell;
use std::os::unix::io::{RawFd, AsRawFd};

/// A file descriptor to watch for incoming events (for async I/O)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Watch {
    fd: RawFd,
    read: bool,
    write: bool,
}

impl Watch {
    /// Get the RawFd this Watch is for
    pub fn fd(&self) -> RawFd { self.fd }
    /// Add POLLIN to events to listen for
    pub fn readable(&self) -> bool { self.read }
    /// Add POLLOUT to events to listen for
    pub fn writable(&self) -> bool { self.write }
}

impl AsRawFd for Watch {
    fn as_raw_fd(&self) -> RawFd { self.fd }
}

/// Note - internal struct, not to be used outside API. Moving it outside its box will break things.
pub struct WatchList {
    watches: RefCell<Vec<*mut ffi::DBusWatch>>,
    enabled_fds: RefCell<Vec<Watch>>,
    on_update: Box<Fn(Watch)>
}

impl WatchList {
    pub fn new(c: &Connection, on_update: Box<Fn(Watch)>) -> Box<WatchList> {
        let w = Box::new(WatchList { on_update: on_update, watches: RefCell::new(vec!()), enabled_fds: RefCell::new(vec!()) });
        if unsafe { ffi::dbus_connection_set_watch_functions(c.i.conn.get(),
            Some(add_watch_cb), Some(remove_watch_cb), Some(toggled_watch_cb), &*w as *const _ as *mut _, None) } == 0 {
            panic!("dbus_connection_set_watch_functions failed");
        }
        w
    }

    pub fn watch_handle(&self, fd: RawFd, flags: libc::c_uint) {
        // println!("watch_handle {} flags {}", fd, flags);
        for &q in self.watches.borrow().iter() {
            let w = self.get_watch(q);
            if w.fd != fd { continue };
            if unsafe { ffi::dbus_watch_handle(q, flags) } == 0 {
                panic!("dbus_watch_handle failed");
            }
            self.update(q);
        };
    }

    pub fn get_enabled_fds(&self) -> Vec<Watch> {
        self.enabled_fds.borrow().clone()
    }

    fn get_watch(&self, watch: *mut ffi::DBusWatch) -> Watch {
        let mut w = Watch { fd: unsafe { ffi::dbus_watch_get_unix_fd(watch) }, read: false, write: false};
        let enabled = self.watches.borrow().contains(&watch) && unsafe { ffi::dbus_watch_get_enabled(watch) != 0 };
        let flags = unsafe { ffi::dbus_watch_get_flags(watch) };
        if enabled {
            w.read = (flags & ffi::DBusWatchEvent::Readable as libc::c_uint) != 0;
            w.write = (flags & ffi::DBusWatchEvent::Writable as libc::c_uint) != 0;
        }
        // println!("Get watch fd {:?} ptr {:?} enabled {:?} flags {:?}", w, watch, enabled, flags);
        w
    }

    fn update(&self, watch: *mut ffi::DBusWatch) {
        let mut w = self.get_watch(watch);

        for &q in self.watches.borrow().iter() {
            if q == watch { continue };
            let ww = self.get_watch(q);
            if ww.fd != w.fd { continue };
            w.read |= ww.read;
            w.write |= ww.write;
        }
        // println!("Updated sum: {:?}", w);

        {
            let mut fdarr = self.enabled_fds.borrow_mut();

            if w.write || w.read {
                if fdarr.contains(&w) { return; } // Nothing changed
            }
            else if !fdarr.iter().any(|q| w.fd == q.fd) { return; } // Nothing changed

            fdarr.retain(|f| f.fd != w.fd);
            if w.write || w.read { fdarr.push(w) };
        }
        (*self.on_update)(w);
    }
}

extern "C" fn add_watch_cb(watch: *mut ffi::DBusWatch, data: *mut libc::c_void) -> u32 {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Add watch {:?}", watch);
    wlist.watches.borrow_mut().push(watch);
    wlist.update(watch);
    1
}

extern "C" fn remove_watch_cb(watch: *mut ffi::DBusWatch, data: *mut libc::c_void) {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Removed watch {:?}", watch);
    wlist.watches.borrow_mut().retain(|w| *w != watch);
    wlist.update(watch);
}

extern "C" fn toggled_watch_cb(watch: *mut ffi::DBusWatch, data: *mut libc::c_void) {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Toggled watch {:?}", watch);
    wlist.update(watch);
}

#[cfg(test)]
mod test {
    use libc;
    use super::super::{Connection, Message, BusType, WatchEvent, ConnectionItem, MessageType};

    #[repr(C)]
    #[derive(Clone, Debug)]
    pub struct PollFd {
        fd: libc::c_int,
        events: libc::c_short,
        revents: libc::c_short,
    }

    const POLLIN: libc::c_short = 0x001;
    const POLLOUT: libc::c_short = 0x004;
    const POLLERR: libc::c_short = 0x008;
    const POLLHUP: libc::c_short = 0x010;

    extern "C" { pub fn poll(fds: *mut PollFd, nfds: libc::c_ulong, timeout: libc::c_int) -> libc::c_int; }

    fn build_pollfds(c: &Connection) -> Vec<PollFd> {
        c.watch_fds().iter().map(|w|
             PollFd {
                 fd: w.fd(),
                 events: POLLERR + POLLHUP + if w.readable() { POLLIN } else { 0 } + if w.writable() { POLLOUT } else { 0 },
                 revents: 0
             }).collect()
    }

    #[test]
    fn async() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/test").unwrap();
        let m = Message::new_method_call(&c.unique_name(), "/test", "com.example.asynctest", "AsyncTest").unwrap();
        let serial = c.send(m).unwrap();
        println!("Async: sent serial {}", serial);

        let mut fds: Vec<PollFd> = build_pollfds(&c);
        let mut new_fds = None;
        let mut i = 0;
        let mut success = false;
        while !success {
            i += 1;
            if let Some(q) = new_fds { fds = q; new_fds = None };

            for f in fds.iter_mut() { f.revents = 0 };
            assert!(unsafe { poll(fds.as_mut_ptr(), fds.len() as libc::c_ulong, 1000) } > 0);

            for f in fds.iter().filter(|pfd| pfd.revents != 0) {
                let m =
                    if (f.revents & POLLIN) != 0 { WatchEvent::Readable as libc::c_uint } else { 0 } +
                    if (f.revents & POLLOUT) != 0 { WatchEvent::Writable as libc::c_uint } else { 0 } +
                    if (f.revents & POLLERR) != 0 { WatchEvent::Error as libc::c_uint } else { 0 } +
                    if (f.revents & POLLHUP) != 0 { WatchEvent::Hangup as libc::c_uint } else { 0 };
                println!("Async: fd {}, revents {} -> {}", f.fd, f.revents, m);
                assert!(f.revents & POLLIN != 0 || f.revents & POLLOUT != 0);

                for e in c.watch_handle(f.fd, m) {
                    println!("Async: got {:?}", e);
                    match e {
                        ConnectionItem::WatchFd(_) => new_fds = Some(build_pollfds(&c)),
                        ConnectionItem::MethodCall(m) => {
                            assert_eq!(m.headers(), (MessageType::MethodCall, Some("/test".to_string()),
                                Some("com.example.asynctest".into()), Some("AsyncTest".to_string())));
                            let mut mr = Message::new_method_return(&m).unwrap();
                            mr.append_items(&["Goodies".into()]);
                            c.send(mr).unwrap();
                        }
                        ConnectionItem::MethodReturn(m) => {
                            assert_eq!(m.headers().0, MessageType::MethodReturn);
                            assert_eq!(m.get_reply_serial().unwrap(), serial);
                            let i = m.get_items();
                            let s: &str = i[0].inner().unwrap();
                            assert_eq!(s, "Goodies");
                            success = true;
                        }
                        _ => (),
                    }
                }
                if i > 100 { panic!() };
            }
        }
    }
}
