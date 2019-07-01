
use std::sync::Arc;
use std::pin::Pin;

use crate::{Error, ConnHandle, Command};

use dbus::channel::{MessageDispatcher, MessageDispatcherConfig, Channel};
use dbus::message::MatchRule;
use dbus::connection::Watch;

use futures::task;
use futures::channel::{mpsc, oneshot};

/// Internal trait. Don't mess with it unless you want to use it for
/// your own main loop.
pub trait IoHandler {
    fn new(watches: &[Watch]) -> Result<Self, Error> where Self: Sized;
    fn after_read_write(&mut self, ctx: &mut task::Context);
}

/// This is the reactor specific part of the Connection.
///
/// To set this up, you need to
///  * create a new instance of Connection, with a specific IoHandler
///  * grab a handle to the Connection so you can talk to it
///  * spawn the Connection onto a compatible reactor/executor:
///    This must match the IoHandler you created the Connection with.
pub struct Connection {
    txrx: Arc<Channel>,
    io: Box<IoHandler>,
    command_sender: mpsc::UnboundedSender<Command>,
    command_receiver: mpsc::UnboundedReceiver<Command>,
    dispatcher: MessageDispatcher<DispatcherCfg>,
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
        let x = Arc::new(x);
        Ok(Connection {
            txrx: x.clone(),
            io: Box::new(io),
            command_sender: s,
            command_receiver: r,
            dispatcher: dbus::MessageDispatcher::new(DispatcherCfg { txrx: x, streams: vec!() }),
            quit: false
        })
    }

    pub fn handle(&self) -> ConnHandle { ConnHandle(self.txrx.clone(), self.command_sender.clone()) }

    fn check_cmd(&mut self, ctx: &mut task::Context) -> bool {
        use futures::Stream;
        let cmd = {
            let p = Pin::new(&mut self.command_receiver);
            p.poll_next(ctx)
        };
        if let task::Poll::Ready(cmd) = cmd {
            match cmd {
                None | Some(Command::Quit) =>  { self.quit = true; },
                Some(Command::AddReply(serial, sender)) => { self.dispatcher.add_reply(serial, sender); },
                Some(Command::AddStream(rule, sender)) => { self.dispatcher.inner_mut().streams.push((rule, sender)); },
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

impl futures::Future for Connection {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<()> {
        let mut has_rw = false;
        loop {
            if self.quit { return task::Poll::Ready(()) };
            if self.check_cmd(ctx) { continue; }
            if self.check_msg() {
                has_rw = false;
                continue;
            }
            if !has_rw {
                self.txrx.read_write(Some(0)).unwrap(); // TODO
                has_rw = true;
                self.io.after_read_write(ctx);
                continue;
            }
            return task::Poll::Pending;
        }
    }
}

struct DispatcherCfg {
    txrx: Arc<dbus::channel::Channel>,
    streams: Vec<(MatchRule<'static>, mpsc::UnboundedSender<dbus::Message>)>,
}

impl MessageDispatcherConfig for DispatcherCfg {
    type Reply = oneshot::Sender<dbus::Message>;
    fn on_reply(r: Self::Reply, msg: dbus::Message, _: &mut MessageDispatcher<Self>) {
        let _ = r.send(msg); // If the receiver has been canceled, the best thing is probably to ignore.
    }
    fn on_signal(msg: dbus::Message, cfg: &mut MessageDispatcher<Self>) {
        for &mut (ref rule, ref mut sender) in &mut cfg.inner_mut().streams {
            if rule.matches(&msg) {
                let _ = sender.unbounded_send(msg); // TODO: If receiver has been dropped, the best thing is probably to remove the receiver.
                return;
            }
        }
    }
    fn on_send(msg: dbus::Message, cfg: &mut MessageDispatcher<Self>) { cfg.inner().txrx.send(msg).unwrap(); }
}


