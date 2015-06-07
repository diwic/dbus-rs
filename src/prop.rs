use super::{Connection, Message, MessageItem, Error};
use std::collections::BTreeMap;

/// Client side properties - get and set properties on a remote application.
pub struct Props<'a> {
    name: String,
    path: String,
    interface: String,
    timeout_ms: i32,
    conn: &'a Connection,
}

impl<'a> Props<'a> {
    pub fn new(conn: &'a Connection, name: &str, path: &str, interface: &str, timeout_ms: i32) -> Props<'a> {
        Props {
            name: name.to_string(),
            path: path.to_string(),
            interface: interface.to_string(),
            timeout_ms: timeout_ms,
            conn: conn,
        }
    }

    pub fn get(&self, propname: &str) -> Result<MessageItem, Error> {
        let mut m = Message::new_method_call(&self.name, &self.path,
            "org.freedesktop.DBus.Properties", "Get").unwrap();
        m.append_items(&[
            MessageItem::Str(self.interface.clone()),
            MessageItem::Str(propname.to_string())
        ]);
        let mut r = try!(self.conn.send_with_reply_and_block(m, self.timeout_ms));
        let reply = try!(r.as_result()).get_items();
        if reply.len() == 1 {
            if let &MessageItem::Variant(ref v) = &reply[0] {
                return Ok((**v).clone())
            }
       }
       let f = format!("Invalid reply for property get {}: '{:?}'", propname, reply);
       return Err(Error::new_custom("InvalidReply", &f));
    }

    pub fn set(&self, propname: &str, value: MessageItem) -> Result<(), Error> {
        let mut m = Message::new_method_call(&self.name, &self.path,
            "org.freedesktop.DBus.Properties", "Set").unwrap();
        m.append_items(&[
            MessageItem::Str(self.interface.clone()),
            MessageItem::Str(propname.to_string()),
            MessageItem::Variant(Box::new(value)),
        ]);
        let mut r = try!(self.conn.send_with_reply_and_block(m, self.timeout_ms));
        try!(r.as_result());
        Ok(())
    }

    pub fn get_all(&self) -> Result<BTreeMap<String, MessageItem>, Error> {
        let mut m = Message::new_method_call(&self.name, &self.path,
            "org.freedesktop.DBus.Properties", "GetAll").unwrap();
        m.append_items(&[MessageItem::Str(self.interface.clone())]);
        let mut r = try!(self.conn.send_with_reply_and_block(m, self.timeout_ms));
        let reply = try!(r.as_result()).get_items();
        if reply.len() == 1 {
            if let &MessageItem::Array(ref a, _) = &reply[0] {
                let mut t = BTreeMap::new();
                let mut haserr = false;
                for p in a.iter() {
                    if let &MessageItem::DictEntry(ref k, ref v) = p {
                        if let &MessageItem::Str(ref ks) = &**k {
                            if let &MessageItem::Variant(ref vv) = &**v {
                                t.insert(ks.to_string(), (**vv).clone());
                            } else { haserr = true; };
                        } else { haserr = true; };
                    } else { haserr = true; };
                }
                if !haserr { return Ok(t) };
            }
        }
        let f = format!("Invalid reply for property GetAll: '{:?}'", reply);
        return Err(Error::new_custom("InvalidReply", &f));
    }
}

/// Wrapper around Props that keeps a map of fetched properties.
pub struct PropHandler<'a> {
    p: Props<'a>,
    map: BTreeMap<String, MessageItem>,
}

impl<'a> PropHandler<'a> {
    pub fn new(p: Props) -> PropHandler {
        PropHandler { p: p, map: BTreeMap::new() }
    }

    pub fn get_all(&mut self) -> Result<(), Error> {
        self.map = try!(self.p.get_all());
        Ok(())
    }

    pub fn map_mut(&mut self) -> &mut BTreeMap<String, MessageItem> { &mut self.map }
    pub fn map(&self) -> &BTreeMap<String, MessageItem> { &self.map }

    pub fn get(&mut self, propname: &str) -> Result<&MessageItem, Error> {
        let v = try!(self.p.get(propname));
        self.map.insert(propname.to_string(), v);
        Ok(self.map.get(propname).unwrap())
    }

    pub fn set(&mut self, propname: &str, value: MessageItem) -> Result<(), Error> {
        try!(self.p.set(propname, value.clone()));
        self.map.insert(propname.to_string(), value);
        Ok(())
    }
}


/* Unfortunately org.freedesktop.DBus has no properties we can use for testing, but PolicyKit should be around on most distros. */
#[test]
fn test_get_policykit_version() {
    use super::BusType;
    let c = Connection::get_private(BusType::System).unwrap();
    let p = Props::new(&c, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority", 10000);

    /* Let's use both the get and getall methods and see if we get the same result */
    let v = p.get("BackendVersion").unwrap();
    let vall = p.get_all().unwrap();
    let v2 = vall.get("BackendVersion").unwrap();

    assert_eq!(&v, &*v2);
    match v {
        MessageItem::Str(ref s) => { println!("Policykit Backend version is {}", s); }
        _ => { panic!("Invalid Get: {:?}", v); }
    };
}

