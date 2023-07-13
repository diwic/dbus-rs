
use std::{io, error};
use std::collections::{HashSet, HashMap};
use xml;

fn find_attr<'a>(a: &'a Vec<xml::attribute::OwnedAttribute>, n: &str) -> Result<&'a str, Box<dyn error::Error>> {
    a.into_iter()
        .find(|q| q.name.prefix.is_none() && q.name.local_name == n)
        .map(|f| &*f.value)
        .ok_or_else(|| format!("attribute not found: {:?}", n).into())
}

/// Server access code generation option
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServerAccess {
    /// Supply a closure from ref to ref
    RefClosure,
    /// Supply a closure from ref to owned object which asrefs
    AsRefClosure,
    /// The interface is implemented for MethodInfo
    MethodInfo
}


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ConnectionType {
    Ffidisp,
    Blocking,
    Nonblock,
}

/// Code generation options
#[derive(Clone, Debug)]
pub struct GenOpts {
    /// Name of dbus crate (used for import)
    pub dbuscrate: String,
    /// MethodType for dbus-tree impl, set to none for client impl only
    pub methodtype: Option<String>,
    /// Generate dbus-crossroads server implementation
    pub crossroads: bool,
    /// Removes a prefix from interface names
    pub skipprefix: Option<String>,
    /// Type of server access (tree)
    pub serveraccess: ServerAccess,
    /// Tries to make variants generic instead of Variant<Box<Refarg>>
    pub genericvariant: bool,
    /// Type of connection, for client only
    pub connectiontype: ConnectionType,
    /// Generates a struct wrapping PropMap to get properties from it with their expected types.
    pub propnewtype: bool,
    /// interface filter. Only matching interface are generated, if non-empty.
    pub interfaces: Option<HashSet<String>>,
    /// The command line argument string. This will be inserted into generated source files.
    pub command_line: String,
}

impl ::std::default::Default for GenOpts {
    fn default() -> Self { GenOpts {
        dbuscrate: "dbus".into(), methodtype: Some("MTFn".into()), skipprefix: None,
        serveraccess: ServerAccess::RefClosure, genericvariant: false,
        connectiontype: ConnectionType::Blocking, propnewtype: false,
        interfaces: None, crossroads: false,
        command_line: String::new()
    }}
}

mod types;

mod write;

use types::*;


