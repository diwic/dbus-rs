
use std::{io, error};
use std::collections::HashSet;
use xml;

fn find_attr<'a>(a: &'a Vec<xml::attribute::OwnedAttribute>, n: &str) -> Result<&'a str, Box<dyn error::Error>> {
    a.into_iter()
        .find(|q| q.name.prefix.is_none() && q.name.local_name == n)
        .map(|f| &*f.value)
        .ok_or_else(|| format!("attribute not found: {:?}", n).into())
}

struct Arg {
    name: String,
    typ: String,
    idx: i32,
    is_out: bool,
}

struct Method {
    name: String,
    fn_name: String,
    iargs: Vec<Arg>,
    oargs: Vec<Arg>,
}

struct Prop {
    name: String,
    get_fn_name: String,
    set_fn_name: String,
    typ: String,
    access: String,
}

struct Signal {
    name: String,
    args: Vec<Arg>,
}

struct Intf {
    origname: String,
    shortname: String,
    methods: Vec<Method>,
    props: Vec<Prop>,
    signals: Vec<Signal>,
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
    /// MethodType for server tree impl, set to none for client impl only
    pub methodtype: Option<String>,
    /// Crossroads server handler type, set to none for client impl only
    pub crhandler: Option<String>,
    /// Removes a prefix from interface names
    pub skipprefix: Option<String>,
    /// Type of server access (tree)
    pub serveraccess: ServerAccess,
    /// Tries to make variants generic instead of Variant<Box<Refarg>>
    pub genericvariant: bool,
    /// Generates code to work with async / futures 0.3
    pub futures: bool,
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
        serveraccess: ServerAccess::RefClosure, genericvariant: false, futures: false,
        crhandler: None, connectiontype: ConnectionType::Blocking, propnewtype: false,
        interfaces: None,
        command_line: String::new()
    }}
}

const RUST_KEYWORDS: [&str; 57] = [
    "as",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "Self",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "union",
    "unsafe",
    "use",
    "where",
    "while",

    "abstract",
    "alignof",
    "async",
    "await",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "offsetof",
    "override",
    "priv",
    "proc",
    "pure",
    "sizeof",
    "try",
    "typeof",
    "unsized",
    "virtual",
    "yield",
];


fn make_camel(s: &str) -> String {
    let mut ucase = true;
    let mut r: String = s.chars().filter_map(|c| match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => {
            let cc = if ucase { c.to_uppercase().next() } else { Some(c) };
            ucase = false;
            cc
        }
        _ => { ucase = true; None }
    }).collect();
    if RUST_KEYWORDS.iter().any(|i| i == &r) { r.push('_') };
    r
}


fn make_snake(s: &str, keyword_check: bool) -> String {
    let mut lcase = false;
    let mut r = String::new();
    for c in s.chars() {
        match c {
             'a'..='z' | '0'..='9' => {
                  r.push(c);
                  lcase = true;
             }
             'A'..='Z' => {
                  if lcase { r.push('_'); }
                  lcase = false;
                  r.push(c.to_lowercase().next().unwrap());
             }
             _ => {
                  if lcase { r.push('_'); }
                  lcase = false;
             }
        }
    }
    if r.len() < 2 { r.push('_'); } // Don't interfere with variable names like 'm' and 'i'
    if keyword_check && RUST_KEYWORDS.iter().any(|i| i == &r) { r.push('_') };
    r
}

fn make_fn_name(intf: &Intf, name: &str) -> String {
    let mut r = make_snake(name, true);
    loop {
        if intf.methods.iter().any(|x| x.fn_name == r) ||
            intf.props.iter().any(|x| x.get_fn_name == r || x.set_fn_name == r) {
            r.push('_');
        } else { return r };
    }
}

struct GenVars {
    prefix: String,
    gen: Vec<String>,
}

