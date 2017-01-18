
use std::{io, error, iter};
use dbus::arg::ArgType;
use xml;

fn find_attr<'a>(a: &'a Vec<xml::attribute::OwnedAttribute>, n: &str) -> Result<&'a str, Box<error::Error>> {
    a.into_iter().find(|q| q.name.local_name == n).map(|f| &*f.value).ok_or_else(|| "attribute not found".into())    
}

struct Method {
    name: String,
    iargs: Vec<(Option<String>, String)>,
    oargs: Vec<String>,
}

struct Intf {
    name: String,
    methods: Vec<Method>,
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
             _ => { lcase = true; }
        }
    }
    r
}

fn xml_to_rust_type<I: Iterator<Item=char>>(i: &mut iter::Peekable<I>, out: bool, instruct: bool) -> Result<String, Box<error::Error>> {
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
        Ok(ArgType::ObjectPath) => if out { "::dbus::Path<'static>" } else { "::dbus::Path" }.into(),
        Ok(ArgType::Signature) => if out { "::dbus::Signature<'static>" } else { "::dbus::Signature" }.into(),
        Ok(ArgType::Variant) => "::dbus::arg::Variant<Box<::dbus::arg::RefArg>>".into(),
        Ok(ArgType::Array) => if i.peek() == Some(&'{') {
            i.next();
            let n1 = try!(xml_to_rust_type(i, out, false));
            let n2 = try!(xml_to_rust_type(i, out, false));
            if i.next() != Some('}') { return Err("No end of dict".into()); }
            format!("::std::collections::HashMap<{}, {}>", n1, n2)
        } else {
            format!("Vec<{}>", try!(xml_to_rust_type(i, out, false)))
        },
/*
 if out { format!("Vec<{}>", try!(xml_to_rust_type(i, out, false))) }
            else { format!("&[{}]", try!(xml_to_rust_type(i, out, false))) },
        Err(_) if c == '{' => {
            let n1 = try!(xml_to_rust_type(i, out, false));
            let n2 = try!(xml_to_rust_type(i, out, false));
            if i.next() != Some('}') { return Err("No end of dict".into()); }
            format!("({}, {})", n1, n2)
            },*/
        Err(_) if c == '(' => {
            let mut s: Vec<String> = vec!();
            loop {
                let n = try!(xml_to_rust_type(i, out, true));
                if n == "" { return Ok(format!("({})", s.join(", "))) };
                s.push(n);
            }
        }
        Err(_) if c == ')' && instruct => "".into(),
        a @ _ => panic!(format!("{:?}", a)),
    })
}

fn make_type(s: &str, out: bool) -> Result<String, Box<error::Error>> {
    let mut i = s.chars().peekable();
    let r = try!(xml_to_rust_type(&mut i, out, false));
    if i.next().is_some() { Err("Expected type to end".into()) }
    else { Ok(r) }
}

fn make_varname(n: &Option<String>, idx: usize) -> String {
    if let Some(nn) = n.as_ref() {
        make_snake(nn)
    } else { format!("arg{}", idx) }
}

fn write_method_decl(s: &mut String, m: &Method) -> Result<(), Box<error::Error>> {
    *s += &format!("    fn {}(&self", make_snake(&m.name));
    for (idx, a) in m.iargs.iter().enumerate() {
        let t = try!(make_type(&a.1, false));
        *s += &format!(", {}: {}", make_varname(&a.0, idx), t);
    }
    match m.oargs.len() {
        0 => { *s += ") -> Result<(), ::dbus::Error>"; }
        1 => { *s += &format!(") -> Result<{}, ::dbus::Error>", try!(make_type(&m.oargs[0], true))); }
        _ => {
            *s += &format!(") -> Result<({}", try!(make_type(&m.oargs[0], true)));
            for z in m.oargs.iter().skip(1) { *s += &format!(", {}", try!(make_type(&z, true))); }
            *s += "), ::dbus::Error>";
        }
    }
    Ok(())
}

