
use std::sync::Arc;
use std::pin::Pin;

use crate::{Error, ConnHandle, Command};

use futures::task;
use futures::channel::{mpsc, oneshot};

/// Internal trait. Don't mess with it unless you want to use it for
/// your own main loop.
pub trait IoHandler {
    fn new(watches: &[dbus::Watch]) -> Result<Self, Error> where Self: Sized;
    fn after_read_write(&mut self, lw: &task::LocalWaker);
}

/// This is the reactor specific part of the Connection.
///
/// To set this up, you need to
///  * create a new instance of Connection, with a specific IoHandler
///  * grab a handle to the Connection so you can talk to it
///  * spawn the Connection onto a compatible reactor/executor:
///    This must match the IoHandler you created the Connection with.
pub struct Connection {
    txrx: Arc<dbus::TxRx>,
    io: Box<IoHandler>,
    command_sender: mpsc::UnboundedSender<Command>,
    command_receiver: mpsc::UnboundedReceiver<Command>,
    dispatcher: dbus::MessageDispatcher<Self>,
    quit: bool,
}


impl Connection {
    /// Creates a new D-Bus connection and connects it to the Session bus.
    ///
    /// Blocking: until the connection is up and running. 
    pub fn new_session<IO: IoHandler + 'static>() -> Result<Self, Error> {
        let mut x = dbus::TxRx::get_private(dbus::BusType::Session)?;
        // TODO we don't support any changing of watches just yet
        let watches = x.watch_fds().map_err(|_| Error::failed(&"failed to get watches"))?;
        let io = IO::new(&watches)?;
        let (s, r) = mpsc::unbounded();
        Ok(Connection {
            txrx: Arc::new(x),
            io: Box::new(io),
            command_sender: s,
            command_receiver: r,
            dispatcher: dbus::MessageDispatcher::new(),
            quit: false
        })
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
                Some(Command::AddReply(serial, sender)) => { self.dispatcher.add_reply(serial, sender); },
            };
            true
        } else { false }
    }

    fn check_msg(&mut self) -> bool {
        if let Some(msg) = self.txrx.pop_message() {
            self.dispatcher.dispatch(msg);
            true
        } else { false }
    }
}

impl dbus::MessageDispatcherConfig for Connection {
    type Reply = oneshot::Sender<dbus::Message>;
    fn call_reply(r: Self::Reply, msg: dbus::Message) {
        let _ = r.send(msg); // If the receiver has been canceled, the best thing is probably to ignore.
    }
}

impl futures::Future for Connection {
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
                self.io.after_read_write(lw);
                continue;
            }
            return task::Poll::Pending;
        }
    }
}

