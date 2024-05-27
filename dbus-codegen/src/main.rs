mod generate;

use crate::generate::{ServerAccess, ConnectionType};

#[cfg(feature = "dbus")]
mod connect_to_dbus {

    use dbus::blocking;

    // This code was copy-pasted from the output of this program. :-)
    pub trait OrgFreedesktopDBusIntrospectable {
        fn introspect(&self) -> Result<String, dbus::Error>;
    }

    impl<'a, C: ::std::ops::Deref<Target=blocking::Connection>> OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C> {

        fn introspect(&self) -> Result<String, dbus::Error> {
            self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
                .and_then(|r: (String, )| Ok(r.0, ))
        }
    }

    pub fn do_introspect(dest: &str, path: &str, systembus: bool) -> String {
        let c = if systembus { blocking::Connection::new_system() } else { blocking::Connection::new_session() };
        let c = c.unwrap();
        let p = c.with_proxy(dest, path, std::time::Duration::from_secs(10));
        p.introspect().unwrap()
    }
}

// Unwrapping is fine here, this is just a test program.

fn main() {
    let app = clap::Command::new("D-Bus Rust code generator").about("Generates Rust code from xml introspection data")
        .arg(clap::Arg::new("interfaces").short('f').long("interfaces").value_name("FILTER")
            .help("Comma separated list of filter strings. Only matching interfaces are generated if set."))
        .arg(clap::Arg::new("genericvariant").short('g').long("generic-variant").action(clap::ArgAction::SetTrue)
             .help("If present, will try to make variant arguments generic instead of Variant<Box<dyn RefArg>>. \
Experimental, does not work with dbus-tree."))
        .arg(clap::Arg::new("methodtype").short('m').long("methodtype").value_name("Fn")
             .help("Type of server method for dbus-tree; valid values are: 'Fn', 'FnMut', 'Sync', 'Generic', and 'None'. Defaults to 'None'."))
        .arg(clap::Arg::new("methodaccess").short('a').long("methodaccess").value_name("RefClosure")
             .help("Specifies how to access the type implementing the interface for dbus-tree (experimental). Valid values are: 'RefClosure', 'AsRefClosure', 'MethodInfo'. \
Defaults to 'RefClosure'."))
        .arg(clap::Arg::new("dbuscrate").long("dbuscrate").value_name("dbus")
             .help("Name of dbus crate, defaults to 'dbus'."))
        .arg(clap::Arg::new("skipprefix").short('i').long("skipprefix").value_name("PREFIX")
             .help("If present, skips a specific prefix for interface names, e g 'org.freedesktop.DBus.'."))
        .arg(clap::Arg::new("client").short('c').long("client").value_name("client")
             .help("Type of client connection. Valid values are: 'blocking', 'nonblock', 'ffidisp'."))
        .arg(clap::Arg::new("propnewtype").short('n').long("prop-newtype").action(clap::ArgAction::SetTrue)
             .help("If present, will generate a struct wrapping PropMap to get properties from it with their expected types."))
        .arg(clap::Arg::new("crossroads").short('r').long("crossroads").action(clap::ArgAction::SetTrue)
            .help("Generate dbus-crossroads server code."))
        .arg(clap::Arg::new("output").short('o').long("output").value_name("FILE")
             .help("Write output into the specified file"))
        .arg(clap::Arg::new("file").long("file").required(false).value_name("FILE")
            .help("D-Bus XML Introspection file"));

    #[cfg(feature = "dbus")]
    let app = app
        .arg(clap::Arg::new("destination").short('d').long("destination").value_name("BUSNAME")
             .help("If present, connects to the supplied service to get introspection data. Reads from stdin otherwise."))
        .arg(clap::Arg::new("path").short('p').long("path").value_name("PATH")
             .help("The path to ask for introspection data. Defaults to '/'. (Ignored if destination is not specified.)"))
        .arg(clap::Arg::new("systembus").short('s').long("system-bus").action(clap::ArgAction::SetTrue)
             .help("Connects to system bus, if not specified, the session bus will be used. (Ignored if destination is not specified.)"));

    let matches = app.get_matches();

    let s = match (matches.get_one::<String>("destination"), matches.get_one::<String>("file")) {
        (Some(_), Some(_)) => panic!("'destination' and 'file' are mutually exclusive arguments - you can't provide both"),
        (None, Some(file_path)) => std::fs::read_to_string(file_path.clone()).unwrap(),
        #[cfg(feature = "dbus")]
        (Some(dest), None) => {
            let path = matches.get_one::<String>("path").map(|s| &**s).unwrap_or("/");
            connect_to_dbus::do_introspect(dest, path, matches.get_flag("systembus"))
        },
        #[cfg(not(feature = "dbus"))]
        (Some(_), None) => unreachable!(),
        (None, None) => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(),&mut s).unwrap();
            s
        }
    };

    let dbuscrate = matches.get_one::<String>("dbuscrate").map(|s| &**s).unwrap_or("dbus");

    let mtype = matches.get_one::<String>("methodtype").map(|s| s.to_lowercase());
    let mtype = match mtype.as_ref().map(|s| &**s) {
        Some("fn") => Some("MTFn"),
        Some("fnmut") => Some("MTFnMut"),
        Some("sync") => Some("MTSync"),
        Some("generic") => Some("MethodType"),
        None | Some("none") => None,
        _ => panic!("Invalid methodtype specified"),
    };

    let maccess = matches.get_one::<String>("methodaccess").map(|s| s.to_lowercase());
    let maccess = match maccess.as_ref().map(|s| &**s) {
        None | Some("refclosure") => ServerAccess::RefClosure,
        Some("asrefclosure") => ServerAccess::AsRefClosure,
        Some("methodinfo") => ServerAccess::MethodInfo,
        _ => panic!("Invalid methodaccess specified"),
    };

    let client = matches.get_one::<String>("client").map(|s| s.to_lowercase());
    let client = match client.as_ref().map(|s| &**s) {
        None | Some("blocking") => ConnectionType::Blocking,
        Some("nonblock") => ConnectionType::Nonblock,
        Some("ffidisp") => ConnectionType::Ffidisp,
        _ => panic!("Invalid client connection type specified"),
    };

    let interfaces = matches.get_one::<String>("interfaces").map(|s| s.split(",").map(|e| e.trim().to_owned()).collect());

    let opts = generate::GenOpts {
        methodtype: mtype.map(|x| x.into()),
        dbuscrate: dbuscrate.into(),
        skipprefix: matches.get_one::<String>("skipprefix").map(|x| x.into()),
        serveraccess: maccess,
        genericvariant: matches.get_flag("genericvariant"),
        connectiontype: client,
        propnewtype: matches.get_flag("propnewtype"),
        crossroads: matches.get_flag("crossroads"),
        interfaces,
        command_line: std::env::args().skip(1).collect::<Vec<String>>().join(" ")
    };

    let mut h: Box<dyn std::io::Write> = match matches.get_one::<String>("output") {
        Some(file_path) => Box::new(std::fs::File::create(file_path)
            .unwrap_or_else(|e| {
                panic!("Failed to open {}", e);
            })),
        None => Box::new(std::io::stdout()),
    };

    h.write(generate::generate(&s, &opts).unwrap().as_bytes()).unwrap();
    h.flush().unwrap();
}
