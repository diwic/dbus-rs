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

/// The I/O Resource should be spawned onto a Tokio compatible reactor.
///
/// If you need to ever cancel this resource (i e disconnect from D-Bus),
/// you need to make this future abortable. If it finishes, you probably lost
/// contact with the D-Bus server.
pub struct IOResource<C> {
    connection: Arc<C>,
    registration: Option<(Registration, std::os::unix::io::RawFd)>,
}

impl<C: AsRef<Channel> + Process> IOResource<C> {
    fn poll_internal(&mut self, ctx: &mut task::Context<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let c: &Channel = (*self.connection).as_ref();

        let w = loop {
            c.read_write(Some(Default::default())).map_err(|_| Error::new_failed("Read/write failed"))?;
            self.connection.process_all();
            let w = c.watch();
            if !w.read { break w; }

            // Because libdbus is level-triggered and tokio is edge-triggered, we need to do read again
            // in case libdbus did not read all available data. Maybe there is a better way to see if there
            // is more incoming data than calling libc::recv?
            // https://github.com/diwic/dbus-rs/issues/254
            let mut x = 0u8;
            let r = unsafe {
                libc::recv(w.fd, &mut x as *mut _ as *mut libc::c_void, 1, libc::MSG_DONTWAIT + libc::MSG_PEEK)
            };
            if r != 1 { break w; }
        };

        let r = match &self.registration {
            None => {
                let reg = Registration::new(&mio::unix::EventedFd(&w.fd))?;
                self.registration = Some((reg, w.fd));
                &self.registration.as_ref().unwrap().0
            }
            Some((reg, fd)) => {
                assert_eq!(*fd, w.fd);
                reg
            },
        };

        // I'm not sure exactly why, but it seems to be best to poll both read and write regardless of
        // read/write state of the watch. See https://github.com/diwic/dbus-rs/issues/233
        let _ = r.poll_read_ready(ctx)?;
        let _ = r.poll_write_ready(ctx)?;

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

    let mut conn = C::from(channel);
    conn.set_timeout_maker(Some(make_timeout));

    let conn = Arc::new(conn);
    let res = IOResource { connection: conn.clone(), registration: None };
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

    let mut server_conn = dbus::blocking::SyncConnection::new_session()?;

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
