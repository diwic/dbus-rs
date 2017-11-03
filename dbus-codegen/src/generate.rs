
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

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServerAccess {
    // Supply a closure from ref to ref
    RefClosure,
    // Supply a closure from ref to owned object which asrefs 
    AsRefClosure,
    // The interface is implemented for MethodInfo
    MethodInfo
}

// Code generation options
#[derive(Clone, Debug)]
pub struct GenOpts {
    // Name of dbus crate (used for import)
    pub dbuscrate: String,
    // MethodType for server impl, set to none for client impl only
    pub methodtype: Option<String>,
    // Removes a prefix from interface names
    pub skipprefix: Option<String>,
    // Type of server access
    pub serveraccess: ServerAccess,
    // Tries to make variants generic instead of Variant<Box<Refarg>>
    pub genericvariant: bool,
}

impl ::std::default::Default for GenOpts {
    fn default() -> Self { GenOpts { 
        dbuscrate: "dbus".into(), methodtype: Some("MTFn".into()), skipprefix: None,
        serveraccess: ServerAccess::RefClosure, genericvariant: false,
    }}
}

fn make_camel(s: &str) -> String {
    let mut ucase = true;
    s.chars().filter_map(|c| match c {
        'a'...'z' | 'A'...'Z' | '0'...'9' => {
            let cc = if ucase { c.to_uppercase().next() } else { Some(c) };
            ucase = false;
            cc
        }
        _ => { ucase = true; None }
    }).collect()
}


fn make_snake(s: &str) -> String {
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
    r
}

struct GenVars {
    prefix: String,
    gen: Vec<String>,
}

