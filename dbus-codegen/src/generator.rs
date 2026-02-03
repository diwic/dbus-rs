mod definitions;
mod types;
mod write;

use std::collections::{HashMap, HashSet};
use std::{
    error::Error,
    io::{Cursor, Read, Write},
};
use xml;

pub use definitions::*;
use types::*;

/// Server access code generation option
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServerAccess {
    /// Supply a closure from ref to ref
    RefClosure,
    /// Supply a closure from ref to owned object which asrefs
    AsRefClosure,
    /// The interface is implemented for MethodInfo
    MethodInfo,
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
    fn default() -> Self {
        GenOpts {
            dbuscrate: "dbus".into(),
            methodtype: Some("MTFn".into()),
            skipprefix: None,
            serveraccess: ServerAccess::RefClosure,
            genericvariant: false,
            connectiontype: ConnectionType::Blocking,
            propnewtype: false,
            interfaces: None,
            crossroads: false,
            command_line: String::new(),
        }
    }
}

fn find_attr<'a>(a: &'a Vec<xml::attribute::OwnedAttribute>, n: &str) -> Result<&'a str, Box<dyn Error>> {
    a.into_iter()
        .find(|q| q.name.prefix.is_none() && q.name.local_name == n)
        .map(|f| &*f.value)
        .ok_or_else(|| format!("attribute not found: {:?}", n).into())
}

fn make_arguments_from_definitions<'a>(arguments: impl Iterator<Item = &'a ArgumentDefinition>, no_refs: bool) -> Vec<Arg<'a>> {
    arguments
        .enumerate()
        .map(|(idx, a)| Arg { name: a.get_name(), typ: a.get_type(), idx: idx as i32, no_refs: no_refs, annotations: a.get_annotations() })
        .collect()
}

#[cfg(feature = "dbus")]
mod connect_to_dbus {

    use dbus::blocking;

    // This code was copy-pasted from the output of this program. :-)
    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, C: ::std::ops::Deref<Target = blocking::Connection>> OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C> {
        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ()).and_then(|r: (String,)| Ok(r.0))
        }
    }

    pub fn do_introspect(dest: &str, path: &str, systembus: bool) -> String {
        let c = if systembus { blocking::Connection::new_system() } else { blocking::Connection::new_session() };
        let c = c.unwrap();
        let p = c.with_proxy(dest, path, std::time::Duration::from_secs(10));
        p.introspect().unwrap()
    }
}

/// The code generator.
///
/// The code generator is initialized with D-Bus interface definitions, and can output Rust code
/// implementing clients and/or servers for those interfaces. The `from_xxx` methods are the various
/// ways to create a generator. The generator can indeed be created from D-Bus introspection data,
/// from an input stream, from a string, or from pre-parsed definitions.
///
/// The generator can generate the code either to a string or directly to an output stream. The same generator
/// can be used multiple times with different options.
///
/// # Examples
///
/// ```no_run
/// use dbus_codegen::Generator;
/// use dbus_codegen::GenOpts;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a generator from D-Bus introspection
/// let gen = Generator::from_dbus("org.freedesktop.DBus", "/", false)?;
/// // Create generation options
/// let opts = GenOpts {
///     methodtype: None,
///     ..Default::default()
/// };
/// // Output code to console
/// let code = gen.output_to_string(&opts)?;
/// println!("{}", code);
/// // Generate proxy and server code to a file
/// let mut file = std::fs::File::create("dbus_proxy.rs")?;
/// gen.output_to_stream(&opts, &mut file)?;
/// let mut file = std::fs::File::create("dbus_server.rs")?;
/// gen.output_to_stream(&GenOpts { crossroads: true, ..opts }, &mut file)?;
/// # Ok(())
/// # }
/// ```
pub struct Generator {
    interfaces: Vec<InterfaceDefinition>,
}

impl Generator {
    /// Create a new generator with interfaces taken from the D-Bus introspection.
    ///
    /// # Arguments
    ///
    /// * `dest` - The destination (bus name) to introspect.
    /// * `path` - The object path to introspect.
    /// * `systembus` - Whether to use the system bus (true) or session bus (false).
    ///
    /// # Returns
    ///
    /// A Result containing the Generator with the interfaces, or an error.
    #[cfg(feature = "dbus")]
    pub fn from_dbus(dest: &str, path: &str, systembus: bool) -> Result<Self, Box<dyn Error>> {
        let xmldata = connect_to_dbus::do_introspect(dest, path, systembus);
        Self::from_string(&xmldata)
    }

