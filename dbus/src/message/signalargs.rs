use crate::arg;
use crate::{Message, MessageType};
use crate::message::MatchRule;
use crate::strings::{BusName, Path, Interface, Member};

/// Helper methods for structs representing a Signal
///
/// # Example
///
/// Listen to InterfacesRemoved signal from org.bluez.obex.
///
/// ```rust,no_run
/// use dbus::ffidisp::Connection;
/// use dbus::message::SignalArgs;
/// use dbus::ffidisp::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesRemoved as IR;
///
/// let c = Connection::new_session().unwrap();
/// // Add a match for this signal
/// let mstr = IR::match_str(Some(&"org.bluez.obex".into()), None);
/// c.add_match(&mstr).unwrap();
///
/// // Wait for the signal to arrive.
/// for msg in c.incoming(1000) {
///     if let Some(ir) = IR::from_message(&msg) {
///         println!("Interfaces {:?} have been removed from bluez on path {}.", ir.interfaces, ir.object);
///     }
/// }
/// ```

pub trait SignalArgs {
    /// D-Bus name of signal
    const NAME: &'static str;

    /// D-Bus name of interface this signal belongs to
    const INTERFACE: &'static str;

    /// Returns a message that emits the signal.
    fn to_emit_message(&self, path: &Path) -> Message where Self: arg::AppendAll {
        let mut m = Message::signal(path, &Interface::from(Self::INTERFACE), &Member::from(Self::NAME));
        arg::AppendAll::append(self, &mut arg::IterAppend::new(&mut m));
        m
    } 

    /// If the message is a signal of the correct type, return its arguments, otherwise return None.
    ///
    /// This does not check sender and path of the message, which is likely relevant to you as well.
    fn from_message(m: &Message) -> Option<Self> where Self: Sized + arg::ReadAll {
        if m.msg_type() != MessageType::Signal { None }
        else if m.interface().as_ref().map(|x| &**x) != Some(Self::INTERFACE) { None }
        else if m.member().as_ref().map(|x| &**x) != Some(Self::NAME) { None }
        else {
            arg::ReadAll::read(&mut m.iter_init()).ok()
        }
    }

    /// Returns a match rule matching this signal.
    ///
    /// If sender and/or path is None, matches all senders and/or paths.
    fn match_rule<'a>(sender: Option<&'a BusName>, path: Option<&'a Path>) -> MatchRule<'a> {
        let mut m: MatchRule = Default::default();
        m.sender = sender.cloned();
        m.path = path.cloned();
        m.msg_type = Some(MessageType::Signal);
        m.interface = Some(Self::INTERFACE.into());
        m.member = Some(Self::NAME.into());
        m
    }


    /// Returns a string that can be sent to `Connection::add_match`.
    ///
    /// If sender and/or path is None, matches all senders and/or paths.
    fn match_str(sender: Option<&BusName>, path: Option<&Path>) -> String {
        Self::match_rule(sender, path).match_str()
    }
}

#[test]
fn intf_removed() {
    use crate::ffidisp::{Connection, BusType};
    use crate::ffidisp::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesRemoved as IR;
    let c = Connection::get_private(BusType::Session).unwrap();
    let mstr = IR::match_str(Some(&c.unique_name().into()), Some(&"/hello".into()));
    println!("Match str: {}", mstr);
    c.add_match(&mstr).unwrap();
    let ir = IR { object: "/hello".into(), interfaces: vec!("ABC.DEF".into(), "GHI.JKL".into()) };

    let cp = c.with_path("dbus.dummy", "/hello", 2000);
    cp.emit(&ir).unwrap();

    for msg in c.incoming(1000) {
        if &*msg.sender().unwrap() != &*c.unique_name() { continue; }
        if let Some(ir2) = IR::from_message(&msg) {
            assert_eq!(ir2.object, ir.object);
            assert_eq!(ir2.interfaces, ir.interfaces);
            break;
        }
    }
}
