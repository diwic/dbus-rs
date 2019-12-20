use dbus::channel::{Channel, BusType};
use dbus::nonblock::{LocalConnection, SyncConnection, Connection, Process};
use dbus::Error;

use std::{future, task, pin};
use std::sync::Arc;

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

        match &mut self.registration {
            None => {
                let reg = Registration::new(&mio::unix::EventedFd(&w.fd))?;
                self.registration = Some((reg, w.fd));
            }
            Some((_reg, fd)) if *fd == w.fd => (),
            Some((_reg, fd)) => return Err(Box::new(Error::new_failed(
                &format!("Unexpected changing file descriptor for dbus (from {} to {})",  w.fd, fd)
            ))),
        };

        if let Some((r,_)) = &self.registration {
            r.take_read_ready()?;
            r.take_write_ready()?;

            if w.read { let _ = r.poll_read_ready(ctx)?; };
            if w.write { let _ = r.poll_write_ready(ctx)?; };
        } else {
            unreachable!()
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


/// Generic connection creator, you might want to use e g `new_session_local`, `new_system_sync` etc for convenience.
pub fn new< C: From<Channel>>(b: BusType) -> Result<(IOResource< C>, Arc<C>), Error> {
    let mut channel = Channel::get_private(b)?;
    channel.set_watch_enabled(true);

    let conn = Arc::new(C::from(channel));

    let res = IOResource { connection: conn.clone(), registration: None };
    Ok((res, conn))
}

pub fn new_session_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::Session) }
pub fn new_system_local() -> Result<(IOResource<LocalConnection>, Arc<LocalConnection>), Error> { new(BusType::System) }
pub fn new_session_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::Session) }
pub fn new_system_sync() -> Result<(IOResource<SyncConnection>, Arc<SyncConnection>), Error> { new(BusType::System) }
pub fn new_session() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::Session) }
pub fn new_system() -> Result<(IOResource<Connection>, Arc<Connection>), Error> { new(BusType::System) }

#[test]
fn method_call() {
    use tokio::task;
    use futures::future::FutureExt;

    let mut rt = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_io()
        .build()
        .unwrap();

    let local = task::LocalSet::new();

    let (res, conn) = new_session_local().unwrap();
    let spawner = async move {
        task::spawn_local(async move { panic!(res.await);});
    };

    let proxy = dbus::nonblock::Proxy::new("org.freedesktop.DBus", "/", conn);
    let fut = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",));

    let (has_owner,): (bool,) = local.block_on(& mut rt , spawner.then(|_| fut)).unwrap();

    assert_eq!(has_owner, false);
}
