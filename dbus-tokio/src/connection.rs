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

        c.read_write(Some(Default::default())).map_err(|_| Error::new_failed("Read/write failed"))?;
        self.connection.process_all();

        let w = c.watch();

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

        r.take_read_ready()?;
        r.take_write_ready()?;

        if w.read { let _ = r.poll_read_ready(ctx)?; };
        if w.write { let _ = r.poll_write_ready(ctx)?; };

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
