
use std::{io, error, iter};
use dbus::arg::ArgType;
use xml;

fn find_attr<'a>(a: &'a Vec<xml::attribute::OwnedAttribute>, n: &str) -> Result<&'a str, Box<error::Error>> {
    a.into_iter().find(|q| q.name.local_name == n).map(|f| &*f.value).ok_or_else(|| "attribute not found".into())    
}

struct Arg {
    name: String,
    typ: String,
    idx: i32,
    is_out: bool,
}

struct Method {
    name: String,
    iargs: Vec<Arg>,
    oargs: Vec<Arg>,
}

struct Prop {
    name: String,
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
}

impl ::std::default::Default for GenOpts {
    fn default() -> Self { GenOpts { 
        dbuscrate: "dbus".into(), methodtype: Some("MTFn".into()), skipprefix: None,
        serveraccess: ServerAccess::RefClosure, genericvariant: false, futures: false,
        crhandler: None,
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
        'a'...'z' | 'A'...'Z' | '0'...'9' => {
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
             'a'...'z' | '0'...'9' => {
                  r.push(c);
                  lcase = true;
             }
             'A'...'Z' => {
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

struct GenVars {
    prefix: String,
    gen: Vec<String>,
}

fn xml_to_rust_type<I: Iterator<Item=char>>(i: &mut iter::Peekable<I>, out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<error::Error>> {

    let c = i.next().ok_or_else(|| "unexpected end of signature")?;
    let atype = ArgType::from_i32(c as i32);
    let result = match (atype, c) {
        (Err(_), '(') => {
            let mut s: Vec<String> = vec!();
            while i.peek() != Some(&')') {
                let n = xml_to_rust_type(i, out, genvars)?;
                s.push(n);
            };
            i.next().unwrap();
            format!("({})", s.join(", "))
        },
        (Err(_), a @ _) => return Err(format!("Unknown character in signature {:?}", a).into()),
        (Ok(a @ _), _) => match (a, out) {
            (ArgType::Byte, _) => "u8".into(),
            (ArgType::Boolean, _) => "bool".into(),
            (ArgType::Int16, _) => "i16".into(),
            (ArgType::UInt16, _) => "u16".into(),
            (ArgType::Int32, _) => "i32".into(),
            (ArgType::UInt32, _) => "u32".into(),
            (ArgType::Int64, _) => "i64".into(),
            (ArgType::UInt64, _) => "u64".into(),
            (ArgType::Double, _) => "f64".into(),
            (ArgType::UnixFd, _) => "dbus::OwnedFd".into(),
            (ArgType::String, false) => "&str".into(),
            (ArgType::String, true) => "String".into(),
            (ArgType::ObjectPath, false) => "dbus::Path".into(),
            (ArgType::ObjectPath, true) => "dbus::Path<'static>".into(),
            (ArgType::Signature, false) => "dbus::Signature".into(),
            (ArgType::Signature, true) => "dbus::Signature<'static>".into(),
            (ArgType::Variant, _) => if let &mut Some(ref mut g) = genvars {
                let t = format!("arg::Variant<{}>", g.prefix);
                g.gen.push(g.prefix.clone());
                g.prefix = format!("{}X", g.prefix);
                t
            } else if out { "arg::Variant<Box<arg::RefArg + 'static>>".into() }
            else { "arg::Variant<Box<arg::RefArg>>".into() }
            (ArgType::Array, _) => if i.peek() == Some(&'{') {
                i.next();
                let n1 = xml_to_rust_type(i, out, &mut None)?;
                let n2 = xml_to_rust_type(i, out, &mut None)?;
                if i.next() != Some('}') { return Err("No end of dict".into()); }
                format!("::std::collections::HashMap<{}, {}>", n1, n2)
            } else {
                format!("Vec<{}>", xml_to_rust_type(i, out, &mut None)?)
            },
            (ArgType::Invalid, _) |
            (ArgType::Struct, _) |
            (ArgType::DictEntry, _) => return Err(format!("Unexpected character in signature {:?}", a).into())
        }
    };
    Ok(result)
}

fn make_type(s: &str, out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<error::Error>> {
    let mut i = s.chars().peekable();
    let r = xml_to_rust_type(&mut i, out, genvars)?;
    if i.next().is_some() { Err("Expected type to end".into()) }
    else { Ok(r) }
}

impl Arg {
    fn varname(&self) -> String {
        if self.name != "" {
           make_snake(&self.name, true)
        } else { format!("arg{}", self.idx) }
    }
    fn typename(&self, genvar: bool) -> Result<(String, Vec<String>), Box<error::Error>> {
        let mut g = if genvar { Some(GenVars {
            prefix: format!("{}{}", if self.is_out { 'R' } else { 'I' }, self.idx),
            gen: vec!(),
        }) } else { None };
        let r = make_type(&self.typ, self.is_out, &mut g)?;
        Ok((r, g.map(|g| g.gen.iter().map(|s|
            if self.is_out { format!("{}: for<'b> arg::Get<'b>", s) } else { format!("{}: arg::Arg + arg::Append", s) } 
        ).collect()).unwrap_or(vec!())))
    }
}

impl Prop {
    fn can_get(&self) -> bool { self.access != "write" }
    fn can_set(&self) -> bool { self.access == "write" || self.access == "readwrite" }
}

fn write_method_decl(s: &mut String, m: &Method, opts: &GenOpts) -> Result<(), Box<error::Error>> {
    let genvar = opts.genericvariant;
    let g: Vec<String> = if genvar {
        let mut g = vec!();
        for z in m.iargs.iter().chain(m.oargs.iter()) {
            let (_, mut z) = z.typename(genvar)?;
            g.append(&mut z);
        }
        g
    } else { vec!() };
 

    *s += &format!("    fn {}{}(&self", make_snake(&m.name, true), 
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
            let v: Vec<String> = m.oargs.iter().map(|z| z.varname()).collect();
            format!("({})", v.join(", "))
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
    } else {
        format!("Result<{}, Self::Err>", success)
    }
}

fn write_prop_decl(s: &mut String, p: &Prop, opts: &GenOpts, set: bool) -> Result<(), Box<error::Error>> {
    if set {
        *s += &format!("    fn set_{}(&self, value: {}) -> {}",
            make_snake(&p.name, false), make_type(&p.typ, true, &mut None)?, make_result("()", opts));
    } else {
        *s += &format!("    fn get_{}(&self) -> {}",
            make_snake(&p.name, false), make_result(&make_type(&p.typ, true, &mut None)?, opts));
    };
    Ok(())
}

fn write_intf(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<error::Error>> {
    
    let iname = make_camel(&i.shortname);  
    *s += &format!("\npub trait {} {{\n", iname);
    if !opts.futures && !opts.crhandler.is_some() {
        *s += "    type Err;\n";
    }
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

fn write_intf_client(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<error::Error>> {
    if opts.futures {
        *s += &format!("\nimpl<'a> {} for dbusf::ConnPath<'a> {{\n",
            make_camel(&i.shortname));
    } else {
        *s += &format!("\nimpl<'a, C: ::std::ops::Deref<Target=dbus::Connection>> {} for dbus::ConnPath<'a, C> {{\n",
            make_camel(&i.shortname));
        *s += "    type Err = dbus::Error;\n";
    }
    for m in &i.methods {
        *s += "\n";
        write_method_decl(s, &m, opts)?;
        *s += " {\n";
        *s += &format!("        let {}m = self.method_call_with_args(&\"{}\".into(), &\"{}\".into(), |{}| {{\n",
            if opts.futures { "" } else { "mut " }, i.origname, m.name, if m.iargs.len() > 0 { "msg" } else { "_" } );
        if m.iargs.len() > 0 {
                *s += "            let mut i = arg::IterAppend::new(msg);\n";
        }
        for a in m.iargs.iter() {
                *s += &format!("            i.append({});\n", a.varname());
        }
        let indent;
        if opts.futures {
             *s += "        });\n";
             *s += &format!("        dbusf::MethodReply::from_msg(m, |{}| {{\n", if m.oargs.len() == 0 { "_" } else { "m" });
             indent = "            ";
        } else {
             *s += "        })?;\n";
             *s += "        m.as_result()?;\n";
             indent = "        ";
        }
        if m.oargs.len() == 0 {
            *s += indent;
            *s += "Ok(())\n";
        } else {
            *s += indent;
            *s += "let mut i = m.iter_init();\n";
            for a in m.oargs.iter() {
                *s += &format!("{}let {}: {} = i.read()?;\n", indent, a.varname(), a.typename(opts.genericvariant)?.0);   
            }
            if m.oargs.len() == 1 {
                *s += &format!("{}Ok({})\n", indent, m.oargs[0].varname());
            } else {
                let v: Vec<String> = m.oargs.iter().map(|z| z.varname()).collect();
                *s += &format!("{}Ok(({}))\n", indent, v.join(", "));
            }
        }
        if opts.futures {
            *s += "        })\n";
        }
        *s += "    }\n";
    }

    let propintf = if opts.futures { "dbusf::stdintf::org_freedesktop::DBusProperties" } else { "dbus::stdintf::org_freedesktop_dbus::Properties" };

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

fn write_signal(s: &mut String, i: &Intf, ss: &Signal) -> Result<(), Box<error::Error>> {
    let structname = format!("{}{}", make_camel(&i.shortname), make_camel(&ss.name));
    *s += "\n#[derive(Debug)]\n";
    *s += &format!("pub struct {} {{\n", structname);
    for a in ss.args.iter() {
        *s += &format!("    pub {}: {},\n", a.varname(), a.typename(false)?.0);
    }
    *s += "}\n\n";

    *s += &format!("impl dbus::SignalArgs for {} {{\n", structname);
    *s += &format!("    const NAME: &'static str = \"{}\";\n", ss.name);
    *s += &format!("    const INTERFACE: &'static str = \"{}\";\n", i.origname);
    *s += &format!("    fn append(&self, {}: &mut arg::IterAppend) {{\n", if ss.args.len() > 0 {"i"} else {"_"});
    for a in ss.args.iter() {
        *s += &format!("        arg::RefArg::append(&self.{}, i);\n", a.varname());
    }
    *s += "    }\n";
    *s += &format!("    fn get({}: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {{\n", if ss.args.len() > 0 {"i"} else {"_"});
    *s += &format!("        Ok({} {{\n", structname);
    for a in ss.args.iter() {
        *s += &format!("            {}: i.read()?,\n", a.varname());
    }
    *s += "        })\n";
    *s += "    }\n";
    *s += "}\n";
    Ok(())
}

fn write_signals(s: &mut String, i: &Intf) -> Result<(), Box<error::Error>> {
    for ss in i.signals.iter() { write_signal(s, i, ss)?; }
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
        ServerAccess::MethodInfo => *s += &format!("        let d: &{}<Err=tree::MethodErr> = {}minfo;\n", make_camel(&i.shortname), z),
    }
}

// Should we implement this for
// 1) MethodInfo? That's the only way receiver can check Sender, etc - ServerAccess::MethodInfo
// 2) D::ObjectPath?  
// 3) A user supplied struct?
// 4) Something reachable from minfo - ServerAccess::RefClosure

fn write_intf_tree(s: &mut String, i: &Intf, mtype: &str, saccess: ServerAccess, genvar: bool) -> Result<(), Box<error::Error>> {
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
            wheres.push(format!("T: {}<Err=tree::MethodErr>", make_camel(&i.shortname)));
            wheres.push(format!("F: 'static + for <'z> Fn(& 'z tree::MethodInfo<tree::{}<D>, D>) -> & 'z T", mtype));
        },
        ServerAccess::AsRefClosure => {
            wheres.push(format!("T: AsRef<{}<Err=tree::MethodErr>>", make_camel(&i.shortname)));
            wheres.push(format!("F: 'static + Fn(&tree::MethodInfo<tree::{}<D>, D>) -> T", mtype));
        },
        ServerAccess::MethodInfo => {},
    };
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
            retargs, make_snake(&m.name, true), argsvar);
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
            *s += &format!("        a.append(d.get_{}()?);\n", make_snake(&p.name, false));
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
            *s += &format!("        d.set_{}(iter.read()?)?;\n", make_snake(&p.name, false));
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

fn write_intf_crossroads(s: &mut String, i: &Intf, opts: &GenOpts) -> Result<(), Box<error::Error>> {
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
            retargs, make_snake(&m.name, true), argsvar);
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
    *s += "// This code was autogenerated with dbus-codegen-rust, see https://github.com/diwic/dbus-rs\n\n";
    *s += &format!("use {} as dbus;\n", opts.dbuscrate);
    *s += &format!("use {}::arg;\n", opts.dbuscrate);
    if opts.futures {
        *s += "use dbus_futures as dbusf;\n";
    }
    if opts.methodtype.is_some() { *s += &format!("use {}::tree;\n", opts.dbuscrate) }
    if opts.crhandler.is_some() { *s += &format!("use {}::crossroads as cr;\n", opts.dbuscrate) }
}

/// Generates Rust structs and traits from D-Bus XML introspection data.
pub fn generate(xmldata: &str, opts: &GenOpts) -> Result<String, Box<error::Error>> {
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
                write_intf(&mut s, &intf, opts)?;
                if opts.crhandler.is_some() {
                    write_intf_crossroads(&mut s, &intf, opts)?;
                } else {
                    write_intf_client(&mut s, &intf, opts)?;
                    if let Some(ref mt) = opts.methodtype {
                        write_intf_tree(&mut s, &intf, &mt, opts.serveraccess, opts.genericvariant)?;
                    }
                }
                write_signals(&mut s, &intf)?;
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                if curm.is_some() { Err("Start of method inside method")? };
                if curintf.is_none() { Err("Start of method outside interface")? };
                curm = Some(Method { name: find_attr(attributes, "name")?.into(),
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
                curprop = Some(Prop {
                    name: find_attr(attributes, "name")?.into(), 
                    typ: find_attr(attributes, "type")?.into(), 
                    access: find_attr(attributes, "access")?.into(), 
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
