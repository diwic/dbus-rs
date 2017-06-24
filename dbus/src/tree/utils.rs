// Small structs that don't have their own unit.

use Signature;
use std::collections::BTreeMap;
use std::sync::Arc;

pub type ArcMap<K, V> = BTreeMap<K, Arc<V>>;

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
/// A D-Bus Argument.
pub struct Argument(Option<String>, Signature<'static>);

impl Argument {
    /// Create a new Argument.
    pub fn new(name: Option<String>, sig: Signature<'static>) -> Argument { Argument(name, sig) }

    /// Descriptive name (if any).
    pub fn name(&self) -> Option<&str> { self.0.as_ref().map(|s| &**s) }

    /// Type signature of argument.
    pub fn signature(&self) -> &Signature<'static> { &self.1 }

    fn introspect(&self, indent: &str, dir: &str) -> String { 
        let n = self.0.as_ref().map(|n| format!("name=\"{}\" ", n)).unwrap_or("".into());
        format!("{}<arg {}type=\"{}\"{}/>\n", indent, n, self.1, dir)
    }

}

pub fn introspect_args(args: &[Argument], indent: &str, dir: &str) -> String {
    args.iter().fold("".to_string(), |aa, az| format!("{}{}", aa, az.introspect(indent, dir)))
}

// Small helper struct to reduce memory somewhat for objects without annotations
#[derive(Clone, Debug, Default)]
pub struct Annotations(Option<BTreeMap<String, String>>);

impl Annotations {
    pub fn new() -> Annotations { Annotations(None) }

    pub fn insert<N: Into<String>, V: Into<String>>(&mut self, n: N, v: V) {
       if self.0.is_none() { self.0 = Some(BTreeMap::new()) }
        self.0.as_mut().unwrap().insert(n.into(), v.into());
    }

    pub fn introspect(&self, indent: &str) -> String {
        self.0.as_ref().map(|s| s.iter().fold("".into(), |aa, (ak, av)| {
            format!("{}{}<annotation name=\"{}\" value=\"{}\"/>\n", aa, indent, ak, av)
        })).unwrap_or(String::new())
    }
}

// Doesn't work, conflicting impls
// impl<S: Into<Signature>> From<S> for Argument

impl From<Signature<'static>> for Argument {
    fn from(t: Signature<'static>) -> Argument { Argument(None, t) }
}

impl<'a> From<&'a str> for Argument {
    fn from(t: &'a str) -> Argument { Argument(None, String::from(t).into()) }
}

impl<N: Into<String>, S: Into<Signature<'static>>> From<(N, S)> for Argument {
    fn from((n, s): (N, S)) -> Argument { Argument(Some(n.into()), s.into()) }
}

pub trait Introspect {
    // At some point we might want to switch to fmt::Write / fmt::Formatter for performance...
    fn xml_name(&self) -> &'static str;
    fn xml_params(&self) -> String;
    fn xml_contents(&self) -> String;
}