fn xml_to_rust_type<I: Iterator<Item=char>>(i: &mut iter::Peekable<I>, out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<error::Error>> {

    let c = try!(i.next().ok_or_else(|| "unexpected end of signature"));
    let atype = ArgType::from_i32(c as i32);
    Ok(match atype {
        Ok(ArgType::Byte) => "u8".into(),
        Ok(ArgType::Boolean) => "bool".into(),
        Ok(ArgType::Int16) => "i16".into(),
        Ok(ArgType::UInt16) => "u16".into(),
        Ok(ArgType::Int32) => "i32".into(),
        Ok(ArgType::UInt32) => "u32".into(),
        Ok(ArgType::Int64) => "i64".into(),
        Ok(ArgType::UInt64) => "u64".into(),
        Ok(ArgType::Double) => "f64".into(),
        Ok(ArgType::String) => if out { "String".into() } else { "&str".into() },
        Ok(ArgType::ObjectPath) => if out { "dbus::Path<'static>" } else { "dbus::Path" }.into(),
        Ok(ArgType::Signature) => if out { "dbus::Signature<'static>" } else { "dbus::Signature" }.into(),
        Ok(ArgType::Variant) => if let &mut Some(ref mut g) = genvars {
            let t = format!("arg::Variant<{}>", g.prefix);
            g.gen.push(g.prefix.clone());
            g.prefix = format!("{}X", g.prefix);
            t
        } else { "arg::Variant<Box<arg::RefArg>>".into() },
        Ok(ArgType::Array) => if i.peek() == Some(&'{') {
            i.next();
            let n1 = try!(xml_to_rust_type(i, out, &mut None));
            let n2 = try!(xml_to_rust_type(i, out, &mut None));
            if i.next() != Some('}') { return Err("No end of dict".into()); }
            format!("::std::collections::HashMap<{}, {}>", n1, n2)
        } else {
            format!("Vec<{}>", try!(xml_to_rust_type(i, out, &mut None)))
        },
        Err(_) if c == '(' => {
            let mut s: Vec<String> = vec!();
            while i.peek() != Some(&')') {
                let n = try!(xml_to_rust_type(i, out, genvars));
                s.push(n);
            };
            i.next().unwrap();
            format!("({})", s.join(", "))
        }
        a @ _ => return Err(format!("Unknown character in signature {:?}", a).into()),
    })
}

fn make_type(s: &str, out: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<error::Error>> {
    let mut i = s.chars().peekable();
    let r = try!(xml_to_rust_type(&mut i, out, genvars));
    if i.next().is_some() { Err("Expected type to end".into()) }
    else { Ok(r) }
}

impl Arg {
    fn varname(&self) -> String {
        if self.name != "" {
           make_snake(&self.name)
        } else { format!("arg{}", self.idx) }
    }
    fn typename(&self, genvar: bool) -> Result<(String, Vec<String>), Box<error::Error>> {
        let mut g = if genvar { Some(GenVars {
            prefix: format!("{}{}", if self.is_out { 'R' } else { 'I' }, self.idx),
            gen: vec!(),
        }) } else { None };
        let r = try!(make_type(&self.typ, self.is_out, &mut g));
        Ok((r, g.map(|g| g.gen.iter().map(|s|
            if self.is_out { format!("{}: arg::Arg + for<'b> arg::Get<'b>", s) } else { format!("{}: arg::Append", s) } 
        ).collect()).unwrap_or(vec!())))
    }
}

impl Prop {
    fn can_get(&self) -> bool { self.access != "write" }
    fn can_set(&self) -> bool { self.access == "write" || self.access == "readwrite" }
}

fn write_method_decl(s: &mut String, m: &Method, genvar: bool) -> Result<(), Box<error::Error>> {

    let g: Vec<String> = if genvar {
        let mut g = vec!();
        for z in m.iargs.iter().chain(m.oargs.iter()) {
            let (_, mut z) = z.typename(genvar)?;
            g.append(&mut z);
        }
        g
    } else { vec!() };

    *s += &format!("    fn {}{}(&self", make_snake(&m.name), 
        if g.len() > 0 { format!("<{}>", g.join(",")) } else { "".into() }
    );

    for a in m.iargs.iter() {
        let t = try!(a.typename(genvar)).0;
        *s += &format!(", {}: {}", a.varname(), t);
    }
    match m.oargs.len() {
        0 => { *s += ") -> Result<(), Self::Err>"; }
        1 => { *s += &format!(") -> Result<{}, Self::Err>", try!(m.oargs[0].typename(genvar)).0); }
        _ => {
            *s += &format!(") -> Result<({}", try!(m.oargs[0].typename(genvar)).0);
            for z in m.oargs.iter().skip(1) { *s += &format!(", {}", try!(z.typename(genvar)).0); }
            *s += "), Self::Err>";
        }
    }
    Ok(())
}

fn write_prop_decl(s: &mut String, p: &Prop, set: bool) -> Result<(), Box<error::Error>> {
    if set {
        *s += &format!("    fn set_{}(&self, value: {}) -> Result<(), Self::Err>",
            make_snake(&p.name), try!(make_type(&p.typ, true, &mut None)));
    } else {
        *s += &format!("    fn get_{}(&self) -> Result<{}, Self::Err>",
            make_snake(&p.name), try!(make_type(&p.typ, true, &mut None)));
    };
    Ok(())
}

fn write_intf(s: &mut String, i: &Intf, genvar: bool) -> Result<(), Box<error::Error>> {
    
    let iname = make_camel(&i.shortname);  
    *s += &format!("\npub trait {} {{\n", iname);
    *s += "    type Err;\n";
    for m in &i.methods {
        try!(write_method_decl(s, &m, genvar));
        *s += ";\n";
    }
    for p in &i.props {
        if p.can_get() {
            try!(write_prop_decl(s, &p, false));
            *s += ";\n";
        }
        if p.can_set() {
            try!(write_prop_decl(s, &p, true));
            *s += ";\n";
        }
    }
    *s += "}\n";
    Ok(())
}

fn write_intf_client(s: &mut String, i: &Intf, genvar: bool) -> Result<(), Box<error::Error>> {
    *s += &format!("\nimpl<'a, C: ::std::ops::Deref<Target=dbus::Connection>> {} for dbus::ConnPath<'a, C> {{\n",
        make_camel(&i.shortname));
    *s += "    type Err = dbus::Error;\n";
    for m in &i.methods {
        *s += "\n";
        try!(write_method_decl(s, &m, genvar));
        *s += " {\n";
        *s += &format!("        let mut m = try!(self.method_call_with_args(&\"{}\".into(), &\"{}\".into(), |{}| {{\n",
            i.origname, m.name, if m.iargs.len() > 0 { "msg" } else { "_" } );
        if m.iargs.len() > 0 {
                *s += "            let mut i = arg::IterAppend::new(msg);\n";
        }
        for a in m.iargs.iter() {
                *s += &format!("            i.append({});\n", a.varname());
        }
        *s += "        }));\n";
        *s += "        try!(m.as_result());\n";
        if m.oargs.len() == 0 {
            *s += "        Ok(())\n";
        } else {
            *s += "        let mut i = m.iter_init();\n";
            for a in m.oargs.iter() {
                *s += &format!("        let {}: {} = try!(i.read());\n", a.varname(), try!(a.typename(genvar)).0);   
            }
            if m.oargs.len() == 1 {
                *s += &format!("        Ok({})\n", m.oargs[0].varname());
            } else {
                let v: Vec<String> = m.oargs.iter().map(|z| z.varname()).collect();
                *s += &format!("        Ok(({}))\n", v.join(", "));
            }
        }
        *s += "    }\n";
    }

    for p in i.props.iter().filter(|p| p.can_get()) {
        *s += "\n";
        try!(write_prop_decl(s, &p, false));
        *s += " {\n";
        *s += &format!("        <Self as dbus::stdintf::OrgFreedesktopDBusProperties>::get(&self, \"{}\", \"{}\").map(|v| v.0)\n",
            i.origname, p.name);
        *s += "    }\n";
    }

    for p in i.props.iter().filter(|p| p.can_set()) {
        *s += "\n";
        try!(write_prop_decl(s, &p, true));
        *s += " {\n";
        *s += &format!("        <Self as dbus::stdintf::OrgFreedesktopDBusProperties>::set(&self, \"{}\", \"{}\", arg::Variant(value))\n",
            i.origname, p.name);
        *s += "    }\n";
    }

    *s += "}\n";
    Ok(())

}

fn write_signal(s: &mut String, i: &Intf, ss: &Signal) -> Result<(), Box<error::Error>> {
    let structname = format!("{}{}", make_camel(&i.shortname), make_camel(&ss.name));
    *s += "\n#[derive(Debug, Default)]\n";
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
        *s += &format!("        (&self.{} as &arg::RefArg).append(i);\n", a.varname());
    }
    *s += "    }\n";
    *s += &format!("    fn get(&mut self, {}: &mut arg::Iter) -> Result<(), arg::TypeMismatchError> {{\n", if ss.args.len() > 0 {"i"} else {"_"});
    for a in ss.args.iter() {
        *s += &format!("        self.{} = try!(i.read());\n", a.varname());
    }
    *s += "        Ok(())\n";
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
        make_snake(&i.shortname), if hasf {"F, T, "} else {""}, if hasm {"M, "} else {""}, treem, if hasf {", f: F"} else {""}, treem);

    let mut wheres: Vec<String> = vec!["D: tree::DataType".into(), "D::Method: Default".into()];
    if i.props.len() > 0 {
        wheres.push("D::Property: Default".into());
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
            *s += &format!("        let {}: {} = try!(i.read());\n", a.varname(), try!(a.typename(genvar)).0);
        }
        write_server_access(s, i, saccess, true);
        let argsvar = m.iargs.iter().map(|q| q.varname()).collect::<Vec<String>>().join(", ");
        let retargs = match m.oargs.len() {
            0 => String::new(),
            1 => format!("let {} = ", m.oargs[0].varname()),
            _ => format!("let ({}) = ", m.oargs.iter().map(|q| q.varname()).collect::<Vec<String>>().join(", ")),
        };
        *s += &format!("        {}try!(d.{}({}));\n",
            retargs, make_snake(&m.name), argsvar);
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
        *s += &format!("\n    let p = factory.property::<{}, _>(\"{}\", Default::default());\n", try!(make_type(&p.typ, false, &mut None)), p.name);
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
            *s += &format!("        a.append(try!(d.get_{}()));\n", make_snake(&p.name));
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
            *s += &format!("        try!(d.set_{}(try!(iter.read())));\n", make_snake(&p.name));
            *s += "        Ok(())\n";
            *s += "    });\n";
        }
        *s +=          "    let i = i.add_p(p);\n";
    }
    *s +=          "    i\n";
    *s +=          "}\n";
    Ok(())
}