fn write_intf(s: &mut String, i: &Intf) -> Result<(), Box<error::Error>> {
    
    let iname = make_camel(&i.name);  
    *s += &format!("\npub trait {} {{\n", iname);
    for m in &i.methods {
        try!(write_method_decl(s, &m));
        *s += ";\n";
    }
    *s += "}\n";
    Ok(())
}

fn write_intf_client(s: &mut String, i: &Intf) -> Result<(), Box<error::Error>> {
    *s += &format!("\nimpl<'a, C: ::std::ops::Deref<Target=::dbus::Connection>> {} for ::dbus::ConnPath<'a, C> {{\n",
        make_camel(&i.name));
    for m in &i.methods {
        *s += "\n";
        try!(write_method_decl(s, &m));
        *s += " {\n";
        *s += &format!("        let mut m = try!(self.method_call_with_args(&\"{}\".into(), &\"{}\".into(), |{}| {{\n",
            i.name, m.name, if m.iargs.len() > 0 { "msg" } else { "_" } );
        if m.iargs.len() > 0 {
                *s += "            let mut i = ::dbus::arg::IterAppend::new(msg);\n";
        }
        for (idx, a) in m.iargs.iter().enumerate() {
                *s += &format!("            i.append({});\n", make_varname(&a.0, idx));
        }
        *s += "        }));\n";
        *s += "        try!(m.as_result());\n";
        if m.oargs.len() == 0 {
            *s += "        Ok(())\n";
        } else {
            *s += "        let mut i = m.iter_init();\n";
            for (idx, a) in m.oargs.iter().enumerate() {
                *s += &format!("        let a{}: {} = try!(i.read());\n", idx, try!(make_type(a, true)));   
            }
            if m.oargs.len() == 1 {
                *s += "        Ok(a0)\n";
            } else {
                let v: Vec<String> = (0..m.oargs.len()).into_iter().map(|idx| idx.to_string()).collect();
                *s += &format!("        Ok((a{}))\n", v.join(", a"));
            }
        }
        *s += "    }\n";
    }
    *s += "}\n";
    Ok(())

}


// Should we implement this for
// 1) MethodInfo? That's the only way receiver can check Sender, etc.
// 2) D::ObjectPath?  
// 3) A user supplied struct?
// 4) Something reachable from minfo?

fn write_intf_tree(s: &mut String, i: &Intf, mtype: &str) -> Result<(), Box<error::Error>> {
    *s += &format!("\npub fn {}_server<F, T, D>(factory: &::dbus::tree::Factory<::dbus::tree::{}<D>, D>, data: D::Interface, f: F) -> ::dbus::tree::Interface<::dbus::tree::{}<D>, D>\n",
        make_snake(&i.name), mtype, mtype);
    *s += &format!("where D: ::dbus::tree::DataType, D::Method: Default, T: {}, \n", make_camel(&i.name));
    *s += &format!("    F: 'static + for <'z> Fn(& 'z ::dbus::tree::MethodInfo<::dbus::tree::{}<D>, D>) -> & 'z T {{\n", mtype);
    *s += &format!("    let i = factory.interface(\"{}\", data);\n", i.name);
    *s += "    let f = ::std::sync::Arc::new(f);";
    for m in &i.methods {
        *s += "\n    let fclone = f.clone();\n";    
        *s += &format!("    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::{}<D>, D>| {{\n", mtype);
        if m.iargs.len() > 0 {
            *s += "        let mut i = minfo.msg.iter_init();\n";
        }
        for (idx, a) in m.iargs.iter().enumerate() {
            *s += &format!("        let a{}: {} = try!(i.read());\n", idx, try!(make_type(&a.1, false)));
        }
        *s += "        let d = fclone(minfo);\n";
        let argsvar = (0..m.iargs.len()).into_iter().map(|q| format!("a{}", q)).collect::<Vec<String>>().join(", ");
        let retargs = match m.oargs.len() {
            0 => String::new(),
            1 => "let r0 = ".into(),
            n @ _ => format!("let ({}) = ", (0..n).into_iter().map(|q| format!("r{}", q)).collect::<Vec<String>>().join(", ")),
        };
        *s += &format!("        {}try!(d.{}({}));\n",
            retargs, make_snake(&m.name), argsvar);
        *s += "        let rm = minfo.msg.method_return();\n";
        for r in 0..m.oargs.len() {
            *s += &format!("        let rm = rm.append1(r{});\n", r);
        }
        *s += "        Ok(vec!(rm))\n";
        *s += "    };\n";
        *s += &format!("    let m = factory.method(\"{}\", Default::default(), h);\n", m.name);
        for a in &m.iargs {
            *s += &format!("    let m = m.in_arg((\"{}\", \"{}\"));\n", a.0.as_ref().map(|s| &**s).unwrap_or(""), a.1);
        }
        for a in &m.oargs {
            *s += &format!("    let m = m.out_arg((\"\", \"{}\"));\n", a); // FIXME: Oarg could use a name, too
        }
        *s +=          "    let i = i.add_m(m);\n";
    }
    *s +=          "    i\n";
    *s +=          "}\n";
    Ok(())
}

