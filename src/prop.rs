use super::{Connection, Message, MessageItem, Error};
use std::collections::TreeMap;

pub struct Props {
    name: String,
    path: String,
    interface: String,
    timeout_ms: int,
}

impl Props {
    pub fn new(name: &str, path: &str, interface: &str, timeout_ms: int) -> Props {
        Props {
            name: name.to_string(),
            path: path.to_string(),
            interface: interface.to_string(),
            timeout_ms: timeout_ms
        }
    }

    pub fn get(&self, conn: &mut Connection, propname: &str) -> Result<MessageItem, Error> {
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
            "org.freedesktop.DBus.Properties", "Get").unwrap();
        m.append_items(&[
            MessageItem::Str(self.interface.clone()),
            MessageItem::Str(propname.to_string())
        ]);
        let mut r = try!(conn.send_with_reply_and_block(m, self.timeout_ms));
        let reply = try!(r.as_result()).get_items();
        if reply.len() == 1 {
            match &reply[0] {
                &MessageItem::Variant(ref v) => return Ok(*v.deref().clone()),
                _ => {},
            }
       }
       let f = format!("Invalid reply for property get {}: '{}'", propname, reply);
       return Err(Error::new_custom("InvalidReply", f.as_slice()));
    }

    pub fn set(&self, conn: &mut Connection, propname: &str, value: MessageItem) -> Result<(), Error> {
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
            "org.freedesktop.DBus.Properties", "Set").unwrap();
        m.append_items(&[
            MessageItem::Str(self.interface.clone()),
            MessageItem::Str(propname.to_string()),
            MessageItem::Variant(box value),
        ]);
        let mut r = try!(conn.send_with_reply_and_block(m, self.timeout_ms));
        try!(r.as_result());
        Ok(())
    }

    pub fn get_all(&self, conn: &mut Connection) -> Result<TreeMap<String, MessageItem>, Error> {
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
            "org.freedesktop.DBus.Properties", "GetAll").unwrap();
        m.append_items(&[MessageItem::Str(self.interface.clone())]);
        let mut r = try!(conn.send_with_reply_and_block(m, self.timeout_ms));
        let reply = try!(r.as_result()).get_items();
        if reply.len() == 1 {
            match &reply[0] {
                &MessageItem::Array(ref a, _) => {
                    let mut t = TreeMap::new();
                    let mut haserr = false;
                    for p in a.iter() {
                        match p {
                            &MessageItem::DictEntry(ref k, ref v) => {
                                match &**k {
                                    &MessageItem::Str(ref ks) => { t.insert(ks.to_string(), *v.deref().clone()); },
                                    _ => { haserr = true; }
                                }
                            }
                            _ => { haserr = true; }
                        }
                    }
                    if !haserr {
                        return Ok(t)
                    };
                }
                _ => {},
            }
       }
       let f = format!("Invalid reply for property GetAll: '{}'", reply);
       return Err(Error::new_custom("InvalidReply", f.as_slice()));
    }
}

/* Unfortunately org.freedesktop.DBus has no properties we can use for testing, but PolicyKit should be around on most distros. */
#[test]
fn test_get_policykit_version() {
    use super::BusType;
    let mut c = Connection::get_private(BusType::System).unwrap();
    let p = Props::new("org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority", 10000);

    /* Let's use both the get and getall methods and see if we get the same result */
    let v = p.get(&mut c, "BackendVersion").unwrap();
    let vall = p.get_all(&mut c).unwrap();

    let v2 = match vall.get("BackendVersion").unwrap() {
        &MessageItem::Variant(ref q) => &**q,
        _ => { panic!("Invalid GetAll: {}", vall); }
    };

    assert_eq!(&v, &*v2);
    match v {
        MessageItem::Str(ref s) => { println!("Policykit Backend version is {}", s); }
        _ => { panic!("Invalid Get: {}", v); }
    };
    
}