fn xml_to_rust_type(i: &mut &[u8], out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<dyn error::Error>> {
    let c = i.get(0).ok_or_else(|| "unexpected end of signature")?;
    *i = &i[1..];
    Ok(match (*c as char, out) {
        ('(', _) => {
            let mut s: Vec<String> = vec!();
            while i.get(0) != Some(&b')') {
                let n = xml_to_rust_type(i, out, genvars)?;
                s.push(n);
            };
            *i = &i[1..];
            format!("({})", s.join(", "))
        },
        ('y', _) => "u8".into(),
        ('b', _) => "bool".into(),
        ('n', _) => "i16".into(),
        ('q', _) => "u16".into(),
        ('i', _) => "i32".into(),
        ('u', _) => "u32".into(),
        ('x', _) => "i64".into(),
        ('t', _) => "u64".into(),
        ('d', _) => "f64".into(),
        ('h', _) => "arg::OwnedFd".into(),
        ('s', false) => "&str".into(),
        ('s', true) => "String".into(),
        ('o', false) => "dbus::Path".into(),
        ('o', true) => "dbus::Path<'static>".into(),
        ('g', false) => "dbus::Signature".into(),
        ('g', true) => "dbus::Signature<'static>".into(),
        ('v', _) => if let &mut Some(ref mut g) = genvars {
            let t = format!("{}", g.prefix);
            // let t = format!("arg::Variant<{}>", g.prefix);
            g.gen.push(g.prefix.clone());
            g.prefix = format!("{}X", g.prefix);
            t
        } else if out { "arg::Variant<Box<dyn arg::RefArg + 'static>>".into() }
        else { "arg::Variant<Box<dyn arg::RefArg>>".into() },
        ('a', _) => if i.get(0) == Some(&b'{') {
            *i = &i[1..];
            if &i[..3] == b"sv}" {
                *i = &i[3..];
                "arg::PropMap".into()
            } else {
                let n1 = xml_to_rust_type(i, out, &mut None)?;
                let n2 = xml_to_rust_type(i, out, &mut None)?;
                if i.get(0) != Some(&b'}') { return Err("No end of dict".into()); }
                *i = &i[1..];
                format!("::std::collections::HashMap<{}, {}>", n1, n2)
            }
        } else {
            format!("Vec<{}>", xml_to_rust_type(i, out, &mut None)?)
        },
        (_, _) => return Err(format!("Unknown character in signature {:?}", c).into()),
    })
}

/// Return whether the given type implements `Copy`.
///
/// Only implented for types which may be returned by `xml_to_rust_type`.
fn can_copy_type(rust_type: &str) -> bool {
    match rust_type {
        "u8" | "bool" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "f64" => true,
        _ => false
    }
}

fn make_type(s: &str, out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<dyn error::Error>> {
    let mut i = s.as_bytes();
    let r = xml_to_rust_type(&mut i, out, genvars)?;
    if i.len() > 0 { Err("Expected type to end".into()) }
    else { Ok(r) }
}

impl Arg {
    fn varname(&self) -> String {
        if self.name != "" {
           make_snake(&self.name, true)
        } else { format!("arg{}", self.idx) }
    }
    fn can_wrap_variant(&self, genvar: bool) -> bool { genvar && self.typ.starts_with("v") }
    fn varname_maybewrap(&self, genvar: bool) -> String {
        if self.can_wrap_variant(genvar) {
            format!("arg::Variant({})", self.varname())
        } else { self.varname() }
    }
    fn typename(&self, genvar: bool) -> Result<(String, Vec<String>), Box<dyn error::Error>> {
        let mut g = if genvar { Some(GenVars {
            prefix: format!("{}{}", if self.is_out { 'R' } else { 'I' }, self.idx),
            gen: vec!(),
        }) } else { None };
        let r = make_type(&self.typ, self.is_out, &mut g)?;
        Ok((r, g.map(|g| g.gen.iter().map(|s|
            if self.is_out { format!("{}: for<'b> arg::Get<'b> + 'static", s) } else { format!("{}: arg::Arg + arg::Append", s) }
        ).collect()).unwrap_or(vec!())))
    }
    fn typename_maybewrap(&self, genvar: bool) -> Result<String, Box<dyn error::Error>> {
        let t = self.typename(genvar)?.0;
        Ok(if self.can_wrap_variant(genvar) {
            format!("arg::Variant<{}>", t)
        } else { t })
    }
}

impl Prop {
    fn can_get(&self) -> bool { self.access != "write" }
    fn can_set(&self) -> bool { self.access == "write" || self.access == "readwrite" }
}

fn write_method_decl(s: &mut String, m: &Method, opts: &GenOpts) -> Result<(), Box<dyn error::Error>> {
    let genvar = opts.genericvariant;
    let g: Vec<String> = if genvar {
        let mut g = vec!();
        for z in m.iargs.iter().chain(m.oargs.iter()) {
            let (_, mut z) = z.typename(genvar)?;
            g.append(&mut z);
        }
        g
    } else { vec!() };


    *s += &format!("    fn {}{}(&self", m.fn_name,
        if g.len() > 0 { format!("<{}>", g.join(",")) } else { "".into() }
    );

    for a in m.iargs.iter() {
        let t = a.typename(genvar)?.0;
        *s += &format!(", {}: {}", a.varname(), t);
    }
    if let Some(crh) = &opts.crhandler { *s += &format!(", info: &cr::{}Info", crh) };

    let r = match m.oargs.len() {
        0 => "()".to_string(),
        1 => m.oargs[0].typename(genvar)?.0,
        _ => {
            let v: Result<Vec<String>, _> = m.oargs.iter().map(|z| z.typename(genvar).map(|t| t.0)).collect();
            format!("({})", v?.join(", "))
        }
    };
    *s += &format!(") -> {}", make_result(&r, opts));

    Ok(())
}

fn make_result(success: &str, opts: &GenOpts) -> String {
    if opts.futures {
        format!("dbusf::MethodReply<{}>", success)
    } else if opts.crhandler.is_some() {
        format!("Result<{}, cr::MethodErr>", success)
    } else if opts.methodtype.is_some() {
        format!("Result<{}, tree::MethodErr>", success)
    } else if opts.connectiontype == ConnectionType::Nonblock {
        format!("nonblock::MethodReply<{}>", success)
    } else {
        format!("Result<{}, dbus::Error>", success)
    }
}

fn write_prop_decl(s: &mut String, p: &Prop, opts: &GenOpts, set: bool) -> Result<(), Box<dyn error::Error>> {
    if set {
        *s += &format!("    fn {}(&self, value: {}) -> {}",
            p.set_fn_name, make_type(&p.typ, true, &mut None)?, make_result("()", opts));
    } else {
        *s += &format!("    fn {}(&self) -> {}",
            p.get_fn_name, make_result(&make_type(&p.typ, true, &mut None)?, opts));
    };
    Ok(())
}

fn write_intf_name(s: &mut String, i: &Intf) -> Result<(), Box<dyn error::Error>> {
    let const_name = make_snake(&i.shortname, false).to_uppercase();
    *s += &format!("\npub const {}_NAME: &str = \"{}\";\n", const_name, i.origname);
    Ok(())
}

fn write_intf(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<dyn error::Error>> {

    let iname = make_camel(&i.shortname);
    *s += &format!("\npub trait {} {{\n", iname);
    for m in &i.methods {
        write_method_decl(s, &m, opts)?;
        *s += ";\n";
    }
    for p in &i.props {
        if p.can_get() {
            write_prop_decl(s, &p, opts, false)?;
            *s += ";\n";
        }
        if p.can_set() {
            write_prop_decl(s, &p, opts, true)?;
            *s += ";\n";
        }
    }
    *s += "}\n";
    Ok(())
}

fn write_intf_client(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<dyn error::Error>> {
    let (module, proxy) = match opts.connectiontype {
        ConnectionType::Ffidisp => ("ffidisp", "ConnPath"),
        ConnectionType::Blocking => ("blocking", "Proxy"),
        ConnectionType::Nonblock => ("nonblock", "Proxy"),
    };

    if module == "nonblock" {
        *s += &format!("\nimpl<'a, T: nonblock::NonblockReply, C: ::std::ops::Deref<Target=T>> {} for {}::{}<'a, C> {{\n",
            make_camel(&i.shortname), module, proxy);
    } else if opts.futures {
        *s += &format!("\nimpl<'a> {} for dbusf::ConnPath<'a> {{\n",
            make_camel(&i.shortname));
    } else if module == "blocking" {
        *s += &format!("\nimpl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target=T>> {} for {}::{}<'a, C> {{\n",
            make_camel(&i.shortname), module, proxy);
    } else {
        assert_eq!(module, "ffidisp");
        *s += &format!("\nimpl<'a, C: ::std::ops::Deref<Target=ffidisp::Connection>> {} for ffidisp::ConnPath<'a, C> {{\n",
            make_camel(&i.shortname));
    }
    for m in &i.methods {
        *s += "\n";
        write_method_decl(s, &m, opts)?;
        *s += " {\n";
        *s += &format!("        self.method_call(\"{}\", \"{}\", (", i.origname, m.name);
        for a in m.iargs.iter() {
            *s += &a.varname_maybewrap(opts.genericvariant);
            *s += ", ";
        }
        *s += "))\n";
        let needs_andthen = (m.oargs.len() == 1) || (m.oargs.iter().any(|oa| oa.can_wrap_variant(opts.genericvariant)));
        if needs_andthen {
            *s += &"            .and_then(|r: (";
            for oa in m.oargs.iter() {
                *s += &oa.typename_maybewrap(opts.genericvariant)?;
                *s += ", ";
            }
            let tuple = m.oargs.len() > 1;
            *s += &format!(")| Ok({}", if tuple { "(" } else { "" });
            for idx in 0..m.oargs.len() {
                *s += &if m.oargs[idx].can_wrap_variant(opts.genericvariant) {
                    format!("(r.{}).0, ", idx)
                } else {
                    format!("r.{}, ", idx)
                };
            }
            *s += &format!("{}))\n", if tuple { ")" } else { "" });
        }
        *s += "    }\n";
    }

    let propintf = format!("{}::stdintf::org_freedesktop_dbus::Properties", module);

    for p in i.props.iter().filter(|p| p.can_get()) {
        *s += "\n";
        write_prop_decl(s, &p, opts, false)?;
        *s += " {\n";
        *s += &format!("        <Self as {}>::get(&self, \"{}\", \"{}\")\n", propintf, i.origname, p.name);
        *s += "    }\n";
    }

    for p in i.props.iter().filter(|p| p.can_set()) {
        *s += "\n";
        write_prop_decl(s, &p, opts, true)?;
        *s += " {\n";
        *s += &format!("        <Self as {}>::set(&self, \"{}\", \"{}\", value)\n", propintf, i.origname, p.name);
        *s += "    }\n";
    }

    *s += "}\n";
    Ok(())

}

fn write_signal(s: &mut String, i: &Intf, ss: &Signal) -> Result<(), Box<dyn error::Error>> {
    let structname = format!("{}{}", make_camel(&i.shortname), make_camel(&ss.name));
    *s += "\n#[derive(Debug)]\n";
    *s += &format!("pub struct {} {{\n", structname);
    for a in ss.args.iter() {
        *s += &format!("    pub {}: {},\n", a.varname(), a.typename(false)?.0);
    }
    *s += "}\n\n";

    *s += &format!("impl arg::AppendAll for {} {{\n", structname);
    *s += &format!("    fn append(&self, {}: &mut arg::IterAppend) {{\n", if ss.args.len() > 0 {"i"} else {"_"});
    for a in ss.args.iter() {
        *s += &format!("        arg::RefArg::append(&self.{}, i);\n", a.varname());
    }
    *s += "    }\n";
    *s += "}\n\n";

    *s += &format!("impl arg::ReadAll for {} {{\n", structname);
    *s += &format!("    fn read({}: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {{\n", if ss.args.len() > 0 {"i"} else {"_"});
    *s += &format!("        Ok({} {{\n", structname);
    for a in ss.args.iter() {
        *s += &format!("            {}: i.read()?,\n", a.varname());
    }
    *s += "        })\n";
    *s += "    }\n";
    *s += "}\n\n";

    *s += &format!("impl dbus::message::SignalArgs for {} {{\n", structname);
    *s += &format!("    const NAME: &'static str = \"{}\";\n", ss.name);
    *s += &format!("    const INTERFACE: &'static str = \"{}\";\n", i.origname);
    *s += "}\n";
    Ok(())
}

fn write_signals(s: &mut String, i: &Intf) -> Result<(), Box<dyn error::Error>> {
    for ss in i.signals.iter() { write_signal(s, i, ss)?; }
    Ok(())
}

fn write_prop_struct(s: &mut String, i: &Intf) -> Result<(), Box<dyn error::Error>> {
    // No point generating the properties struct if the interface has no gettable properties.
    if !i.props.iter().any(|property| property.can_get()) {
        return Ok(())
    }

    let struct_name = format!("{}Properties", make_camel(&i.shortname));
    *s += &format!(r#"
#[derive(Copy, Clone, Debug)]
pub struct {0}<'a>(pub &'a arg::PropMap);

impl<'a> {0}<'a> {{
    pub fn from_interfaces(
        interfaces: &'a ::std::collections::HashMap<String, arg::PropMap>,
    ) -> Option<Self> {{
        interfaces.get("{1}").map(Self)
    }}
"#, struct_name, i.origname);

    for p in &i.props {
        if p.can_get() {
            let rust_type = make_type(&p.typ, true, &mut None)?;
            if can_copy_type(&rust_type) {
                *s += &format!(r#"
    pub fn {}(&self) -> Option<{}> {{
        arg::prop_cast(self.0, "{}").copied()
    }}
"#, p.get_fn_name, rust_type, p.name);
            } else {
                *s += &format!(r#"
    pub fn {}(&self) -> Option<&{}> {{
        arg::prop_cast(self.0, "{}")
    }}
"#, p.get_fn_name, rust_type, p.name);
            }
        }
    }
    *s += "}\n";
    Ok(())
}

fn write_server_access(s: &mut String, i: &Intf, saccess: ServerAccess, minfo_is_ref: bool) {
    let z = if minfo_is_ref {""} else {"&"};
    match saccess {
        ServerAccess::AsRefClosure => {
            *s += &format!("        let dd = fclone({}minfo);\n", z);
            *s += "        let d = dd.as_ref();\n";
        },
        ServerAccess::RefClosure => *s += &format!("        let d = fclone({}minfo);\n", z),
        ServerAccess::MethodInfo => *s += &format!("        let d: &dyn {} = {}minfo;\n", make_camel(&i.shortname), z),
    }
}

// Should we implement this for
// 1) MethodInfo? That's the only way receiver can check Sender, etc - ServerAccess::MethodInfo
// 2) D::ObjectPath?
// 3) A user supplied struct?
// 4) Something reachable from minfo - ServerAccess::RefClosure

fn write_intf_tree(s: &mut String, i: &Intf, mtype: &str, saccess: ServerAccess, genvar: bool) -> Result<(), Box<dyn error::Error>> {
    let hasf = saccess != ServerAccess::MethodInfo;
    let hasm = mtype == "MethodType";

    let treem: String = if hasm { "M".into() } else { format!("tree::{}<D>", mtype) };

    *s += &format!("\npub fn {}_server<{}{}D>(factory: &tree::Factory<{}, D>, data: D::Interface{}) -> tree::Interface<{}, D>\n",
        make_snake(&i.shortname, false), if hasf {"F, T, "} else {""}, if hasm {"M, "} else {""}, treem, if hasf {", f: F"} else {""}, treem);

    let mut wheres: Vec<String> = vec!["D: tree::DataType".into(), "D::Method: Default".into()];
    if i.props.len() > 0 {
        wheres.push("D::Property: Default".into());
    };
    if i.signals.len() > 0 {
        wheres.push("D::Signal: Default".into());
    };
    if hasm {
        wheres.push("M: MethodType<D>".into());
    };
    match saccess {
        ServerAccess::RefClosure => {
            wheres.push(format!("T: {}", make_camel(&i.shortname)));
            wheres.push(format!("F: 'static + for <'z> Fn(& 'z tree::MethodInfo<tree::{}<D>, D>) -> & 'z T", mtype));
        },
        ServerAccess::AsRefClosure => {
            wheres.push(format!("T: AsRef<dyn {}>", make_camel(&i.shortname)));
            wheres.push(format!("F: 'static + Fn(&tree::MethodInfo<tree::{}<D>, D>) -> T", mtype));
        },
        ServerAccess::MethodInfo => {},
    };
    if let ServerAccess::RefClosure | ServerAccess::AsRefClosure = saccess {
        if mtype == "MTSync" {
            wheres.push("F: Send + Sync".into());
        }
    }
    *s += "where\n";
    for w in wheres { *s += &format!("    {},\n", w); }
    *s += "{\n";

    *s += &format!("    let i = factory.interface(\"{}\", data);\n", i.origname);
    if hasf {
        *s += "    let f = ::std::sync::Arc::new(f);";
    }
    for m in &i.methods {
        if hasf {
            *s += "\n    let fclone = f.clone();\n";
        }
        *s += &format!("    let h = move |minfo: &tree::MethodInfo<{}, D>| {{\n", treem);
        if m.iargs.len() > 0 {
            *s += "        let mut i = minfo.msg.iter_init();\n";
        }
        for a in &m.iargs {
            *s += &format!("        let {}: {} = i.read()?;\n", a.varname(), a.typename(genvar)?.0);
        }
        write_server_access(s, i, saccess, true);
        let argsvar = m.iargs.iter().map(|q| q.varname()).collect::<Vec<String>>().join(", ");
        let retargs = match m.oargs.len() {
            0 => String::new(),
            1 => format!("let {} = ", m.oargs[0].varname()),
            _ => format!("let ({}) = ", m.oargs.iter().map(|q| q.varname()).collect::<Vec<String>>().join(", ")),
        };
        *s += &format!("        {}d.{}({})?;\n",
            retargs, m.fn_name, argsvar);
        *s += "        let rm = minfo.msg.method_return();\n";
        for r in &m.oargs {
            *s += &format!("        let rm = rm.append1({});\n", r.varname());
        }
        *s += "        Ok(vec!(rm))\n";
        *s += "    };\n";
        *s += &format!("    let m = factory.method{}(\"{}\", Default::default(), h);\n", if hasm {"_sync"} else {""}, m.name);
        for a in &m.iargs {
            *s += &format!("    let m = m.in_arg((\"{}\", \"{}\"));\n", a.name, a.typ);
        }
        for a in &m.oargs {
            *s += &format!("    let m = m.out_arg((\"{}\", \"{}\"));\n", a.name, a.typ);
        }
        *s +=          "    let i = i.add_m(m);\n";
    }
    for p in &i.props {
        *s += &format!("\n    let p = factory.property::<{}, _>(\"{}\", Default::default());\n", make_type(&p.typ, false, &mut None)?, p.name);
        *s += &format!("    let p = p.access(tree::Access::{});\n", match &*p.access {
            "read" => "Read",
            "readwrite" => "ReadWrite",
            "write" => "Write",
            _ => return Err(format!("Unexpected access value {}", p.access).into()),
        });
        if p.can_get() {
            if hasf {
                *s += "    let fclone = f.clone();\n";
            }
            *s += "    let p = p.on_get(move |a, pinfo| {\n";
            *s += "        let minfo = pinfo.to_method_info();\n";
            write_server_access(s, i, saccess, false);
            *s += &format!("        a.append(d.{}()?);\n", &p.get_fn_name);
            *s += "        Ok(())\n";
            *s += "    });\n";
        }
        if p.can_set() {
            if hasf {
                *s += "    let fclone = f.clone();\n";
            }
            *s += "    let p = p.on_set(move |iter, pinfo| {\n";
            *s += "        let minfo = pinfo.to_method_info();\n";
            write_server_access(s, i, saccess, false);
            *s += &format!("        d.{}(iter.read()?)?;\n", &p.set_fn_name);
            *s += "        Ok(())\n";
            *s += "    });\n";
        }
        *s +=          "    let i = i.add_p(p);\n";
    }
    for ss in &i.signals {
        *s += &format!("    let s = factory.signal(\"{}\", Default::default());\n", ss.name);
        for a in &ss.args {
            *s += &format!("    let s = s.arg((\"{}\", \"{}\"));\n", a.name, a.typ);
        }
        *s += "    let i = i.add_s(s);\n";
    }
    *s +=          "    i\n";
    *s +=          "}\n";
    Ok(())
}

fn write_intf_crossroads(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<dyn error::Error>> {
    let crh = opts.crhandler.as_ref().unwrap();
    *s += &format!("\npub fn {}_ifaceinfo<I>() -> cr::IfaceInfo<'static, cr::{}>\n",
        make_snake(&i.shortname, false), crh);
    *s += &format!("where I: {}{} {{\n",
        make_camel(&i.shortname), if crh == "Par" { " + Send + Sync + 'static" } else { "" });
    *s += &format!("    cr::IfaceInfo::new(\"{}\", vec!(\n", i.origname);

    for m in &i.methods {
        *s += &format!("        MethodInfo::new_{}(\"{}\", |intf: &I, info| {{\n", crh.to_lowercase(), m.name);
        if m.iargs.len() > 0 {
            *s += "            let mut i = info.msg().iter_init();\n";
        }
        for a in &m.iargs {
            *s += &format!("            let {}: {} = i.read()?;\n", a.varname(), a.typename(opts.genericvariant)?.0);
        }
        let mut argsvar: Vec<_> = m.iargs.iter().map(|q| q.varname()).collect();
        argsvar.push("info".into());
        let argsvar = argsvar.join(", ");
        let retargs = match m.oargs.len() {
            0 => String::new(),
            1 => format!("let {} = ", m.oargs[0].varname()),
            _ => format!("let ({}) = ", m.oargs.iter().map(|q| q.varname()).collect::<Vec<String>>().join(", ")),
        };
        *s += &format!("            {}intf.{}({})?;\n",
            retargs, m.fn_name, argsvar);
        *s += "            let rm = info.msg().method_return();\n";
        for r in &m.oargs {
            *s += &format!("            let rm = rm.append1({});\n", r.varname());
        }
        *s += "            Ok(Some(rm))\n";
        *s += "        }),\n";
    }

    *s += "    ), vec!(), vec!())\n"; // TODO: Props, signals
    *s += "}\n";
    Ok(())
}


fn write_module_header(s: &mut String, opts: &GenOpts) {
    *s += &format!("// This code was autogenerated with `dbus-codegen-rust {}`, see https://github.com/diwic/dbus-rs\n", opts.command_line);
    *s += &format!("use {} as dbus;\n", opts.dbuscrate);
    *s += "#[allow(unused_imports)]\n";
    *s += &format!("use {}::arg;\n", opts.dbuscrate);
    if opts.futures {
        *s += "use dbus_futures as dbusf;\n";
    }
    if opts.methodtype.is_some() { *s += &format!("use {}_tree as tree;\n", opts.dbuscrate) } else {
        *s += &format!("use {}::{};\n", opts.dbuscrate, match opts.connectiontype {
            ConnectionType::Ffidisp => "ffidisp",
            ConnectionType::Blocking => "blocking",
            ConnectionType::Nonblock => "nonblock",
        });
    }
    if opts.crhandler.is_some() { *s += &format!("use {}::crossroads as cr;\n", opts.dbuscrate) }
}

/// Generates Rust structs and traits from D-Bus XML introspection data.
pub fn generate(xmldata: &str, opts: &GenOpts) -> Result<String, Box<dyn error::Error>> {
    use xml::EventReader;
    use xml::reader::XmlEvent;

    let mut s = String::new();
    write_module_header(&mut s, opts);
    let mut curintf = None;
    let mut curm = None;
    let mut cursig = None;
    let mut curprop = None;
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
                    methods: Vec::new(), signals: Vec::new(), props: Vec::new() });
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
                write_intf(&mut s, &intf, opts)?;
                if opts.crhandler.is_some() {
                    write_intf_crossroads(&mut s, &intf, opts)?;
                } else if let Some(ref mt) = opts.methodtype {
                    write_intf_tree(&mut s, &intf, &mt, opts.serveraccess, opts.genericvariant)?;
                } else {
                    write_intf_client(&mut s, &intf, opts)?;
                }
                write_signals(&mut s, &intf)?;
                if opts.propnewtype {
                    write_intf_name(&mut s, &intf)?;
                    write_prop_struct(&mut s, &intf)?;
                }
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                if curm.is_some() { Err("Start of method inside method")? };
                if curintf.is_none() { Err("Start of method outside interface")? };
                let name = find_attr(attributes, "name")?;
                curm = Some(Method { name: name.into(), fn_name: make_fn_name(curintf.as_ref().unwrap(), name),
                    iargs: Vec::new(), oargs: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "method" => {
                if curm.is_none() { Err("End of method outside method")? };
                if curintf.is_none() { Err("End of method outside interface")? };
                curintf.as_mut().unwrap().methods.push(curm.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "signal" => {
                if cursig.is_some() { Err("Start of signal inside signal")? };
                if curintf.is_none() { Err("Start of signal outside interface")? };
                cursig = Some(Signal { name: find_attr(attributes, "name")?.into(), args: Vec::new() });
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
                let is_out = if cursig.is_some() { true } else { match find_attr(attributes, "direction") {
                    Err(_) => false,
                    Ok("in") => false,
                    Ok("out") => true,
                    _ => { Err("Invalid direction")?; unreachable!() }
                }};
                let arr = if let Some(ref mut sig) = cursig { &mut sig.args }
                    else if is_out { &mut curm.as_mut().unwrap().oargs } else { &mut curm.as_mut().unwrap().iargs };
                let arg = Arg { name: find_attr(attributes, "name").unwrap_or("").into(),
                    typ: typ, is_out: is_out, idx: arr.len() as i32 };
                arr.push(arg);
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
