use crate::{Error, Message};
use std::time::Duration;
use super::BusType;
use std::sync::Mutex;
use futures_executor::block_on;
use dbus_native_channel::address;
use std::error::Error as stdError;
use futures_util::io as fio;
use std::pin::Pin;
use std::os::unix::net::UnixStream;

/// Low-level connection - handles read/write to the socket
///
/// You probably do not need to worry about this as you would typically
/// use the various blocking and non-blocking "Connection" structs instead.
///
/// This version avoids any dependency on the C dbus library, making it possible
/// to use with async rust code etc.
//#[derive(Debug)]
pub struct Channel {
    unique_name: Option<crate::strings::BusName<'static>>,
    out_queue: Mutex<(u32, Vec<Message>)>,
    reader: Pin<Box<dyn fio::AsyncBufRead>>,
    writer: Pin<Box<dyn fio::AsyncWrite>>,
}

async fn do_auth<W: fio::AsyncWrite + std::marker::Unpin, R: fio::AsyncBufRead + std::marker::Unpin>(r: &mut R, w: &mut W) -> Result<bool, Box<dyn stdError>> {
    use dbus_native_channel::authentication::Authentication;
    use fio::{AsyncWriteExt, AsyncBufReadExt};
    let (mut auth, s) = Authentication::new(true);
    w.write_all(s.as_bytes()).await?;
    loop {
        let mut v = vec!();
        r.read_until(b'\n', &mut v).await?;
        let s = auth.handle(&v)?;
        w.write_all(s.as_bytes()).await?;
        if let Authentication::Begin(unixfd) = &auth {
            return Ok(*unixfd)
        }
    }
}

impl Channel {

    /// Creates a new D-Bus connection.
    ///
    /// Blocking: until the connection is up and running.
    pub fn get_private(bus: BusType) -> Result<Channel, Error> {
        block_on(async {
            Self::get_private_async(bus, |s| {
                s.set_nonblocking(true).unwrap();
                let s2 = s.try_clone().unwrap();
                let r = fio::AllowStdIo::new(std::io::BufReader::new(s));
                let w = fio::AllowStdIo::new(s2);
                (r, w)
            }).await.map_err(|x| Error::new_failed(&x.to_string()))
        })
    }

    /// Creates a new D-Bus connection without blocking.
    pub async fn get_private_async<R, W, F>(bus: BusType, f: F) -> Result<Channel, Box<dyn stdError>>
    where
        R: fio::AsyncBufRead + 'static,
        W: fio::AsyncWrite + 'static,
        F: FnOnce(UnixStream) -> (R, W) {
        let addr = match bus {
            BusType::Starter => address::read_starter_address(),
            BusType::Session => address::read_session_address(),
            BusType::System => address::read_system_address(),
        }?;
        // Tokio does not do this any less blocking than we do here.
        let stream = address::connect_blocking(&addr)?;
        let (r, w) = f(stream);
        let (mut r, mut w) = (Box::pin(r), Box::pin(w));
        // let mut astream = fio::AllowStdIo::new(stream);
        do_auth(&mut r, &mut w).await?;
        let mut c = Channel {
            unique_name: None,
            out_queue: Default::default(),
            reader: r,
            writer: w,
        };
        // TODO: hello message here
        Ok(c)
    }

    /// Puts a message into the out queue, and tries to send it.
    ///
    /// Returns a serial number than can be used to match against a reply.
    ///
    /// Note: usually the message is sent when this call happens, but in
    /// case internal buffers are full, it will be left in the out queue.
    /// Call "flush" or "read_write" to retry flushing the out queue.
    pub fn send(&self, mut msg: Message) -> Result<u32, ()> {
        let mut q = self.out_queue.lock().unwrap();
        let serial = q.0;
        msg.set_serial(serial);
        q.0 += 1;
        q.1.push(msg);
        Ok(serial)
    }

    /// Removes a message from the incoming queue, or waits until timeout if the queue is empty.
    ///
    pub fn blocking_pop_message(&self, timeout: Duration) -> Result<Option<Message>, Error> {
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
        let _serial = self.send(msg).map_err(|_| Error::new_failed("Failed to send message"))?;
        todo!()
    }

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> Option<&crate::strings::BusName> {
        self.unique_name.as_ref()
    }
}
