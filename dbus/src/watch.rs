use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Mutex, atomic::AtomicU8, atomic::Ordering};
use std::os::raw::c_void;

/// Platform-specific file descriptor type
#[cfg(unix)]
pub type WatchFd = std::os::unix::io::RawFd;

/// Platform-specific file descriptor type
#[cfg(windows)]
pub type WatchFd = std::os::windows::io::RawSocket;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A file descriptor, and an indication whether it should be read from, written to, or both.
pub struct Watch {
    /// File descriptor
    pub fd: WatchFd,
    /// True if wakeup should happen when the file descriptor is ready for reading
    pub read: bool,
    /// True if wakeup should happen when the file descriptor is ready for writing
    pub write: bool,
}

impl Watch {
    pub unsafe fn from_raw_enabled(watch: *mut ffi::DBusWatch) -> (Self, bool) {
        #[cfg(unix)]
        let mut w = Watch {fd: ffi::dbus_watch_get_unix_fd(watch), read: false, write: false};
        #[cfg(windows)]
        let mut w = Watch {fd: ffi::dbus_watch_get_socket(watch) as WatchFd, read: false, write: false};
        let enabled = ffi::dbus_watch_get_enabled(watch) != 0;
        let flags = ffi::dbus_watch_get_flags(watch);
        use std::os::raw::c_uint;
        w.read = (flags & ffi::DBUS_WATCH_READABLE as c_uint) != 0;
        w.write = (flags & ffi::DBUS_WATCH_WRITABLE as c_uint) != 0;
        (w, enabled)
    }
}

pub trait SetWatchFunctions {
    unsafe fn set_watch_functions(&self, add_function: ffi::DBusAddWatchFunction,
        remove_function: ffi::DBusRemoveWatchFunction, toggled_function: ffi::DBusWatchToggledFunction,
        data: *mut c_void, free_data_function: ffi::DBusFreeFunction) -> u32;
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct WatchHandle(*mut ffi::DBusWatch);

unsafe impl Send for WatchHandle {}
unsafe impl Sync for WatchHandle {}

/// This struct must be boxed as it is called from D-Bus callbacks!
#[derive(Debug)]
pub struct WatchMapInner<T: SetWatchFunctions> {
    inner: ManuallyDrop<T>,
    pub(crate) list: Mutex<HashMap<WatchHandle, (Watch, bool)>>,
    current_rw: AtomicU8,
    current_fd: Option<WatchFd>,
}

fn calc_rw(list: &HashMap<WatchHandle, (Watch, bool)>) -> u8 {
    let mut r = 0;
    for (w, b) in list.values() {
        if *b && w.read { r |= 1; }
        if *b && w.write { r |= 2; }
    }
    r
}

impl<T: SetWatchFunctions> WatchMapInner<T> {
    fn new(inner: T) -> Box<Self> {
        extern "C" fn add_watch_cb<T: SetWatchFunctions>(watch: *mut ffi::DBusWatch, data: *mut c_void) -> u32 { unsafe {
            let wm: &WatchMapInner<T> = &*(data as *mut _);
            wm.list.lock().unwrap().insert(WatchHandle(watch), Watch::from_raw_enabled(watch));
            1
        }}
        extern "C" fn remove_watch_cb<T: SetWatchFunctions>(watch: *mut ffi::DBusWatch, data: *mut c_void) { unsafe {
            let wm: &WatchMapInner<T> = &*(data as *mut _);
            wm.list.lock().unwrap().remove(&WatchHandle(watch));
        }}
        extern "C" fn toggled_watch_cb<T: SetWatchFunctions>(watch: *mut ffi::DBusWatch, data: *mut c_void) { unsafe {
            let wm: &WatchMapInner<T> = &*(data as *mut _);
            let mut list = wm.list.lock().unwrap();
            let (_, ref mut b) = list.get_mut(&WatchHandle(watch)).unwrap();
            *b = ffi::dbus_watch_get_enabled(watch) != 0;
            wm.current_rw.store(calc_rw(&list), Ordering::Release);
        }}

        let mut wm = Box::new(Self {
            inner: ManuallyDrop::new(inner), list: Default::default(), current_rw: Default::default(), current_fd: None
        });
        let wptr: &Self = &wm;
        if unsafe { wm.inner.set_watch_functions(
            Some(add_watch_cb::<T>), Some(remove_watch_cb::<T>), Some(toggled_watch_cb::<T>),
            wptr as *const _ as *mut _, None) } == 0 {
                panic!("Cannot enable watch tracking (OOM?)")
        }

        {
            let list = wm.list.lock().unwrap();
            wm.current_rw.store(calc_rw(&list), Ordering::Release);

            // This will never panic in practice, see https://lists.freedesktop.org/archives/dbus/2019-July/017786.html
            for (w, _) in list.values() {
                if let Some(ref fd) = &wm.current_fd {
                    assert_eq!(*fd, w.fd);
                } else {
                    wm.current_fd = Some(w.fd);
                }
            }
        }

        wm
    }
}

impl<T: SetWatchFunctions> Drop for WatchMapInner<T> {
    fn drop(&mut self) {
        let wptr: &Self = &self;
        if unsafe { self.inner.set_watch_functions(
            None, None, None, wptr as *const _ as *mut _, None) } == 0 {
                panic!("Cannot disable watch tracking (OOM?)")
        }
    }
}

#[derive(Debug)]
pub struct WithWatchMap<T: Clone + SetWatchFunctions> {
    inner: T,
    pub(crate) watchmap: Option<Box<WatchMapInner<T>>>,
}

impl<T: Clone + SetWatchFunctions> Drop for WithWatchMap<T> {
    fn drop(&mut self) {
        self.set_watch_enabled(false); // Make sure "watchmap" is destroyed before "inner" is
    }
}

impl<T: Clone + SetWatchFunctions> WithWatchMap<T> {
    pub fn new(inner: T) -> Self {
        Self { inner, watchmap: None }
    }

    #[inline(always)]
    pub fn inner(&self) -> &T {
        &self.inner
    }

    #[inline(always)]
    pub fn is_watch_enabled(&self) -> bool {
        self.watchmap.is_some()
    }

    /// Enables watch tracking, a prequisite for calling watch.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn set_watch_enabled(&mut self, enable: bool) {
        if enable == self.is_watch_enabled() { return }
        if enable {
            self.watchmap = Some(WatchMapInner::new(self.inner.clone()));
        } else {
            self.watchmap = None;
        }
    }

    /// Gets the file descriptor to listen for read/write.
    ///
    /// Panics: if set_watch_enabled is false.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn watch(&self) -> Watch {
        let wm = self.watchmap.as_ref().unwrap();
        let rw = wm.current_rw.load(Ordering::Acquire);
        Watch {
            fd: wm.current_fd.unwrap(),
            read: (rw & 1) != 0,
            write: (rw & 2) != 0,
        }
    }
}