pub fn generate(xmldata: &str, mtype: Option<&str>) -> Result<String, Box<error::Error>> {
    use xml::EventReader;
    use xml::reader::XmlEvent;

    let mut s = String::new();
    let mut curintf = None;
    let mut curm = None;
    let parser = EventReader::new(io::Cursor::new(xmldata));
    for e in parser {
        match try!(e) {
            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "interface" => {
                if curm.is_some() { try!(Err("Start of Interface inside method")) };
                if curintf.is_some() { try!(Err("Start of Interface inside interface")) };
                curintf = Some(Intf { name: try!(find_attr(attributes, "name")).into(), methods: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "interface" => {
                if curm.is_some() { try!(Err("End of Interface inside method")) };
                if curintf.is_none() { try!(Err("End of Interface outside interface")) };
                let intf = curintf.take().unwrap();
                try!(write_intf(&mut s, &intf));
                try!(write_intf_client(&mut s, &intf));
                if let Some(mt) = mtype {
                    try!(write_intf_tree(&mut s, &intf, mt));
                }
            }

            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "method" => {
                if curm.is_some() { try!(Err("Start of method inside method")) };
                if curintf.is_none() { try!(Err("Start of method outside interface")) };
                curm = Some(Method { name: try!(find_attr(attributes, "name")).into(), iargs: Vec::new(), oargs: Vec::new() });
            }
            XmlEvent::EndElement { ref name } if &name.local_name == "method" => {
                if curm.is_none() { try!(Err("End of method outside method")) };
                if curintf.is_none() { try!(Err("End of method outside interface")) };
                curintf.as_mut().unwrap().methods.push(curm.take().unwrap());
            }
            XmlEvent::StartElement { ref name, ref attributes, .. } if &name.local_name == "arg" => {
                if curm.is_none() { continue }; // this arg belongs to a signal
                if curintf.is_none() { try!(Err("Start of arg outside interface")) };
                let tp = try!(find_attr(attributes, "type")).into();
                let is_out = match find_attr(attributes, "direction") {
                    Err(_) => false,
                    Ok("in") => false,
                    Ok("out") => true,
                    _ => { try!(Err("Invalid direction")); unreachable!() }
                };
                if is_out { curm.as_mut().unwrap().oargs.push(tp) }
                else { curm.as_mut().unwrap().iargs.push((find_attr(attributes, "name").ok().map(|q| q.into()), tp)) };
            }
            _ => (),
        }
    }
    if curintf.is_some() { try!(Err("Unterminated interface")) }
    Ok(s)
}

#[cfg(test)]
mod tests {

use super::generate;

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
        let s = generate(FROM_DBUS, Some("MTSync")).unwrap();
        println!("{}", s);
        //assert_eq!(s, "fdjsf");
    }

    #[test]
    fn from_policykit() {
        let s = generate(FROM_POLICYKIT, Some("MTFn")).unwrap();
        println!("{}", s);
        /* let mut f = ::std::fs::File::create("./tests/generated/mod.rs").unwrap();
        (&mut f as &mut ::std::io::Write).write_all(s.as_bytes()).unwrap();
        drop(f); */
        // assert_eq!(s, "fdjsf");
    }

}
