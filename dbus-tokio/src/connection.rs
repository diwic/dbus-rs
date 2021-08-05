//! Contains connection components.
//!
//! # Example
//!
//! ```
//! use dbus_tokio::connection;
//! use dbus::nonblock::Proxy;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//!     // Connect to the D-Bus session bus (this is blocking, unfortunately).
//!     let (resource, conn) = connection::new_session_sync()?;
//!
//!     // The resource is a task that should be spawned onto a tokio compatible
//!     // reactor ASAP. If the resource ever finishes, you lost connection to D-Bus.
//!     tokio::spawn(async {
//!         let err = resource.await;
//!         panic!("Lost connection to D-Bus: {}", err);
//!     });
//!
//!     // Make a "proxy object" that contains the destination and path of our method call.
//!     let proxy = Proxy::new("org.freedesktop.DBus", "/", Duration::from_secs(5), conn);
//!
//!     // Call the method and await a response. See the argument guide for details about
//!     // how to send and receive arguments to the method.
//!     let (names,): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ()).await?;
//!
//!     // Print all the names.
//!     for name in names { println!("{}", name); }
//!
//!     Ok(())
//! }
//! ```


use dbus::channel::{Channel, BusType};
use dbus::nonblock::{LocalConnection, SyncConnection, Process, NonblockReply};

use std::{future, io, task, pin};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::os::unix::io::RawFd;

use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};

#[derive(Debug)]
enum WakeStatus {
    /// The resource task has not yet been polled; ready is false if it has never been polled
    /// before, otherwise, it's being woken as a result of the Channel calling the waker callback
    Waiting { ready: bool },
    /// The resource task is ready to be woken again
    Polled { waker: task::Waker },
}

enum IOResourceRegistration {
    Unregistered(RawFd, tokio::io::Interest),
    Registered(AsyncFd<RawFd>),
}

/// The I/O Resource should be spawned onto a Tokio compatible reactor.
///
/// If you need to ever cancel this resource (i e disconnect from D-Bus),
/// you need to make this future abortable. If it finishes, you probably lost
/// contact with the D-Bus server.
pub struct IOResource<C> {
    connection: Arc<C>,
    registration: IOResourceRegistration,
    wake: Arc<Mutex<WakeStatus>>,
    write_pending: bool,
}

/// An error that can occur in the dbus-tokio reactor
#[derive(Debug)]
#[non_exhaustive]
pub enum IOResourceError {
    /// An error that occurred while interacting with dbus
    Dbus(dbus::Error),
    /// An error that likely occurred on tokio's side
    Io(io::Error),
}

impl From<dbus::Error> for IOResourceError {
    fn from(e: dbus::Error) -> Self {
        IOResourceError::Dbus(e)
    }
}
impl From<io::Error> for IOResourceError {
    fn from(e: io::Error) -> Self {
        IOResourceError::Io(e)
    }
}

impl std::fmt::Display for IOResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IOResourceError::Dbus(e) => e.fmt(f),
            IOResourceError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for IOResourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            IOResourceError::Dbus(e) => e,
            IOResourceError::Io(e) => e,
        })
    }
}

