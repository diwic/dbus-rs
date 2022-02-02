/* This example asks the rtkit service to make our thread realtime priority.
   Rtkit puts a few limitations on us to let us become realtime, such as setting
   RLIMIT_RTTIME correctly, hence the syscalls. */

use std::cmp;
use std::time::Duration;

use rustix::process::{Resource, Rlimit};

fn make_realtime(prio: u32) -> Result<u32, Box<dyn std::error::Error>> {
    let c = dbus::blocking::Connection::new_system()?;

    let proxy = c.with_proxy("org.freedesktop.RealtimeKit1", "/org/freedesktop/RealtimeKit1",
        Duration::from_millis(10000));
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;

    // Make sure we don't fail by wanting too much
    let max_prio: i32 = proxy.get("org.freedesktop.RealtimeKit1", "MaxRealtimePriority")?;
    let prio = cmp::min(prio, max_prio as u32);

    // Enforce RLIMIT_RTPRIO, also a must before asking rtkit for rtprio
    let max_rttime: i64 = proxy.get("org.freedesktop.RealtimeKit1", "RTTimeUSecMax")?;
    let old_limit = rustix::process::getrlimit(Resource::Rttime);

    let new_limit = Rlimit { current: Some(max_rttime as u64), maximum: Some(max_rttime as u64) };
    rustix::process::setrlimit(Resource::Rttime, new_limit).map_err(|err| format!("setrlimit failed: {:?}", err))?;

    // Finally, let's ask rtkit to make us realtime
    let thread_id = rustix::thread::gettid();
    let r = proxy.method_call("org.freedesktop.RealtimeKit1", "MakeThreadRealtime", (thread_id.as_raw_nonzero().get() as u64, prio));

    if r.is_err() {
        rustix::process::setrlimit(Resource::Rttime, old_limit).ok();
    }

    r?;
    Ok(prio)
}


fn main() {
    match make_realtime(5) {
        Ok(n) => println!("Got rtprio, level {}", n),
        Err(e) => println!("No rtprio: {}", e),
    }
}
