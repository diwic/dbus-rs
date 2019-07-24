use dbus::channel::{Channel, BusType, Watch};
use dbus::nonblock::Connection;
use dbus::Error;

use std::{future, task, pin};
use std::sync::Arc;

use tokio_reactor::Registration;

/// The I/O Resource should be spawned onto a Tokio compatible reactor.
///
/// If you need to ever cancel this resource (i e disconnect from D-Bus),
/// you need to make this future abortable. If it finishes, you probably lost
/// contact with the D-Bus server.
pub struct IOResource {
    connection: Arc<Connection>,
    watches: Vec<(Watch, Registration)>,
}

impl IOResource {
    fn poll_internal(&self) -> Result<(), Box<std::error::Error + Send + Sync>> {
        for (w, r) in &self.watches { 
            r.register(&mio::unix::EventedFd(&w.fd))?;
            if w.read { r.poll_read_ready()?; };
            if w.write { r.poll_write_ready()?; };
        }
        self.connection.read_write()?;
        self.connection.dispatch_all();
        Ok(())
    }
}

impl future::Future for IOResource {
    fn poll(self: pin::Pin<&mut Self>, _ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.poll_internal() {
            Ok(_) => task::Poll::Pending,
            Err(e) => task::Poll::Ready(e),
        }
    }
    type Output = Box<std::error::Error + Send + Sync>;
}


pub fn new_session() -> Result<(IOResource, Arc<Connection>), Error> {
    let mut channel = Channel::get_private(BusType::Session)?;
    let watches = channel.watch_fds().unwrap(); // This is a just created channel, so this should never fail
    let watches = watches.into_iter().map(|w| (w, Registration::new())).collect();

    let conn = Arc::new(Connection::from(channel));
    let res = IOResource { connection: conn.clone(), watches };
    Ok((res, conn))
}


#[test]
fn method_call() {
    use fut03::future::{FutureExt, TryFutureExt, ready};
    use tokio::runtime::Runtime;

    let mut rt = Runtime::new().unwrap();

    let (res, conn) = new_session().unwrap();

    let res = res.then(|e| { panic!(e); ready(()) }).unit_error().boxed().compat();
    rt.spawn(res);

    let proxy = dbus::nonblock::Proxy::new("org.freedesktop.DBus", "/", conn);
    let fut = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",));
    let fut = fut.boxed().compat();
    let (has_owner,): (bool,) = rt.block_on(fut).unwrap();

    assert_eq!(has_owner, false);
}
