extern crate xml;
extern crate dbus;
extern crate clap;

mod generate;

// Copy-pasted from the output of this program :-)
pub trait OrgFreedesktopDBusIntrospectable {
    fn introspect(&self) -> Result<String, ::dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=::dbus::Connection>> OrgFreedesktopDBusIntrospectable for ::dbus::ConnPath<'a, C> {

    fn introspect(&self) -> Result<String, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Introspectable".into(), &"Introspect".into(), |_| {
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: String = try!(i.read());
        Ok(a0)
    }
}


// Unwrapping is fine here, this is just a test program.

fn main() {
    let matches = clap::App::new("D-Bus Rust code generator").about("Generates Rust code from xml introspection data")
        .arg(clap::Arg::with_name("destination").short("d").long("destination").takes_value(true).value_name("BUSNAME")
             .help("If present, connects to the supplied service to get introspection data. Reads from stdin otherwise."))  
        .arg(clap::Arg::with_name("path").short("p").long("path").takes_value(true).value_name("PATH")
             .help("The path to ask for introspection data. Defaults to '/'. (Ignored if destination is not specified.)"))
        .arg(clap::Arg::with_name("systembus").short("s").long("system-bus")
             .help("Connects to system bus, if not specified, the session bus will be used. (Ignored if destination is not specified.)"))
        .arg(clap::Arg::with_name("methodtype").short("m").long("methodtype").takes_value(true).value_name("Fn")
             .help("Type of server method; valid values are: 'Fn', 'FnMut', 'Sync', and 'None'. Defaults to 'Fn'."))
        .get_matches();

    let s = 
    if let Some(dest) = matches.value_of("destination") {
        let path = matches.value_of("path").unwrap_or("/");
        let bus = if matches.is_present("systembus") { dbus::BusType::System } else { dbus::BusType::Session };
        let c = dbus::Connection::get_private(bus).unwrap();
        let p = c.with_path(dest, path, 10000);
        p.introspect().unwrap()
    } else {
        let mut s = String::new();
        (&mut std::io::stdin() as &mut std::io::Read).read_to_string(&mut s).unwrap();
        s
    };

    let mtype = matches.value_of("methodtype").map(|s| s.to_lowercase());
    let mtype = match mtype.as_ref().map(|s| &**s) {
        None | Some("fn") => Some("MTFn"),
        Some("fnmut") => Some("MTFnMut"),
        Some("sync") => Some("MTSync"),
        Some("none") => None,
        _ => panic!("Invalid methodtype specified"),
    };

    let mut stdout = std::io::stdout();
    let h: &mut std::io::Write = &mut stdout;
    h.write(generate::generate(&s, mtype).unwrap().as_bytes()).unwrap();
    h.flush().unwrap();
}
