use super::{Connection, Message, MessageItem, Error};
use std::collections::BTreeMap;

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
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
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
       return Err(Error::new_custom("InvalidReply", f.as_slice()));
    }

    pub fn set(&self, propname: &str, value: MessageItem) -> Result<(), Error> {
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
            "org.freedesktop.DBus.Properties", "Set").unwrap();
        m.append_items(&[
            MessageItem::Str(self.interface.clone()),
            MessageItem::Str(propname.to_string()),
            MessageItem::Variant(box value),
        ]);
        let mut r = try!(self.conn.send_with_reply_and_block(m, self.timeout_ms));
        try!(r.as_result());
        Ok(())
    }

    pub fn get_all(&self) -> Result<BTreeMap<String, MessageItem>, Error> {
        let mut m = Message::new_method_call(self.name.as_slice(), self.path.as_slice(),
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
                            t.insert(ks.to_string(), (**v).clone());
                        } else { haserr = true; };
                    } else { haserr = true; };
                }
                if !haserr { return Ok(t) };
            }
        }
        let f = format!("Invalid reply for property GetAll: '{:?}'", reply);
        return Err(Error::new_custom("InvalidReply", f.as_slice()));
    }
}

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

    fn invalid_args(m: &Message) -> Message {
        Message::new_error(m, "org.freedesktop.DBus.Error.InvalidArgs", "Invalid arguments").unwrap()
    }

    fn handle_get(&self, msg: &mut Message) -> Message {
        let items = msg.get_items();
        let name = if let Some(s) = items.get(1) { s } else { return PropHandler::invalid_args(msg) };
        let name = if let &MessageItem::Str(ref s) = name { s } else { return PropHandler::invalid_args(msg) };
        let value = if let Some(s) = self.map.get(name) { s } else { return PropHandler::invalid_args(msg) };

        let mut reply = Message::new_method_return(msg).unwrap();
        reply.append_items(&[MessageItem::Variant(box value.clone())]);
        reply
    }

    fn handle_getall(&self, msg: &mut Message) -> Message {
        let mut reply = Message::new_method_return(msg).unwrap();
        for (k, v) in self.map.iter() {
            reply.append_items(&[MessageItem::DictEntry(box MessageItem::Str(k.clone()), box v.clone())]);
        }
        reply
    }

    /* Return value:
       None => not handled,
       Some(Err(())) => message reply send failed,
       Some(Ok()) => message reply send ok */
    pub fn handle_message(&mut self, msg: &mut Message) -> Option<Result<(), ()>> {
        let (_, path, iface, method) = msg.headers();
        if iface.is_none() || iface.unwrap().as_slice() != "org.freedesktop.DBus.Properties" { return None; }
        if path.is_none() || path.unwrap() != self.p.path { return None; }
        if method.is_none() { return None; }

        let items = msg.get_items();
        if let Some(i) = items.get(0) {
            if let &MessageItem::Str(ref s) = i {
                if *s != self.p.interface { return None; }
            } else { return None; } // Hmm, invalid message
        } else { return None }; // Hmm, invalid message

        // Ok, we have a match
        let reply = match method.unwrap().as_slice() {
            "Get" => self.handle_get(msg),
//            "Set" => self.handle_set(msg),
            "GetAll" => self.handle_getall(msg),
            _ => PropHandler::invalid_args(msg)
        };
        Some(self.p.conn.send(reply))
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

    let v2 = match vall.get("BackendVersion").unwrap() {
        &MessageItem::Variant(ref q) => &**q,
        _ => { panic!("Invalid GetAll: {:?}", vall); }
    };

    assert_eq!(&v, &*v2);
    match v {
        MessageItem::Str(ref s) => { println!("Policykit Backend version is {}", s); }
        _ => { panic!("Invalid Get: {:?}", v); }
    };
    
}

#[test]
fn test_prop_server() {
    let c = Connection::get_private(super::BusType::Session).unwrap();
    let busname = format!("com.example.prophandler.test{}", ::std::rand::random::<u32>());
    assert_eq!(c.register_name(busname.as_slice(), super::NameFlag::ReplaceExisting as u32).unwrap(), super::RequestNameReply::PrimaryOwner);

    let mut p = PropHandler::new(Props::new(&c, &*busname, "/propserver", &*busname, 5000));
    c.register_object_path("/propserver").unwrap();
    p.map_mut().insert("Foo".to_string(), super::MessageItem::Int16(-15));

    let thread = ::std::thread::Thread::scoped(move || {
        let c = Connection::get_private(super::BusType::Session).unwrap();
        let mut pr = PropHandler::new(Props::new(&c, &*busname, "/propserver", &*busname, 5000));
        assert_eq!(pr.get("Foo").unwrap(), &super::MessageItem::Int16(-15));
    });

    loop {
        let n = match c.iter(1000).next() {
            None => panic!("c.iter.next returned None"),
            Some(n) => n,
        };
        if let super::ConnectionItem::MethodCall(mut msg) = n {
            let q = p.handle_message(&mut msg);
            if q.is_none() {
                println!("Non-matching message {:?}", msg);
                c.send(super::Message::new_error(&msg, "org.freedesktop.DBus.Error.UnknownMethod", "Unknown method").unwrap()).unwrap();
                continue;
            }
            assert_eq!(q, Some(Ok(())));
            break;
        }
    }
    thread.join().ok().expect("failed to join thread");
}