fn write_module_header(s: &mut String, opts: &GenOpts) {
    *s += "// This code was autogenerated with dbus-codegen-rust, see https://github.com/diwic/dbus-rs\n\n";
    if opts.methodtype.is_some() { *s += "#![allow(dead_code)]\n" }
    *s += &format!("use {} as dbus;\n", opts.dbuscrate);
    *s += &format!("use {}::arg;\n", opts.dbuscrate); 
    if opts.methodtype.is_some() { *s += &format!("use {}::tree;\n", opts.dbuscrate) }
}

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
        match try!(e) {
            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "interface" => {
                if curm.is_some() { try!(Err("Start of Interface inside method")) };
                if curintf.is_some() { try!(Err("Start of Interface inside interface")) };
                let n = try!(find_attr(attributes, "name"));
                let mut n2 = n;
                if let &Some(ref p) = &opts.skipprefix {
                    if n.len() > p.len() && n.starts_with(p) { n2 = &n[p.len()..]; }
                }
                curintf = Some(Intf { origname: n.into(), shortname: n2.into(), 
                    methods: Vec::new(), signals: Vec::new(), props: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "interface" => {
                if curm.is_some() { try!(Err("End of Interface inside method")) };
                if curintf.is_none() { try!(Err("End of Interface outside interface")) };
                let intf = curintf.take().unwrap();
                try!(write_intf(&mut s, &intf, opts.genericvariant));
                try!(write_intf_client(&mut s, &intf, opts.genericvariant));
                if let Some(ref mt) = opts.methodtype {
                    try!(write_intf_tree(&mut s, &intf, &mt, opts.serveraccess, opts.genericvariant));
                }
                try!(write_signals(&mut s, &intf));
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                if curm.is_some() { try!(Err("Start of method inside method")) };
                if curintf.is_none() { try!(Err("Start of method outside interface")) };
                curm = Some(Method { name: try!(find_attr(attributes, "name")).into(),
                    iargs: Vec::new(), oargs: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "method" => {
                if curm.is_none() { try!(Err("End of method outside method")) };
                if curintf.is_none() { try!(Err("End of method outside interface")) };
                curintf.as_mut().unwrap().methods.push(curm.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "signal" => {
                if cursig.is_some() { try!(Err("Start of signal inside signal")) };
                if curintf.is_none() { try!(Err("Start of signal outside interface")) };
                cursig = Some(Signal { name: try!(find_attr(attributes, "name")).into(), args: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "signal" => {
                if cursig.is_none() { try!(Err("End of signal outside signal")) };
                if curintf.is_none() { try!(Err("End of signal outside interface")) };
                curintf.as_mut().unwrap().signals.push(cursig.take().unwrap());
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "property" => {
                if curprop.is_some() { try!(Err("Start of property inside property")) };
                if curintf.is_none() { try!(Err("Start of property outside interface")) };
                curprop = Some(Prop {
                    name: try!(find_attr(attributes, "name")).into(), 
                    typ: try!(find_attr(attributes, "type")).into(), 
                    access: try!(find_attr(attributes, "access")).into(), 
                });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "property" => {
                if curprop.is_none() { try!(Err("End of property outside property")) };
                if curintf.is_none() { try!(Err("End of property outside interface")) };
                curintf.as_mut().unwrap().props.push(curprop.take().unwrap());
            }


            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "arg" => {
                if curm.is_none() && cursig.is_none() { try!(Err("Start of arg outside method and signal")) };
                if curintf.is_none() { try!(Err("Start of arg outside interface")) };
                let typ = try!(find_attr(attributes, "type")).into();
                let is_out = if cursig.is_some() { true } else { match find_attr(attributes, "direction") {
                    Err(_) => false,
                    Ok("in") => false,
                    Ok("out") => true,
                    _ => { try!(Err("Invalid direction")); unreachable!() }
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
    if curintf.is_some() { try!(Err("Unterminated interface")) }
    Ok(s)
}

#[cfg(test)]
mod tests {

use super::{generate, ServerAccess, GenOpts};

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

static FROM_POLICYKIT: &'static str = r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
                      "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<!-- GDBus 2.48.1 -->
<node>
  <interface name="org.freedesktop.DBus.Properties">
    <method name="Get">
      <arg type="s" name="interface_name" direction="in"/>
      <arg type="s" name="property_name" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <method name="GetAll">
      <arg type="s" name="interface_name" direction="in"/>
      <arg type="a{sv}" name="properties" direction="out"/>
    </method>
    <method name="Set">
      <arg type="s" name="interface_name" direction="in"/>
      <arg type="s" name="property_name" direction="in"/>
      <arg type="v" name="value" direction="in"/>
    </method>
    <signal name="PropertiesChanged">
      <arg type="s" name="interface_name"/>
      <arg type="a{sv}" name="changed_properties"/>
      <arg type="as" name="invalidated_properties"/>
    </signal>
  </interface>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect">
      <arg type="s" name="xml_data" direction="out"/>
    </method>
  </interface>
  <interface name="org.freedesktop.DBus.Peer">
    <method name="Ping"/>
    <method name="GetMachineId">
      <arg type="s" name="machine_uuid" direction="out"/>
    </method>
  </interface>
  <interface name="org.freedesktop.PolicyKit1.Authority">
    <method name="EnumerateActions">
      <arg type="s" name="locale" direction="in">
      </arg>
      <arg type="a(ssssssuuua{ss})" name="action_descriptions" direction="out">
      </arg>
    </method>
    <method name="CheckAuthorization">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
      <arg type="s" name="action_id" direction="in">
      </arg>
      <arg type="a{ss}" name="details" direction="in">
      </arg>
      <arg type="u" name="flags" direction="in">
      </arg>
      <arg type="s" name="cancellation_id" direction="in">
      </arg>
      <arg type="(bba{ss})" name="result" direction="out">
      </arg>
    </method>
    <method name="CancelCheckAuthorization">
      <arg type="s" name="cancellation_id" direction="in">
      </arg>
    </method>
    <method name="RegisterAuthenticationAgent">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
      <arg type="s" name="locale" direction="in">
      </arg>
      <arg type="s" name="object_path" direction="in">
      </arg>
    </method>
    <method name="RegisterAuthenticationAgentWithOptions">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
      <arg type="s" name="locale" direction="in">
      </arg>
      <arg type="s" name="object_path" direction="in">
      </arg>
      <arg type="a{sv}" name="options" direction="in">
      </arg>
    </method>
    <method name="UnregisterAuthenticationAgent">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
      <arg type="s" name="object_path" direction="in">
      </arg>
    </method>
    <method name="AuthenticationAgentResponse">
      <arg type="s" name="cookie" direction="in">
      </arg>
      <arg type="(sa{sv})" name="identity" direction="in">
      </arg>
    </method>
    <method name="AuthenticationAgentResponse2">
      <arg type="u" name="uid" direction="in">
      </arg>
      <arg type="s" name="cookie" direction="in">
      </arg>
      <arg type="(sa{sv})" name="identity" direction="in">
      </arg>
    </method>
    <method name="EnumerateTemporaryAuthorizations">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
      <arg type="a(ss(sa{sv})tt)" name="temporary_authorizations" direction="out">
      </arg>
    </method>
    <method name="RevokeTemporaryAuthorizations">
      <arg type="(sa{sv})" name="subject" direction="in">
      </arg>
    </method>
    <method name="RevokeTemporaryAuthorizationById">
      <arg type="s" name="id" direction="in">
      </arg>
    </method>
    <signal name="Changed">
    </signal>
    <property type="s" name="BackendName" access="read">
    </property>
    <property type="s" name="BackendVersion" access="read">
    </property>
    <property type="u" name="BackendFeatures" access="read">
    </property>
  </interface>
</node>
"#; 

    #[test]
    fn from_dbus() {
        let s = generate(FROM_DBUS, &GenOpts { methodtype: Some("MTSync".into()), ..Default::default() }).unwrap();
        println!("{}", s);
        //assert_eq!(s, "fdjsf");
    }

    #[test]
    fn from_policykit() {
        let s = generate(FROM_POLICYKIT, &Default::default()).unwrap();
        println!("{}", s);
        let mut f = ::std::fs::File::create("./tests/generated/mod.rs").unwrap();
        (&mut f as &mut ::std::io::Write).write_all(s.as_bytes()).unwrap();
        drop(f);
        // assert_eq!(s, "fdjsf");
    }

    #[test]
    fn from_policykit_asref_generic() {
        let g = GenOpts { methodtype: Some("Generic".into()), serveraccess: ServerAccess::AsRefClosure, ..Default::default() };
        let s = generate(FROM_POLICYKIT, &g).unwrap();
        println!("{}", s);
        let mut f = ::std::fs::File::create("./tests/generated/asref.rs").unwrap();
        (&mut f as &mut ::std::io::Write).write_all(s.as_bytes()).unwrap();
    }
}