/// Generates Rust structs and traits from D-Bus XML introspection data.
pub fn generate(xmldata: &str, opts: &GenOpts) -> Result<String, Box<dyn error::Error>> {
    use xml::EventReader;
    use xml::reader::XmlEvent;

    let mut s = String::new();
    write::module_header(&mut s, opts);
    let mut curintf = None;
    let mut curm = None;
    let mut cursig = None;
    let mut curprop = None;
    let mut curarg = None;
    let parser = EventReader::new(io::Cursor::new(xmldata));
    for e in parser {
        match e? {
            XmlEvent::StartElement { ref name, .. } if name.prefix.is_some() => (),
            XmlEvent::EndElement { ref name, .. } if name.prefix.is_some() => (),
            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "interface" => {
                if curm.is_some() { Err("Start of Interface inside method")? };
                if curintf.is_some() { Err("Start of Interface inside interface")? };
                let n = find_attr(attributes, "name")?;
                let mut n2 = n;
                if let &Some(ref p) = &opts.skipprefix {
                    if n.len() > p.len() && n.starts_with(p) { n2 = &n[p.len()..]; }
                }
                curintf = Some(Intf { origname: n.into(), shortname: n2.into(),
                    methods: Vec::new(), signals: Vec::new(), props: Vec::new(), annotations: HashMap::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "interface" => {
                if curm.is_some() { Err("End of Interface inside method")? };
                if curintf.is_none() { Err("End of Interface outside interface")? };
                let intf = curintf.take().unwrap();
                // If filters are set and no filter matches -> Just print a message and continue parsing
                if let Some(filter) = &opts.interfaces {
                    if !filter.contains(&intf.shortname) && !filter.contains(&intf.origname) {
                        eprintln!("Skip filtered interface '{}'", &intf.shortname);
                        continue;
                    }
                }
                write::intf(&mut s, &intf, opts)?;
                write::signals(&mut s, &intf)?;
                if opts.propnewtype {
                    write::intf_name(&mut s, &intf)?;
                    write::prop_struct(&mut s, &intf)?;
                }
                if opts.crossroads {
                    write::intf_cr(&mut s, &intf)?;
                }
                if let Some(ref mt) = opts.methodtype {
                    write::intf_tree(&mut s, &intf, &mt, opts.serveraccess, opts.genericvariant)?;
                } else if !opts.crossroads {
                    write::intf_client(&mut s, &intf, opts)?;
                }
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                if curm.is_some() { Err("Start of method inside method")? };
                if curintf.is_none() { Err("Start of method outside interface")? };
                let name = find_attr(attributes, "name")?;
                curm = Some(Method { name: name.into(), fn_name: make_fn_name(curintf.as_ref().unwrap(), name),
                    iargs: Vec::new(), oargs: Vec::new(), annotations: HashMap::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "method" => {
                if curm.is_none() { Err("End of method outside method")? };
                if curintf.is_none() { Err("End of method outside interface")? };
                curintf.as_mut().unwrap().methods.push(curm.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "signal" => {
                if cursig.is_some() { Err("Start of signal inside signal")? };
                if curintf.is_none() { Err("Start of signal outside interface")? };
                cursig = Some(Signal { name: find_attr(attributes, "name")?.into(), args: Vec::new(), annotations: HashMap::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "signal" => {
                if cursig.is_none() { Err("End of signal outside signal")? };
                if curintf.is_none() { Err("End of signal outside interface")? };
                curintf.as_mut().unwrap().signals.push(cursig.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "property" => {
                if curprop.is_some() { Err("Start of property inside property")? };
                if curintf.is_none() { Err("Start of property outside interface")? };
                let name = find_attr(attributes, "name")?;
                let get_fn_name = make_fn_name(curintf.as_ref().unwrap(), name);
                let set_fn_name = make_fn_name(curintf.as_ref().unwrap(), &format!("Set{}", name));
                curprop = Some(Prop {
                    name: name.into(),
                    typ: find_attr(attributes, "type")?.into(),
                    access: find_attr(attributes, "access")?.into(),
                    get_fn_name: get_fn_name,
                    set_fn_name: set_fn_name,
                    annotations: HashMap::new(),
                });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "property" => {
                if curprop.is_none() { Err("End of property outside property")? };
                if curintf.is_none() { Err("End of property outside interface")? };
                curintf.as_mut().unwrap().props.push(curprop.take().unwrap());
            }


            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "arg" => {
                if curm.is_none() && cursig.is_none() { Err("Start of arg outside method and signal")? };
                if curintf.is_none() { Err("Start of arg outside interface")? };
                let typ = find_attr(attributes, "type")?.into();
                let is_out = match find_attr(attributes, "direction") {
                    Err(_) => false,
                    Ok("in") => false,
                    Ok("out") => true,
                    _ => { Err("Invalid direction")?; unreachable!() }
                };
                let no_refs = is_out || cursig.is_some() || opts.crossroads;
                curarg = Some(Arg { name: find_attr(attributes, "name").unwrap_or("").into(),
                    typ: typ, no_refs, idx: 0, is_out, annotations: HashMap::new() });
            }

            XmlEvent::EndElement { ref name } if &name.local_name == "arg" => {
                if curarg.is_none() { Err("End of arg outside arg")? };
                let arg = curarg.as_mut().unwrap();
                let arr = if let Some(ref mut sig) = cursig { &mut sig.args }
                else if arg.is_out { &mut curm.as_mut().unwrap().oargs } else { &mut curm.as_mut().unwrap().iargs };
                arg.idx = arr.len() as i32;
                arr.push(curarg.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, ..} if &name.local_name == "annotation" => {
                if let Ok(key) = find_attr(attributes, "name") {
                    if let Ok(value) = find_attr(attributes, "value") {
                        if let Some(ref mut arg) = curarg { arg.annotations.insert(key.into(), value.into()); }
                        else if let Some(ref mut sig) = cursig { sig.annotations.insert(key.into(), value.into()); }
                        else if let Some(ref mut prop) = curprop { prop.annotations.insert(key.into(), value.into()); }
                        else if let Some(ref mut met) = curm { met.annotations.insert(key.into(), value.into()); }
                        else if let Some(ref mut intf) = curintf { intf.annotations.insert(key.into(), value.into()); }
                    }
                }
            }
            _ => (),
        }
    }
    if curintf.is_some() { Err("Unterminated interface")? }
    Ok(s)
}

#[cfg(test)]
mod tests {

use super::{generate, GenOpts};

static FROM_DBUS: &'static str = r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
"http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.DBus">
    <method name="Hello">
      <arg direction="out" type="s"/>
    </method>
    <method name="RequestName">
      <arg direction="in" type="s"/>
      <arg direction="in" type="u"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="ReleaseName">
      <arg direction="in" type="s"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="StartServiceByName">
      <arg direction="in" type="s"/>
      <arg direction="in" type="u"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="UpdateActivationEnvironment">
      <arg direction="in" type="a{ss}"/>
    </method>
    <method name="NameHasOwner">
      <arg direction="in" type="s"/>
      <arg direction="out" type="b"/>
    </method>
    <method name="ListNames">
      <arg direction="out" type="as"/>
    </method>
    <method name="ListActivatableNames">
      <arg direction="out" type="as"/>
    </method>
    <method name="AddMatch">
      <arg direction="in" type="s"/>
    </method>
    <method name="RemoveMatch">
      <arg direction="in" type="s"/>
    </method>
    <method name="GetNameOwner">
      <arg direction="in" type="s"/>
      <arg direction="out" type="s"/>
    </method>
    <method name="ListQueuedOwners">
      <arg direction="in" type="s"/>
      <arg direction="out" type="as"/>
    </method>
    <method name="GetConnectionUnixUser">
      <arg direction="in" type="s"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="GetConnectionUnixProcessID">
      <arg direction="in" type="s"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="GetAdtAuditSessionData">
      <arg direction="in" type="s"/>
      <arg direction="out" type="ay"/>
    </method>
    <method name="GetConnectionSELinuxSecurityContext">
      <arg direction="in" type="s"/>
      <arg direction="out" type="ay"/>
    </method>
    <method name="GetConnectionAppArmorSecurityContext">
      <arg direction="in" type="s"/>
      <arg direction="out" type="s"/>
    </method>
    <method name="ReloadConfig">
    </method>
    <method name="GetId">
      <arg direction="out" type="s"/>
    </method>
    <method name="GetConnectionCredentials">
      <arg direction="in" type="s"/>
      <arg direction="out" type="a{sv}"/>
    </method>
    <signal name="NameOwnerChanged">
      <arg type="s"/>
      <arg type="s"/>
      <arg type="s"/>
    </signal>
    <signal name="NameLost">
      <arg type="s"/>
    </signal>
    <signal name="NameAcquired">
      <arg type="s"/>
    </signal>
  </interface>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg direction="out" type="s"/>
    </method>
  </interface>
  <interface name="org.freedesktop.DBus.Monitoring">
    <method name="BecomeMonitor">
      <arg direction="in" type="as"/>
      <arg direction="in" type="u"/>
    </method>
  </interface>
  <interface name="org.freedesktop.DBus.Debug.Stats">
    <method name="GetStats">
      <arg direction="out" type="a{sv}"/>
    </method>
    <method name="GetConnectionStats">
      <arg direction="in" type="s"/>
      <arg direction="out" type="a{sv}"/>
    </method>
    <method name="GetAllMatchRules">
      <arg direction="out" type="a{sas}"/>
    </method>
  </interface>
</node>
"#;

    #[test]
    fn from_dbus() {
        let s = generate(FROM_DBUS, &GenOpts { methodtype: Some("MTSync".into()), ..Default::default() }).unwrap();
        println!("{}", s);
        //assert_eq!(s, "fdjsf");
    }
}
