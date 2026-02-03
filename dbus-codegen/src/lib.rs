extern crate xml;

mod generator;

pub use crate::generator::{
    ArgumentDefinition, ConnectionType, GenOpts, Generator, InterfaceDefinition, MethodDefinition, PropertyDefinition, ServerAccess,
    SignalDefinition,
};

/// Generates Rust structs and traits from D-Bus XML introspection data.
#[deprecated(since = "0.13.0", note = "please use `Generator::from_string(xmldata)?.output_to_string(opts)` instead")]
pub fn generate(xmldata: &str, opts: &GenOpts) -> Result<String, Box<dyn std::error::Error>> {
    Generator::from_string(xmldata)?.output_to_string(opts)
}