    /// Create a new Generator with interfaces taken from the given input stream.
    ///
    /// # Arguments
    ///
    /// * `input` - The input stream to read the XML data from.
    ///
    /// # Returns
    ///
    /// A Result containing the Generator with the interfaces, or an error.
    pub fn from_stream(input: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let mut xmldata = String::new();
        input.read_to_string(&mut xmldata)?;
        Self::from_string(&xmldata)
    }

    /// Create a new Generator with interfaces taken from the given XML string.
    ///
    /// # Arguments
    ///
    /// * `xmldata` - The XML string to parse.
    ///
    /// # Returns
    ///
    /// A Result containing the Generator with the interfaces, or an error.
    pub fn from_string(xmldata: &str) -> Result<Self, Box<dyn Error>> {
        use xml::reader::XmlEvent;
        use xml::EventReader;

        let mut interfaces = Vec::new();
        let mut curintf = None;
        let mut curm = None;
        let mut cursig = None;
        let mut curprop = None;
        let mut curarg = None;
        let mut curarg_is_out = false;
        let parser = EventReader::new(Cursor::new(xmldata));
        for e in parser {
            match e? {
                XmlEvent::StartElement { ref name, .. } if name.prefix.is_some() => (),
                XmlEvent::EndElement { ref name, .. } if name.prefix.is_some() => (),
                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "interface" => {
                    if curm.is_some() {
                        Err("Start of Interface inside method")?
                    };
                    if curintf.is_some() {
                        Err("Start of Interface inside interface")?
                    };
                    curintf = Some(InterfaceDefinition::new(find_attr(attributes, "name")?.into()));
                }
                XmlEvent::EndElement { ref name } if &name.local_name == "interface" => {
                    if curm.is_some() {
                        Err("End of Interface inside method")?
                    };
                    if curintf.is_none() {
                        Err("End of Interface outside interface")?
                    };
                    interfaces.push(curintf.take().unwrap());
                }

                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                    if curm.is_some() {
                        Err("Start of method inside method")?
                    };
                    if curintf.is_none() {
                        Err("Start of method outside interface")?
                    };
                    curm = Some(MethodDefinition::new(find_attr(attributes, "name")?.into()));
                }
                XmlEvent::EndElement { ref name } if &name.local_name == "method" => {
                    if curm.is_none() {
                        Err("End of method outside method")?
                    };
                    if curintf.is_none() {
                        Err("End of method outside interface")?
                    };
                    curintf.as_mut().unwrap().add_method(curm.take().unwrap());
                }

                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "signal" => {
                    if cursig.is_some() {
                        Err("Start of signal inside signal")?
                    };
                    if curintf.is_none() {
                        Err("Start of signal outside interface")?
                    };
                    cursig = Some(SignalDefinition::new(find_attr(attributes, "name")?.into()));
                }
                XmlEvent::EndElement { ref name } if &name.local_name == "signal" => {
                    if cursig.is_none() {
                        Err("End of signal outside signal")?
                    };
                    if curintf.is_none() {
                        Err("End of signal outside interface")?
                    };
                    curintf.as_mut().unwrap().add_signal(cursig.take().unwrap());
                }

                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "property" => {
                    if curprop.is_some() {
                        Err("Start of property inside property")?
                    };
                    if curintf.is_none() {
                        Err("Start of property outside interface")?
                    };
                    curprop = Some(PropertyDefinition::new(
                        find_attr(attributes, "name")?.into(),
                        find_attr(attributes, "type")?.into(),
                        find_attr(attributes, "access")?.into(),
                    ));
                }
                XmlEvent::EndElement { ref name } if &name.local_name == "property" => {
                    if curprop.is_none() {
                        Err("End of property outside property")?
                    };
                    if curintf.is_none() {
                        Err("End of property outside interface")?
                    };
                    curintf.as_mut().unwrap().add_property(curprop.take().unwrap());
                }

                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "arg" => {
                    if curm.is_none() && cursig.is_none() {
                        Err("Start of arg outside method and signal")?
                    };
                    if curintf.is_none() {
                        Err("Start of arg outside interface")?
                    };
                    curarg_is_out = match find_attr(attributes, "direction") {
                        Err(_) => false,
                        Ok("in") => false,
                        Ok("out") => true,
                        _ => {
                            Err("Invalid direction")?;
                            unreachable!()
                        }
                    };
                    curarg = Some(ArgumentDefinition::new(
                        find_attr(attributes, "name").unwrap_or("").into(),
                        find_attr(attributes, "type")?.into(),
                    ));
                }
                XmlEvent::EndElement { ref name } if &name.local_name == "arg" => {
                    if curarg.is_none() {
                        Err("End of arg outside arg")?
                    };
                    let arg = curarg.take().unwrap();
                    if let Some(ref mut sig) = cursig {
                        sig.add_arg(arg);
                    } else if curarg_is_out {
                        curm.as_mut().unwrap().add_output_arg(arg);
                    } else {
                        curm.as_mut().unwrap().add_input_arg(arg);
                    }
                }

                XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "annotation" => {
                    if let Ok(key) = find_attr(attributes, "name") {
                        if let Ok(value) = find_attr(attributes, "value") {
                            if let Some(ref mut arg) = curarg {
                                arg.add_annotation(key.into(), value.into());
                            } else if let Some(ref mut sig) = cursig {
                                sig.add_annotation(key.into(), value.into());
                            } else if let Some(ref mut prop) = curprop {
                                prop.add_annotation(key.into(), value.into());
                            } else if let Some(ref mut met) = curm {
                                met.add_annotation(key.into(), value.into());
                            } else if let Some(ref mut intf) = curintf {
                                intf.add_annotation(key.into(), value.into());
                            }
                        }
                    }
                }
                _ => (),
            }
        }
        if curintf.is_some() {
            Err("Unterminated interface")?
        }
        Self::from_definition(interfaces)
    }

    /// Create a new Generator with interfaces taken from the given definitions.
    ///
    /// # Arguments
    ///
    /// * `interfaces` - The interfaces to generate code for.
    ///
    /// # Returns
    ///
    /// A Result containing the Generator with the interfaces, or an error.
    pub fn from_definition(interfaces: Vec<InterfaceDefinition>) -> Result<Self, Box<dyn Error>> {
        Ok(Self { interfaces })
    }

    /// Output to a string.
    ///
    /// # Arguments
    ///
    /// * `opts` - The generation options.
    pub fn output_to_string(&self, opts: &GenOpts) -> Result<String, Box<dyn Error>> {
        let mut output = Vec::new();
        self.output_to_stream(opts, &mut output)?;
        Ok(String::from_utf8(output)?)
    }

    /// Output to the given stream.
    ///
    /// # Arguments
    ///
    /// * `opts` - The generation options.
    /// * `output` - The output stream to write the generated code to.
    pub fn output_to_stream(&self, opts: &GenOpts, output: &mut dyn Write) -> Result<(), Box<dyn Error>> {
        let mut s = write::Stream(output);
        write::module_header(&mut s, opts);
        for intf_def in &self.interfaces {
            // Create empty interface
            let mut n = intf_def.get_name();
            if let &Some(ref p) = &opts.skipprefix {
                if n.len() > p.len() && n.starts_with(p) {
                    n = &n[p.len()..];
                }
            }
            if let Some(filter) = &opts.interfaces {
                if !filter.contains(n) && !filter.contains(intf_def.get_name()) {
                    eprintln!("Skip filtered interface '{}'", n);
                    continue;
                }
            }
            let mut intf = Intf {
                origname: intf_def.get_name(),
                shortname: n,
                methods: Vec::new(),
                signals: Vec::new(),
                props: Vec::new(),
                annotations: intf_def.get_annotations(),
            };

            // Add signals
            for s in intf_def.iter_signals() {
                intf.signals.push(Signal {
                    name: s.get_name(),
                    args: make_arguments_from_definitions(s.iter_args(), true),
                    annotations: s.get_annotations(),
                });
            }

            // Add methods
            for m in intf_def.iter_methods() {
                intf.methods.push(Method {
                    name: m.get_name(),
                    fn_name: make_fn_name(&intf, m.get_name()),
                    iargs: make_arguments_from_definitions(m.iter_input_args(), opts.crossroads),
                    oargs: make_arguments_from_definitions(m.iter_output_args(), true),
                    annotations: m.get_annotations(),
                });
            }

            // Add properties
            for p in intf_def.iter_properties() {
                intf.props.push(Prop {
                    name: p.get_name(),
                    typ: p.get_type(),
                    access: p.get_access(),
                    get_fn_name: make_fn_name(&intf, p.get_name()),
                    set_fn_name: make_fn_name(&intf, &format!("Set{}", p.get_name())),
                    annotations: p.get_annotations(),
                });
            }

            // Write interface
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

        // Terminate
        output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::{GenOpts, Generator};

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
        let s = Generator::from_string(FROM_DBUS)
            .unwrap()
            .output_to_string(&GenOpts { methodtype: Some("MTSync".into()), ..Default::default() })
            .unwrap();
        println!("{}", s);
        //assert_eq!(s, "fdjsf");
    }
}
