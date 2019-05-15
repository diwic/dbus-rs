use thin_main_loop as tml;
use thin_main_loop::future as tmlf;

use std::pin::Pin;
use futures::task;

use crate::connection::IoHandler;
use crate::Error;

pub struct Io(Vec<tmlf::Io>);

impl IoHandler for Io {
    fn new(watches: &[dbus::Watch]) -> Result<Self, Error> where Self: Sized {
        Ok(Io(watches.into_iter().filter_map(|w| {
            match (w.readable(), w.writable()) {
                (false, false) => None,
                (true, false) => Some(tmlf::io(tml::CbHandle(w.fd()), tml::IODirection::Read)),
                _ => unimplemented!(),
            }
        }).collect()))
    }
    fn after_read_write(&mut self, ctx: &mut task::Context) {
        for io in &mut self.0 {
            use futures::Stream;
            let p = Pin::new(io);
            let _ = p.poll_next(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use thin_main_loop::future as tmlf;
    use thin_main_loop as tml;
    use super::Io;
    use crate::{ReplyMessage, Connection};
    use futures::{FutureExt, TryFutureExt, StreamExt, TryStreamExt};

    #[test]
    fn basic_conn() {
        let ctr = Connection::new_session::<Io>().unwrap();
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
        let ctr = Connection::new_session::<Io>().unwrap();
        let c = ctr.handle();
        let mut exec = tmlf::Executor::new().unwrap();
        exec.spawn(ctr);

        use crate::stdintf::org_freedesktop::DBus;
        let r = c.with_dbus_path().get_interfaces().into_future().then(|reply| {
            let reply = reply.unwrap();
            assert!(reply.len() > 0);
            println!("Supported interfaces: {:?}", reply);
            tml::terminate();
            futures::future::ready(())
        });
        exec.spawn(r);
        exec.run();
    }

    #[test]
    fn request_name() {
        let ctr = Connection::new_session::<Io>().unwrap();
        let c = ctr.handle();
        let mut exec = tmlf::Executor::new().unwrap();
        exec.spawn(ctr);

        use crate::stdintf::org_freedesktop::DBusNameOwnerChanged;
        let mut has_name = false;
        let our_name = c.unique_name();
        let s = c.add_signal_stream::<DBusNameOwnerChanged>(None, None).into_stream().for_each(move |s| {
            let s = s.unwrap();
            println!("{:?}", s);
            if s.name == "com.example.dbus-rs.namerequest" {
                if has_name {
                    assert_eq!(s.old_owner, our_name);
                    assert_eq!(s.new_owner, "");
                    tml::terminate(); 
                } else {
                    assert_eq!(s.new_owner, our_name);
                    has_name = true;
                }
            }
            futures::future::ready(())
        });
        exec.spawn(s);

        let r = c.request_name("com.example.dbus-rs.namerequest", true, true, true)
            .and_then(|reply| {
                assert_eq!(reply, dbus::RequestNameReply::PrimaryOwner);
                c.release_name("com.example.dbus-rs.namerequest0")
            }).and_then(|reply| {
                assert_eq!(reply, dbus::ReleaseNameReply::NonExistent);
                c.release_name("com.example.dbus-rs.namerequest")
            }).into_future().then(|reply| {
                assert_eq!(reply.unwrap(), dbus::ReleaseNameReply::Released);
               // tml::terminate();
                futures::future::ready(())
            });

        exec.spawn(r);
        exec.run();
    }
}
