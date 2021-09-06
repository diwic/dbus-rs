// More advanced server example, tokio + crossroads version

// This is supposed to look like a D-Bus service that allows the user to manipulate storage devices.

use dbus_tokio::connection;
use futures::future;
use tokio::time::sleep;
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::MatchRule;
use dbus::nonblock::SyncConnection;
use std::time::Duration;
use dbus::{Path, Message};
use dbus_crossroads::{MethodErr, Crossroads, IfaceToken, IfaceBuilder};
use std::sync::{Arc, Mutex};

// Our storage device
#[derive(Debug)]
struct Device {
    description: String,
    path: Path<'static>,
    index: i32,
    online: bool,
    checking: bool,
}

impl Device {
    // Creates a "test" device (not a real one, since this is an example).
    fn new_bogus(index: i32) -> Device {
        Device {
            description: format!("This is device {}, which is {}.", index,
                ["totally awesome", "really fancy", "still going strong"][(index as usize) % 3]),
            path: format!("/Device{}", index).into(),
            index: index,
            online: index % 2 == 0,
            checking: false,
        }
    }
}

fn register_iface(cr: &Arc<Mutex<Crossroads>>, conn: Arc<SyncConnection>) -> IfaceToken<Device> {
    let cr2 = cr.clone();
    let mut cr_lock = cr.lock().unwrap();
    cr_lock.register("com.example.dbus.rs.device", |b: &mut IfaceBuilder<Device>| {
        // The online property can be both set and get
        b.property("online")
            .get(|_, device| Ok(device.online))
            .set(|_, device, value| {
                if value && device.checking {
                    Err(MethodErr::failed(&"Device currently under check, cannot bring online"))?
                }
                device.online = value;
                Ok(Some(value))
            });
        // The "checking" property is read only
        b.property("checking")
            .emits_changed_false()
            .get(|_, device| Ok(device.checking));
        // ...and so is the "description" property
        b.property("description")
            .emits_changed_const()
            .get(|_, device| Ok(device.description.clone()));

        // Add a method for starting a device check.
        // This method has no input or output arguments.
        b.method("check", (), (), move |_, device, _: ()| {
            if device.checking {
                Err(MethodErr::failed(&"Device currently under check, cannot start another check"))?
            }
            if device.online {
                Err(MethodErr::failed(&"Device is currently online, cannot start check"))?
            }
            device.checking = true;

            let path = device.path.clone();
            let cr_clone = cr2.clone();
            let conn_clone = conn.clone();
            tokio::spawn(async move {
                // Let's pretend we're doing a 15 second check of the device.
                sleep(Duration::from_secs(15)).await;

                // Now we need to set checking to false again.
                // However, at this point we have no longer access to the "device" variable,
                // so we have to do this the manual way.
                let mut cr = cr_clone.lock().unwrap();
                let device: &mut Device = cr.data_mut(&path).unwrap();
                device.checking = false;

                // Send a signal that the check completed.
                let msg = Message::signal(&path, &"com.example.dbus.rs.device".into(), &"CheckComplete".into());
                let _ = conn_clone.send(msg);
            });
            Ok(())
        });
        // Advertise that we send a signal when check completes.
        b.signal::<(), _>("CheckComplete", ());
    })
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Connect to the D-Bus session bus (this is blocking, unfortunately).
    let (resource, c) = connection::new_session_sync()?;

    // The resource is a task that should be spawned onto a tokio compatible
    // reactor ASAP. If the resource ever finishes, you lost connection to D-Bus.
    //
    // To shut down the connection, both call _handle.abort() and drop the connection.
    let _handle = tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    // Create a new crossroads instance.
    //
    // We have to wrap it inside an arc/mutex because we need to modify it outside message handling,
    // i e, when a check is completed.
    let cr = Arc::new(Mutex::new(Crossroads::new()));

    // Build and register our "com.example.dbus.rs.device" interface.
    let token = register_iface(&cr, c.clone());

    // Create devices and register them in the tree
    {
        let mut cr_lock = cr.lock().unwrap();
        for i in 0..10 {
            let d = Device::new_bogus(i);
            cr_lock.insert(d.path.clone(), &[token], d);
        }
    }

    c.request_name("com.example.dbus.rs.advancedserverexample", false, true, false).await?;

    // We add the Crossroads instance to the connection so that incoming method calls will be handled.
    c.start_receive(MatchRule::new_method_call(), Box::new(move |msg, conn| {
        let mut cr_lock = cr.lock().unwrap();
        cr_lock.handle_message(msg, conn).unwrap();
        true
    }));

    // Run forever.
    future::pending::<()>().await;
    unreachable!()
}