impl<C: AsRef<Channel> + Process> IOResource<C> {
    fn poll_internal(&mut self, ctx: &mut task::Context<'_>) -> Result<(), IOResourceError> {
        let c: &Channel = (*self.connection).as_ref();

        let mut wake_status = self.wake.lock().unwrap();

        // Register all wake-up events in reactor
        if let IOResourceRegistration::Unregistered(watch_fd, interest) = self.registration {
            let watch_reg = AsyncFd::with_interest(watch_fd, interest)?;
            self.registration = IOResourceRegistration::Registered(watch_reg);
        }
        let watch_reg = match &self.registration {
            IOResourceRegistration::Registered(res) => res,
            IOResourceRegistration::Unregistered(..) => unreachable!(),
        };

        // Make a promise that we process all events recieved before this moment
        // If new event arrives after calls to poll_*_ready, tokio will wake us up.
        let mut read_guard = watch_reg.poll_read_ready(ctx)?;

        let send_ready = match &*wake_status {
            // we don't need to clone+update the waker if it's the same one
            WakeStatus::Polled { waker } if ctx.waker().will_wake(waker) => false,
            _ => {
                let prev_status = std::mem::replace(
                    &mut *wake_status,
                    WakeStatus::Polled { waker: ctx.waker().clone() },
                );
                matches!(prev_status, WakeStatus::Waiting { ready: true })
            }
        };

        // If we were woken up by write ready - reset it
        let mut write_guard = watch_reg.poll_write_ready(ctx)?;

        if read_guard.is_ready() || send_ready || (self.write_pending && write_guard.is_ready()) {
            loop {
                self.write_pending = false;
                c.read_write(Some(Duration::default())).map_err(|_| dbus::Error::new_failed("Read/write failed"))?;
                self.connection.process_all();

                if c.has_messages_to_send() {
                    self.write_pending = true;
                    // DBus has unsent messages
                    // Assume it's because a write to fd would block
                    if check_ready_now(&mut write_guard, || watch_reg.poll_write_ready(ctx))? {
                        continue
                    }
                }

                // Because libdbus is level-triggered and tokio is edge-triggered, we need to do read again
                // in case libdbus did not read all available data. Maybe there is a better way to see if there
                // is more incoming data than calling libc::recv?
                // https://github.com/diwic/dbus-rs/issues/254
                let watch_fd = *watch_reg.get_ref();
                let mut x = 0u8;
                let r = unsafe {
                    libc::recv(watch_fd, &mut x as *mut _ as *mut libc::c_void, 1, libc::MSG_DONTWAIT | libc::MSG_PEEK)
                };
                if r != 1 {
                    if check_ready_now(&mut read_guard, || watch_reg.poll_read_ready(ctx))? {
                        continue
                    }
                    // all done, reading would block so we stop polling and yield back to the runtime
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Call when a read/write would block. Returns Ok(true) if we're ready to go again right now
fn check_ready_now<'a>(
    guard: &mut task::Poll<AsyncFdReadyGuard<'a, RawFd>>,
    poll_ready: impl FnOnce() -> task::Poll<std::io::Result<AsyncFdReadyGuard<'a, RawFd>>>,
) -> std::io::Result<bool> {
    if let task::Poll::Ready(g) = guard {
        g.clear_ready();
    }
    // Ask tokio to notify us when fd will became ready
    let ready_now = poll_ready()?;
    // if we're ready right now, we should loop and try again
    let try_again = ready_now.is_ready();
    *guard = ready_now;
    Ok(try_again)
}

impl<C: AsRef<Channel> + Process> future::Future for IOResource<C> {
    type Output = IOResourceError;
    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.poll_internal(ctx) {
            Ok(()) => task::Poll::Pending,
            Err(e) => task::Poll::Ready(e),
        }
    }
}

fn make_timeout(timeout: Instant) -> pin::Pin<Box<dyn future::Future<Output=()> + Send + Sync + 'static>> {
    let t = tokio::time::sleep_until(timeout.into());
    Box::pin(t)
}

/// Create a connection from channel, you may need to invoke `channel.register()?` to make sure the
/// channel is usable.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
///
/// use dbus::channel::Channel;
/// use dbus_tokio::connection;
/// use dbus_tokio::connection::IOResource;
/// use dbus::nonblock::SyncConnection;
///
/// # tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {
/// let mut channel = Channel::open_private("unix:path=/run/user/1000/bus").expect("open private channel failed");
///
/// channel.register().expect("register channel failed");
///
/// let (resource, conn): (IOResource<SyncConnection>, Arc<SyncConnection>) = connection::from_channel(channel).expect("create connection failed");
///
/// tokio::spawn(resource);
/// # })
///
/// // do anything with the conn
/// ```
pub fn from_channel<C: From<Channel> + NonblockReply>(mut channel: Channel) -> Result<(IOResource<C>, Arc<C>), dbus::Error> {
    channel.set_watch_enabled(true);
    let watch = channel.watch();
    let watch_fd = watch.fd;
    // TODO: figure out if there's a reason in tokio that there can't be an Interest::NONE
    let mut interest = tokio::io::Interest::READABLE;
    if watch.write {
        interest |= tokio::io::Interest::WRITABLE;
    }

    let mut conn = C::from(channel);
    conn.set_timeout_maker(Some(make_timeout));

    // When we send async messages from other tasks we must wake up this one to do the flush
    let wake = Arc::new(Mutex::new(WakeStatus::Waiting { ready: false }));
    conn.set_waker(Some(Box::new({
        let wake = wake.clone();
        move || {
            let mut wake_status = wake.lock().unwrap();
            let prev_status = std::mem::replace(
                &mut *wake_status,
                WakeStatus::Waiting { ready: true }
            );
            match prev_status {
                WakeStatus::Polled { waker } => {
                    waker.wake();
                    Ok(())
                }
                WakeStatus::Waiting { .. } => {
                    Err(())
                }
            }
        }
    })));

    let conn = Arc::new(conn);

    let res = IOResource {
        connection: conn.clone(),
        registration: IOResourceRegistration::Unregistered(watch_fd, interest),
        wake,
        write_pending: false,
    };
    Ok((res, conn))
}

/// Generic connection creator, you might want to use e g `new_session_local`, `new_system_sync` etc for convenience.
pub fn new<C: From<Channel> + NonblockReply>(b: BusType) -> Result<(IOResource<C>, Arc<C>), dbus::Error> {
    let channel = Channel::get_private(b)?;

    from_channel(channel)
}

/// Creates a connection to the session bus, to use with Tokio's basic (single-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_session_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), dbus::Error> { new(BusType::Session) }

/// Creates a connection to the system bus, to use with Tokio's basic (single-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_system_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), dbus::Error> { new(BusType::System) }

/// Creates a connection to the session bus, to use with Tokio's default (multi-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_session_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), dbus::Error> { new(BusType::Session) }

/// Creates a connection to the system bus, to use with Tokio's default (multi-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_system_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), dbus::Error> { new(BusType::System) }

/* Let's skip these for now, not sure if they are useful?
pub fn new_session() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::Session) }
pub fn new_system() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::System) }
*/

#[cfg(test)]
mod test {

use super::*;

#[test]
fn method_call_local() {
    use tokio::task;
    use std::time::Duration;

    let mut rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let local = task::LocalSet::new();

    let (res, conn) = new_session_local().unwrap();
    local.spawn_local(async move { panic!(res.await);});

    let proxy = dbus::nonblock::Proxy::new("org.freedesktop.DBus", "/", Duration::from_secs(2), conn);
    let fut = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",));

    let (has_owner,): (bool,) = local.block_on(&mut rt, fut).unwrap();

    assert_eq!(has_owner, false);
}

#[tokio::test]
async fn timeout() {
    use std::time::Duration;

    let (ress, conns) = new_session_sync().unwrap();
    tokio::spawn(async move { panic!(ress.await);});
    conns.request_name("com.example.dbusrs.tokiotest", true, true, true).await.unwrap();
    use dbus::channel::MatchingReceiver;
    conns.start_receive(dbus::message::MatchRule::new_method_call(), Box::new(|_,_| true));

    let (res, conn) = new_session_sync().unwrap();
    tokio::spawn(async move { panic!(res.await);});
    let proxy = dbus::nonblock::Proxy::new("com.example.dbusrs.tokiotest", "/", Duration::from_millis(150), conn);
    let e: Result<(), _> = proxy.method_call("com.example.dbusrs.tokiotest", "Whatever", ()).await;
    let e = e.unwrap_err();
    assert_eq!(e.name(), Some("org.freedesktop.DBus.Error.Timeout"));
}

#[tokio::test]
async fn large_message() -> Result<(), Box<dyn std::error::Error>> {
    use dbus::arg::Variant;
    use dbus_tree::Factory;
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };

    type BigProps<'a> = Vec<(dbus::Path<'a>, HashMap<String, Variant<Box<i32>>>)>;

    // Simulate a big property list that something like connman would return.
    fn make_big_reply<'a>() -> Result<BigProps<'a>, String> {
        let prop_map: HashMap<String, Variant<Box<i32>>> = (0..500).map(|i| (format!("key {}", i), Variant(Box::new(i)))).collect();
        (0..30u8).map(|i| Ok((dbus::strings::Path::new(format!("/{}", i))?, prop_map.clone()))).collect()
    }

    let server_conn = dbus::blocking::SyncConnection::new_session()?;

    server_conn.request_name("com.example.dbusrs.tokiobigtest", false, true, false)?;
    let f = Factory::new_sync::<()>();
    let tree =
        f.tree(()).add(f.object_path("/", ()).add(f.interface("com.example.dbusrs.tokiobigtest", ()).add_m(f.method("Ping", (), |m| {
            // println!("received ping!");
            Ok(vec![m.msg.method_return().append1(make_big_reply().map_err(|err| dbus::MethodErr::failed(&err))?)])
        }))));
    tree.start_receive_sync(&server_conn);

    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();
    tokio::task::spawn_blocking(move || {
        while !done2.load(Ordering::Acquire) {
            server_conn.process(Duration::from_millis(100)).unwrap();
        }
    });

    let (resource, client_conn) = new_session_sync()?;
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    let mut client_interval = tokio::time::interval(Duration::from_millis(10));
    let proxy = dbus::nonblock::Proxy::new("com.example.dbusrs.tokiobigtest", "/", Duration::from_secs(1), client_conn);
    for _ in 0..10 {
        client_interval.tick().await;
        println!("sending ping");
        proxy.method_call::<(BigProps,), _, _, _>("com.example.dbusrs.tokiobigtest", "Ping", ()).await.unwrap();
        println!("received prop list!");
    }

    done.store(true, Ordering::Release);

    Ok(())
}

}
