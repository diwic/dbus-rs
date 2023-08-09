use std::error;
use std::collections::HashMap;

pub (super) struct Arg {
    pub name: String,
    pub typ: String,
    pub idx: i32,
    pub no_refs: bool,
    pub is_out: bool,
    pub annotations: HashMap<String, String>,
}

pub (super) struct Method {
    pub name: String,
    pub fn_name: String,
    pub iargs: Vec<Arg>,
    pub oargs: Vec<Arg>,
    pub annotations: HashMap<String, String>,
}

pub (super) struct Prop {
    pub name: String,
    pub get_fn_name: String,
    pub set_fn_name: String,
    pub typ: String,
    pub access: String,
    pub annotations: HashMap<String, String>,
}

pub (super) struct Signal {
    pub name: String,
    pub args: Vec<Arg>,
    pub annotations: HashMap<String, String>,
}

pub (super) struct Intf {
    pub origname: String,
    pub shortname: String,
    pub methods: Vec<Method>,
    pub props: Vec<Prop>,
    pub signals: Vec<Signal>,
    pub annotations: HashMap<String, String>,
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


pub (super) fn make_camel(s: &str) -> String {
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


pub (super) fn make_snake(s: &str, keyword_check: bool) -> String {
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

pub (super) fn make_fn_name(intf: &Intf, name: &str) -> String {
    let mut r = make_snake(name, true);
    loop {
        if intf.methods.iter().any(|x| x.fn_name == r) ||
            intf.props.iter().any(|x| x.get_fn_name == r || x.set_fn_name == r) {
            r.push('_');
        } else { return r };
    }
}

pub (super) struct GenVars {
    prefix: String,
    gen: Vec<String>,
}

pub (super) fn xml_to_rust_type(i: &mut &[u8], no_refs: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<dyn error::Error>> {
    let c = i.get(0).ok_or_else(|| "unexpected end of signature")?;
    *i = &i[1..];
    Ok(match (*c as char, no_refs) {
        ('(', _) => {
            let mut s: Vec<String> = vec!();
            while i.get(0) != Some(&b')') {
                let n = xml_to_rust_type(i, no_refs, genvars)?;
                s.push(n);
            };
            *i = &i[1..];
            format!("({},)", s.join(", "))
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
        } else if no_refs { "arg::Variant<Box<dyn arg::RefArg + 'static>>".into() }
        else { "arg::Variant<Box<dyn arg::RefArg>>".into() },
        ('a', _) => if i.get(0) == Some(&b'{') {
            *i = &i[1..];
            if &i[..3] == b"sv}" {
                *i = &i[3..];
                "arg::PropMap".into()
            } else {
                let n1 = xml_to_rust_type(i, no_refs, &mut None)?;
                let n2 = xml_to_rust_type(i, no_refs, &mut None)?;
                if i.get(0) != Some(&b'}') { return Err("No end of dict".into()); }
                *i = &i[1..];
                format!("::std::collections::HashMap<{}, {}>", n1, n2)
            }
        } else {
            format!("Vec<{}>", xml_to_rust_type(i, no_refs, &mut None)?)
        },
        (_, _) => return Err(format!("Unknown character in signature {:?}", c).into()),
    })
}

/// Return whether the given type implements `Copy`.
///
/// Only implented for types which may be returned by `xml_to_rust_type`.
pub (super) fn can_copy_type(rust_type: &str) -> bool {
    match rust_type {
        "u8" | "bool" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "f64" => true,
        _ => false
    }
}

pub (super) fn make_type(s: &str, no_refs: bool, genvars: &mut Option<GenVars>) -> Result<String, Box<dyn error::Error>> {
    let mut i = s.as_bytes();
    let r = xml_to_rust_type(&mut i, no_refs, genvars)?;
    if i.len() > 0 { Err("Expected type to end".into()) }
    else { Ok(r) }
}

impl Arg {
    pub fn varname(&self) -> String {
        if self.name != "" {
           make_snake(&self.name, true)
        } else { format!("arg{}", self.idx) }
    }
    pub fn can_wrap_variant(&self, genvar: bool) -> bool { genvar && self.typ.starts_with("v") }
    pub fn varname_maybewrap(&self, genvar: bool) -> String {
        if self.can_wrap_variant(genvar) {
            format!("arg::Variant({})", self.varname())
        } else { self.varname() }
    }
    pub fn typename(&self, genvar: bool) -> Result<(String, Vec<String>), Box<dyn error::Error>> {
        if let Some(u) = self.user_type() {
            return Ok((u, vec!()));
        }
        let mut g = if genvar { Some(GenVars {
            prefix: format!("{}{}", if self.no_refs { 'R' } else { 'I' }, self.idx),
            gen: vec!(),
        }) } else { None };
        let r = make_type(&self.typ, self.no_refs, &mut g)?;
        Ok((r, g.map(|g| g.gen.iter().map(|s|
            if self.no_refs { format!("{}: for<'b> arg::Get<'b> + 'static", s) } else { format!("{}: arg::Arg + arg::Append", s) }
        ).collect()).unwrap_or(vec!())))
    }
    pub fn typename_norefs(&self) -> Result<String, Box<dyn error::Error>> {
        if let Some(u) = self.user_type() {
            return Ok(u);
        }
        make_type(&self.typ, true, &mut None)
    }
    pub fn typename_maybewrap(&self, genvar: bool) -> Result<String, Box<dyn error::Error>> {
        let t = self.typename(genvar)?.0;
        Ok(if self.can_wrap_variant(genvar) {
            format!("arg::Variant<{}>", t)
        } else { t })
    }
    fn user_type(&self) -> Option<String> {
        if let Some(v) = self.annotations.get("rs.dbus.ArgType") {
            let mut t = if self.no_refs {
                "".to_owned()
            } else {
                "&".to_owned()
            };
            t += v;
            Some(t)
        } else {
            None
        }
    }
}

impl Prop {
    pub fn can_get(&self) -> bool { self.access != "write" }
    pub fn can_set(&self) -> bool { self.access == "write" || self.access == "readwrite" }
    pub fn typename(&self) -> Result<String, Box<dyn error::Error>> {
        if let Some(v) = self.annotations.get("rs.dbus.ArgType") {
            Ok(v.clone())
        } else {
            make_type(&self.typ, true, &mut None)
        }
    }
}
