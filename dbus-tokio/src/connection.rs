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
use dbus::Error;

use std::{future, task, pin};
use std::sync::Arc;
use std::time::Instant;

use tokio::io::Registration;

struct IOResourceUnregistered {
    watch_fd: std::os::unix::io::RawFd,
    waker_resource: mio::Registration,
}

struct IOResourceRegistered {
    watch_fd: std::os::unix::io::RawFd,
    watch_reg: Registration,
    waker_reg: Registration,
}

enum IOResourceRegistration {
    Unregistered(IOResourceUnregistered),
    Registered(IOResourceRegistered),
}

/// The I/O Resource should be spawned onto a Tokio compatible reactor.
///
/// If you need to ever cancel this resource (i e disconnect from D-Bus),
/// you need to make this future abortable. If it finishes, you probably lost
/// contact with the D-Bus server.
pub struct IOResource<C> {
    connection: Arc<C>,
    registration: IOResourceRegistration,
    write_pending: bool,
}

fn is_poll_ready(i: task::Poll<mio::Ready>) -> bool {
    match i {
        task::Poll::Ready(r) => !r.is_empty(),
        task::Poll::Pending => false,
    }
}

impl<C: AsRef<Channel> + Process> IOResource<C> {
    fn poll_internal(&mut self, ctx: &mut task::Context<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let c: &Channel = (*self.connection).as_ref();

        // Register all wake-up events in reactor
        let IOResourceRegistered{watch_fd, watch_reg, waker_reg} = match &self.registration {
            IOResourceRegistration::Unregistered(IOResourceUnregistered{watch_fd, waker_resource}) => {
                let watch_fd = *watch_fd;
                let watch_reg = Registration::new(&mio::unix::EventedFd(&watch_fd))?;
                let waker_reg = Registration::new(waker_resource)?;
                self.registration = IOResourceRegistration::Registered(IOResourceRegistered{watch_fd, watch_reg, waker_reg});
                match &self.registration {
                    IOResourceRegistration::Registered(res) => res,
                    _ => unreachable!(),
                }
            },
            IOResourceRegistration::Registered(res) => res,
        };

        // Make a promise that we process all events recieved before this moment
        // If new event arrives after calls to poll_*_ready, tokio will wake us up.
        let read_ready = is_poll_ready(watch_reg.poll_read_ready(ctx)?);
        let send_ready = is_poll_ready(waker_reg.poll_read_ready(ctx)?);
        // If we were woken up by write ready - reset it
        let write_ready = watch_reg.take_write_ready()?.map(|r| r.is_writable()).unwrap_or(false);

        if read_ready || send_ready || (self.write_pending && write_ready) {
            loop {
                self.write_pending = false;
                c.read_write(Some(Default::default())).map_err(|_| Error::new_failed("Read/write failed"))?;
                self.connection.process_all();

                if c.has_messages_to_send() {
                    self.write_pending = true;
                    // DBus has unsent messages
                    // Assume it's because a write to fd would block
                    // Ask tokio to notify us when fd will became writable
                    if is_poll_ready(watch_reg.poll_write_ready(ctx)?) {
                        // try again immediately
                        continue
                    }
                }

                // Because libdbus is level-triggered and tokio is edge-triggered, we need to do read again
                // in case libdbus did not read all available data. Maybe there is a better way to see if there
                // is more incoming data than calling libc::recv?
                // https://github.com/diwic/dbus-rs/issues/254
                let mut x = 0u8;
                let r = unsafe {
                    libc::recv(*watch_fd, &mut x as *mut _ as *mut libc::c_void, 1, libc::MSG_DONTWAIT | libc::MSG_PEEK)
                };
                if r != 1 { break; }
            }
        }

        Ok(())
    }
}

impl<C: AsRef<Channel> + Process> future::Future for IOResource<C> {
    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.poll_internal(ctx) {
            Ok(_) => {
                task::Poll::Pending
            },
            Err(e) => task::Poll::Ready(e),
        }
    }
    type Output = Box<dyn std::error::Error + Send + Sync>;
}

fn make_timeout(timeout: Instant) -> pin::Pin<Box<dyn future::Future<Output=()> + Send + Sync + 'static>> {
    let t = tokio::time::delay_until(timeout.into());
    Box::pin(t)
}

/// Generic connection creator, you might want to use e g `new_session_local`, `new_system_sync` etc for convenience.
pub fn new<C: From<Channel> + NonblockReply>(b: BusType) -> Result<(IOResource<C>, Arc<C>), Error> {
    let mut channel = Channel::get_private(b)?;
    channel.set_watch_enabled(true);
    let watch = channel.watch();
    let watch_fd = watch.fd;

    let mut conn = C::from(channel);
    conn.set_timeout_maker(Some(make_timeout));

    // When we send async messages from other tasks we must wake up this one to do the flush
    let (waker_resource, waker) = mio::Registration::new2();
    conn.set_waker({ Some(Box::new(
        move || waker.set_readiness(mio::Ready::readable()).map_err(|_| ())
    ))});

    let conn = Arc::new(conn);

    let res = IOResource {
        connection: conn.clone(),
        registration: IOResourceRegistration::Unregistered(IOResourceUnregistered{watch_fd, waker_resource}),
        write_pending: false,
    };
    Ok((res, conn))
}

/// Creates a connection to the session bus, to use with Tokio's basic (single-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_session_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::Session) }

/// Creates a connection to the system bus, to use with Tokio's basic (single-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_system_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::System) }

/// Creates a connection to the session bus, to use with Tokio's default (multi-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_session_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::Session) }

/// Creates a connection to the system bus, to use with Tokio's default (multi-thread) scheduler.
///
/// Note: This function blocks until the connection is set up.
pub fn new_system_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::System) }

/* Let's skip these for now, not sure if they are useful?
pub fn new_session() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::Session) }
pub fn new_system() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::System) }
*/

#[test]
fn method_call_local() {
    use tokio::task;
    use std::time::Duration;

    let mut rt = tokio::runtime::Builder::new()
        .basic_scheduler()
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
    use futures::StreamExt;
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
    let f = dbus::tree::Factory::new_sync::<()>();
    let tree =
        f.tree(()).add(f.object_path("/", ()).add(f.interface("com.example.dbusrs.tokiobigtest", ()).add_m(f.method("Ping", (), |m| {
            // println!("received ping!");
            Ok(vec![m.msg.method_return().append1(make_big_reply().map_err(|err| dbus::tree::MethodErr::failed(&err))?)])
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

    let client_interval = tokio::time::interval(Duration::from_millis(10));
    let proxy = dbus::nonblock::Proxy::new("com.example.dbusrs.tokiobigtest", "/", Duration::from_secs(1), client_conn);
    client_interval
        .take(10)
        .for_each(|_| async {
            println!("sending ping");
            proxy.method_call::<(BigProps,), _, _, _>("com.example.dbusrs.tokiobigtest", "Ping", ()).await.unwrap();
            println!("received prop list!");
        })
        .await;

    done.store(true, Ordering::Release);

    Ok(())
}
