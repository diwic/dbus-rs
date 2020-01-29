use mio;

use tokio_reactor::PollEvented;
use crate::connection::IoHandler;
use crate::Error;

use futures::task;

pub struct Io(Vec<PollEvented<AWatch>>);

impl IoHandler for Io {
    fn new(watches: &[dbus::Watch]) -> Result<Self, Error> where Self: Sized {
        Ok(Io(watches.into_iter().filter_map(|w| {
            if !w.readable() && !w.writable() { return None; }
            let p = PollEvented::new(AWatch(w.clone()));
            if w.readable() { p.clear_read_ready(mio::Ready::readable()).unwrap() };
            if w.writable() { p.clear_write_ready().unwrap() };
            Some(p)
        }).collect()))
    }
    fn after_read_write(&mut self, _: &task::LocalWaker) {
        for p in &mut self.0 {
            p.clear_read_ready(mio::Ready::readable()).unwrap(); // TODO
            p.clear_write_ready().unwrap();
        }
    }
}


#[derive(Debug)]
struct AWatch(dbus::Watch);

impl mio::Evented for AWatch {
    fn register(&self,
                poll: &mio::Poll,
                token: mio::Token,
                mut interest: mio::Ready,
                mut opts: mio::PollOpt) -> std::io::Result<()>
    {
        if !self.0.readable() { interest.remove(mio::Ready::readable()) };
        if !self.0.writable() { interest.remove(mio::Ready::writable()) };
        opts.remove(mio::PollOpt::edge());
        opts.insert(mio::PollOpt::level());
        mio::unix::EventedFd(&self.0.fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &mio::Poll,
                  token: mio::Token,
                  mut interest: mio::Ready,
                  mut opts: mio::PollOpt) -> std::io::Result<()>
    {
        if !self.0.readable() { interest.remove(mio::Ready::readable()) };
        if !self.0.writable() { interest.remove(mio::Ready::writable()) };
        opts.remove(mio::PollOpt::edge());
        opts.insert(mio::PollOpt::level());
        mio::unix::EventedFd(&self.0.fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> std::io::Result<()> {
        mio::unix::EventedFd(&self.0.fd()).deregister(poll)
    }
}


#[cfg(test)]
mod tests {
    use super::Io;
    use crate::{ReplyMessage, Connection};
    use futures::{FutureExt, TryFutureExt};
    use tokio;

    #[test]
    fn tokio_basic() {
        let ctr = Connection::new_session::<Io>().unwrap();
        let c = ctr.handle();
        // let mut exec = tmlf::Executor::new().unwrap();
        tokio::spawn(futures::compat::Compat::new(ctr));

        let remote_path = c.with_path("org.freedesktop.DBus", "/");
        let reply_future: ReplyMessage = remote_path.method_call_with_args(
            &"org.freedesktop.DBus".into(), &"ListNames".into(), |_| {});
        let r2: futures::future::IntoFuture<_> = reply_future.into_future();
        let r3 = r2.then(|msg| {
            let msg = msg.unwrap();
            let reply: Vec<String> = msg.read1().unwrap();
            let my_name = c.unique_name();
            assert!(reply.len() > 0);
            assert!(reply.iter().any(|t| t == my_name));
            println!("{:?}, {:?}", my_name, reply);
            futures::future::ready(())
        });
        tokio::run(r3);
    }
}
