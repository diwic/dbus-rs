extern crate xml;
#[cfg(feature = "dbus")]
extern crate dbus;
extern crate structopt;

mod generate;

use std::{
    fmt::Display,
    path::PathBuf,
};

use structopt::StructOpt;
use strum::{IntoEnumIterator, VariantNames};
use strum_macros::{Display, EnumIter, EnumString, EnumVariantNames};

#[cfg(feature = "dbus")]
use dbus::ffidisp::Connection;

use crate::generate::{ServerAccess, ConnectionType};

#[cfg(feature = "dbus")]
// Copy-pasted from the output of this program :-)
pub trait OrgFreedesktopDBusIntrospectable {
    fn introspect(&self) -> Result<String, ::dbus::Error>;
}

#[cfg(feature = "dbus")]
impl<'a, C: ::std::ops::Deref<Target=::dbus::ffidisp::Connection>> OrgFreedesktopDBusIntrospectable for ::dbus::ffidisp::ConnPath<'a, C> {

    fn introspect(&self) -> Result<String, ::dbus::Error> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Introspectable".into(), &"Introspect".into(), |_| {
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let a0: String = i.read()?;
        Ok(a0)
    }
}

#[derive(StructOpt)]
#[structopt(name = "D-Bus Rust code generator", about = "Generates Rust code from xml introspection data")]
struct Args {
    /// If present, connects to the supplied service to get introspection data. Reads from stdin otherwise.
    #[cfg(feature = "dbus")]
    #[structopt(short, long, value_name = "BUSNAME")]
    destination: Option<String>,
    /// The path to ask for introspection data. (Ignored if destination is not specified.)
    #[cfg(feature = "dbus")]
    #[structopt(short, long, default_value = "/", value_name = "PATH")]
    path: String,
    /// Comma separated list of filter strings. Only matching interfaces are generated if set.
    #[structopt(short = "f", long, value_name = "FILTER", use_delimiter = true)]
    interfaces: Option<Vec<String>>,
    /// Connects to system bus, if not specified, the session bus will be used. (Ignored if destination is not specified.)
    #[cfg(feature = "dbus")]
    #[structopt(short, long = "system-bus")]
    systembus: bool,
    /// If present, will try to make variant arguments generic instead of Variant<Box<dyn RefArg>>.
    /// Experimental, does not work with server methods (other than None).
    #[structopt(short, long = "generic-variant")]
    genericvariant: bool,
    /// Type of server method
    #[structopt(
        short,
        long,
        value_name = "Fn",
        default_value = "Fn",
        parse(from_str = lowercase_to_enum),
        possible_values = &MethodType::VARIANTS,
        case_insensitive = true,
    )]
    methodtype: MethodType,
    /// Specifies how to access the type implementing the interface (experimental).
    #[structopt(
        short = "a",
        long,
        value_name = "RefClosure",
        default_value = "RefClosure",
        parse(from_str = lowercase_to_enum),
        possible_values = &ServerAccess::VARIANTS,
        case_insensitive = true,
    )]
    methodaccess: ServerAccess,
    /// Name of dbus crate, defaults to 'dbus'.
    #[structopt(long, value_name = "dbus", default_value = "dbus")]
    dbuscrate: String,
    /// If present, skips a specific prefix for interface names, e g 'org.freedesktop.DBus.'.
    #[structopt(short = "i", long, value_name = "PREFIX")]
    skipprefix: Option<String>,
    // /// Generates code to use with futures 0.3 (experimental)
    // #[structopt(short, long)]
    // futures: bool,
    /// Type of client connection.
    #[structopt(
        short,
        long,
        value_name = "CLIENT",
        default_value = "Blocking",
        parse(from_str = lowercase_to_enum),
        possible_values = &ConnectionType::VARIANTS,
        case_insensitive = true,
    )]
    client: ConnectionType,
    /// Write output into the specified file
    #[structopt(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// D-Bus XML Introspection file
    #[structopt(long, value_name = "FILE")]
    file: Option<PathBuf>,
}

/// This program was previously documented as accepting lowercase strings for enums, so it would be a breaking change
/// to stop accepting them. If any enum variants are named the same but with different casing this will panic.
fn lowercase_to_enum<T: IntoEnumIterator + Display>(src: &str) -> T {
    assert!({
        let mut lower_names = T::iter().map(|s| s.to_string().to_lowercase()).collect::<Vec<_>>();
        lower_names.sort_unstable();
        lower_names.windows(2).all(|w| w[0] != w[1])
    });
    let src = src.to_lowercase();
    for variant in T::iter() {
        if src == variant.to_string().to_lowercase() {
            return variant;
        }
    }
    unreachable!("possible_values from clap will guarantee we never arrive here.")
}

#[derive(Display, EnumIter, EnumString, EnumVariantNames)]
enum MethodType {
    Fn,
    FnMut,
    Sync,
    Generic,
    Par,
    None,
}

// Unwrapping is fine here, this is just a test program.

fn main() {
    let args = Args::from_args();

    #[cfg(feature = "dbus")]
    if args.destination.is_some() && args.file.is_some() {
        panic!("Expected either xml file path as argument or destination option. But both are provided.");
    }
    let mut s = String::new();
    #[cfg(feature = "dbus")]
    {
        if let Some(dest) = args.destination {
            let c = if args.systembus { Connection::new_system() } else { Connection::new_session() };
            let c = c.unwrap();
            let p = c.with_path(dest, args.path, 10000);
            s = p.introspect().unwrap();
        }
    }
    if s == "" {
        s = if let Some(file_path) = args.file  {
            std::fs::read_to_string(&file_path).unwrap()
        } else {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(),&mut s).unwrap();
            s
        };
    }
    let s = s;

    let (mtype, crhandler) = match args.methodtype {
        MethodType::Fn => (Some("MTFn"), None),
        MethodType::FnMut => (Some("MTFnMut"), None),
        MethodType::Sync => (Some("MTSync"), None),
        MethodType::Generic => (Some("MethodType"), None),
        MethodType::Par => (None, Some("Par")),
        MethodType::None => (None, None),
    };

    let opts = generate::GenOpts { methodtype: mtype.map(|x| x.into()), dbuscrate: args.dbuscrate.into(),
        skipprefix: args.skipprefix,
        serveraccess: args.methodaccess,
        genericvariant: args.genericvariant,
        futures: false,
        connectiontype: args.client,
        crhandler: crhandler.map(|x| x.to_string()),
        interfaces: args.interfaces.map(|v| v.into_iter().collect()),
        command_line: std::env::args().skip(1).collect::<Vec<String>>().join(" ")
    };

    let mut h: Box<dyn std::io::Write> = match args.output {
        Some(file_path) => Box::new(std::fs::File::create(file_path)
            .unwrap_or_else(|e| {
                panic!("Failed to open {}", e);
            })),
        None => Box::new(std::io::stdout()),
    };

    h.write(generate::generate(&s, &opts).unwrap().as_bytes()).unwrap();
    h.flush().unwrap();
}
