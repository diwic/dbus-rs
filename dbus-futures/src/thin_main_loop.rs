use thin_main_loop as tml;
use thin_main_loop::future as tmlf;

use std::sync::Arc;
use std::pin::Pin;

use crate::{Error, ConnHandle, Command};

use futures::task;
use futures::channel::{mpsc, oneshot};

use std::collections::HashMap;


/// This is the reactor specific part of the Connection.
///
/// You need to spawn this on a thin_main_loop executor, or nothing will happen.
/// Before you do that, you will probably want to get a handle or two.
pub struct ConnTxRx {
    txrx: Arc<dbus::TxRx>,
    all_io: Vec<tmlf::Io>,
    command_sender: mpsc::UnboundedSender<Command>,
    command_receiver: mpsc::UnboundedReceiver<Command>,
    replies: HashMap<u32, oneshot::Sender<dbus::Message>>,
    quit: bool,
}

impl ConnTxRx {
    /// Creates a new D-Bus connection and connects it to the Session bus.
    ///
    /// Blocking: until the connection is up and running. 
    pub fn new_session() -> Result<Self, Error> {
        let mut x = dbus::TxRx::get_private(dbus::BusType::Session)?;
        // TODO we don't support any changing of watches just yet
        let watches = x.watch_fds().unwrap();
        let all_io: Vec<_> = watches.into_iter().filter_map(|w| {
            match (w.readable(), w.writable()) {
                (false, false) => None,
                (true, false) => Some(tmlf::io(tml::CbHandle(w.fd()), tml::IODirection::Read)),
                _ => unimplemented!(),
            }
        }).collect();
        let (s, r) = mpsc::unbounded();
        Ok(ConnTxRx { txrx: Arc::new(x), all_io: all_io, command_sender: s, command_receiver: r, replies: Default::default(), quit: false })
    }

    pub fn handle(&self) -> ConnHandle { ConnHandle(self.txrx.clone(), self.command_sender.clone()) }


    fn check_cmd(&mut self, lw: &task::LocalWaker) -> bool {
        use futures::Stream;
        let cmd = {
            let p = Pin::new(&mut self.command_receiver);
            p.poll_next(lw)
        };
        if let task::Poll::Ready(cmd) = cmd {
            match cmd {
                None | Some(Command::Quit) =>  { self.quit = true; },
                Some(Command::AddReply(serial, sender)) => { self.replies.insert(serial, sender); },
            };
            true
        } else { false }
    }

    fn check_msg(&mut self) -> bool {
        if let Some(msg) = self.txrx.pop_message() {
            if let Some(serial) = msg.get_reply_serial() {
                if let Some(sender) = self.replies.remove(&serial) {
                    let _ = sender.send(msg); // If the sender was removed, just ignore that.
                }
            }
            true
        } else { false }
    }

    fn register_io(&mut self, lw: &task::LocalWaker) {
        for io in &mut self.all_io {
            let p = Pin::new(io);
            let _ = futures::Stream::poll_next(p, lw);
        }
    }
}

impl futures::Future for ConnTxRx {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, lw: &task::LocalWaker) -> task::Poll<()> {
        let mut has_rw = false;
        loop {
            if self.quit { return task::Poll::Ready(()) };
            if self.check_cmd(lw) { continue; }
            if self.check_msg() {
                has_rw = false;
                continue;
            }
            if !has_rw {
                self.txrx.read_write(Some(0)).unwrap(); // TODO
                has_rw = true;
                self.register_io(lw);
                continue;
            }
            return task::Poll::Pending;
        }
    }
}

#[cfg(test)]
mod tests {
    use thin_main_loop::future as tmlf;
    use thin_main_loop as tml;
    use super::ConnTxRx;
    use crate::ReplyMessage;
    use futures::{FutureExt, TryFutureExt};


    #[test]
    fn basic_conn() {
        let ctr = ConnTxRx::new_session().unwrap();
        let c = ctr.handle();
        let mut exec = tmlf::Executor::new().unwrap();
        exec.spawn(ctr);

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
            tml::terminate();
            futures::future::ready(())
        });
        exec.spawn(r3);
        exec.run();
        // let reply: Vec<String> = exec.block_on(reply_future).unwrap();
    }

    #[test]
    fn gen_conn() {
        let ctr = ConnTxRx::new_session().unwrap();
        let c = ctr.handle();
        let mut exec = tmlf::Executor::new().unwrap();
        exec.spawn(ctr);

        use crate::stdintf::org_freedesktop::DBus;
        let remote_path = c.with_path("org.freedesktop.DBus", "/org/freedesktop/DBus");
        let r = remote_path.get_interfaces().into_future().then(|reply| {
            let reply = reply.unwrap();
            assert!(reply.len() > 0);
            println!("Supported interfaces: {:?}", reply);
            tml::terminate();
            futures::future::ready(())
        });
        exec.spawn(r);
        exec.run();
    }
}
