use crate::channel::{Channel, Watch};
use crate::{Error, Message};
use std::time::Duration;
use std::os::unix::net::UnixStream;
use crate::to_c_str;

/// Which bus to connect to
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum BusType {
    /// The Session bus - local to every logged in session
    Session,
    /// The system wide bus
    System,
    /// The bus that started us, if any
    Starter,
}

#[derive(Debug)]
pub struct ConnHandle {
    stream: UnixStream,
    unique_name: Option<String>,
}

impl Channel {
    /// Creates a new D-Bus connection.
    ///
    /// Blocking: until the connection is up and running.
    pub fn get_private(bus: BusType) -> Result<Channel, Error> {
        let addr = match bus {
            BusType::Session => native::address::read_session_address(),
            BusType::System => native::address::read_system_address(),
            BusType::Starter => native::address::read_starter_address(),
        }.map_err(|x| Error::new_failed(&format!("{}", x)))?;
        let mut c = Self::open_private(&addr)?;
        c.register()?;
        Ok(c)
    }

    /// Creates a new D-Bus connection to a remote address.
    ///
    /// Note: for all common cases (System / Session bus) you probably want "get_private" instead.
    ///
    /// Blocking: until the connection is established.
    pub fn open_private(address: &str) -> Result<Channel, Error> {
        let stream = native::address::connect_blocking(address)
            .map_err(|x| Error::new_failed(&format!("{}", x)))?;
        Ok(Channel {
            handle: ConnHandle {
                stream,
                unique_name: None,
            }
        })
    }

    /// Registers a new D-Bus connection with the bus.
    ///
    /// Note: `get_private` does this automatically, useful with `open_private`
    ///
    /// Blocking: until a "Hello" response is received from the server.
    pub fn register(&mut self) -> Result<(), Error> {
        if self.unique_name().is_some() { return Ok(()); }
        let msg = Message::new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello").unwrap();
        let reply = self.send_with_reply_and_block(msg, Duration::from_millis(5000))?;
        self.handle.unique_name = Some(reply.read1()?);
        Ok(())
    }

    /// Gets whether the connection is currently open.
    pub fn is_connected(&self) -> bool {
        todo!()
    }

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> Option<&str> {
        self.handle.unique_name.as_ref().map(|x| &**x)
    }

    /// Puts a message into libdbus out queue, and tries to send it.
    ///
    /// Returns a serial number than can be used to match against a reply.
    ///
    /// Note: usually the message is sent when this call happens, but in
    /// case internal D-Bus buffers are full, it will be left in the out queue.
    /// Call "flush" or "read_write" to retry flushing the out queue.
    pub fn send(&self, msg: Message) -> Result<u32, ()> {
        todo!()
    }

    /// Sends a message over the D-Bus and waits for a reply. This is used for method calls.
    ///
    /// Blocking: until a reply is received or the timeout expires.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    ///
    /// Note: In case pop_message and send_with_reply_and_block is called in parallel from different threads,
    /// they might race to retreive the reply message from the internal queue.
    pub fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        let serial = self.send(msg).map_err(|_| Error::new_failed("Failed to send message"))?;
        todo!()
    }

    /// Flush the queue of outgoing messages.
    ///
    /// Blocking: until the outgoing queue is empty.
    pub fn flush(&self) { todo!() }

    /// Read and write to the connection.
    ///
    /// Incoming messages are put in the internal queue, outgoing messages are written.
    ///
    /// Blocking: If there are no messages, for up to timeout, or forever if timeout is None.
    /// For non-blocking behaviour, set timeout to Some(0).
    pub fn read_write(&self, timeout: Option<Duration>) -> Result<(), ()> {
        todo!()
    }

    /// Removes a message from the incoming queue, or returns None if the queue is empty.
    ///
    /// Use "read_write" first, so that messages are put into the incoming queue.
    /// For unhandled messages, please call MessageDispatcher::default_dispatch to return
    /// default replies for method calls.
    pub fn pop_message(&self) -> Option<Message> {
        todo!()
    }

    /// Removes a message from the incoming queue, or waits until timeout if the queue is empty.
    ///
    pub fn blocking_pop_message(&self, timeout: Duration) -> Result<Option<Message>, Error> {
        todo!()
    }

    /// Gets the file descriptor to listen for read/write.
    ///
    /// Panics: if set_watch_enabled is false.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn watch(&self) -> Watch {
        todo!()
    }

    /// Get an up-to-date list of file descriptors to watch.
    ///
    /// Obsolete - in practice, you can use watch and set_watch_enabled instead.
    pub fn watch_fds(&mut self) -> Result<Vec<Watch>, ()> {
        Ok(vec![self.watch()])
    }
}


pub fn machine_id_reply(m: &Message) -> Message {
    match native::machineid::read_machine_id() {
        Ok(id) => m.method_return().append1(id),
        Err(_) =>  m.error(&"org.freedesktop.DBus.Error.Failed".into(), &to_c_str("Failed to retreive UUID")),
    }
}
