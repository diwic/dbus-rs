use dbus::channel::{Channel, BusType};
use dbus::nonblock::{LocalConnection, SyncConnection, Process};
use dbus::Error;

use std::{future, task, pin};
use std::sync::Arc;


#[cfg(feature = "nightly")]
use tokio02net::driver::Registration;
#[cfg(not(feature = "nightly"))]
use tokio_reactor::Registration;

/// The I/O Resource should be spawned onto a Tokio compatible reactor.
///
/// If you need to ever cancel this resource (i e disconnect from D-Bus),
/// you need to make this future abortable. If it finishes, you probably lost
/// contact with the D-Bus server.
pub struct IOResource<C> {
    connection: Arc<C>,
    registration: Registration,
}

impl<C: AsRef<Channel> + Process> IOResource<C> {
    fn poll_internal(&self, _ctx: &mut task::Context<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let c: &Channel = (*self.connection).as_ref();
        let w = c.watch();
        let r = &self.registration;
        r.register(&mio::unix::EventedFd(&w.fd))?;
        r.take_read_ready()?;
        r.take_write_ready()?;

        #[cfg(feature = "nightly")]
        {
            if w.read { let _ = r.poll_read_ready(_ctx)?; };
            if w.write { let _ = r.poll_write_ready(_ctx)?; };
        }

        #[cfg(not(feature = "nightly"))]
        {
            if w.read { r.poll_read_ready()?; };
            if w.write { r.poll_write_ready()?; };
        }

        c.read_write(Some(Default::default())).map_err(|_| Error::new_failed("Read/write failed"))?;
        self.connection.process_all();
        Ok(())
    }
}

impl<C: AsRef<Channel> + Process> future::Future for IOResource<C> {
    fn poll(self: pin::Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.poll_internal(ctx) {
            Ok(_) => {
                task::Poll::Pending
            },
            Err(e) => task::Poll::Ready(e),
        }
    }
    type Output = Box<dyn std::error::Error + Send + Sync>;
}


/// Generic connection creator, you might want to use e g `new_session_local`, `new_system_sync` etc for convenience. 
pub fn new<C: From<Channel>>(b: BusType) -> Result<(IOResource<C>, Arc<C>), Error> {
    let mut channel = Channel::get_private(b)?;
    channel.set_watch_enabled(true);

    let conn = Arc::new(C::from(channel));
    let res = IOResource { connection: conn.clone(), registration: Registration::new() };
    Ok((res, conn))
}

pub fn new_session_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::Session) }
pub fn new_system_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::System) }
pub fn new_session_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::Session) }
pub fn new_system_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::System) }

#[test]
fn method_call() {
    use fut03::future::{FutureExt, TryFutureExt, ready};
    use tokio::runtime::current_thread::Runtime;

    let mut rt = Runtime::new().unwrap();

    let (res, conn) = new_session_local().unwrap();

    #[allow(unreachable_code)] // Easier than trying to figure a good return type for the closure
    let res = res.then(|e| { panic!(e); ready(()) }).unit_error().boxed_local().compat();
    rt.spawn(res);

    let proxy = dbus::nonblock::Proxy::new("org.freedesktop.DBus", "/", conn);
    let fut = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",));
    let fut = fut.boxed().compat();
    let (has_owner,): (bool,) = rt.block_on(fut).unwrap();

    assert_eq!(has_owner, false);
}
