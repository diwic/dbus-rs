extern crate dbus_codegen;

use dbus_codegen::{generate, ServerAccess, GenOpts, ConnectionType};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

static POLICYKIT_XML: &'static str = r#"
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

static DEPRECATED_XML: &'static str = r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
                      "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="com.github.dbus_rs.dbus_codegen1">
    <annotation name="org.freedesktop.DBus.Deprecated" value="this interface is deprecated"/>
    <method name="Method1">
      <annotation name="org.freedesktop.DBus.Deprecated" value="this method is deprecated"/>
      <arg type="s" name="inbound" direction="in"/>
      <arg type="s" name="outbound" direction="in"/>
    </method>
    <signal name="Signal1">
      <annotation name="org.freedesktop.DBus.Deprecated" value="this signal is deprecated"/>
      <arg type="s" name="signal_name"/>
    </signal>
    <property name="property_name" type="s" access="readwrite">
      <annotation name="org.freedesktop.DBus.Deprecated" value="this property is deprecated"/>
    </property>
    <property name="property_true" type="s" access="readwrite">
      <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="true"/>
    </property>
    <property name="property_invalidates" type="s" access="readwrite">
      <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="invalidates"/>
    </property>
    <property name="property_false" type="s" access="readwrite">
      <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="false"/>
    </property>
  </interface>
</node>
"#;

static USER_TYPES_XML: &'static str = r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
                      "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="com.example.MyService1.InterestingInterface">
    <method name="Method1">
      <arg name="arg1" direction="in" type="s">
        <annotation name="rs.dbus.ArgType" value="codegen_tests::user_type::MyType"/>
      </arg>
      <arg name="outarg2" direction="out" type="u">
        <annotation name="rs.dbus.ArgType" value="codegen_tests::user_type::MyType"/>
      </arg>
    </method>

    <signal name="Signal1">
      <arg name="arg1" type="s">
        <annotation name="rs.dbus.ArgType" value="codegen_tests::user_type::MyType"/>
      </arg>
    </signal>

    <property name="Bar" type="y" access="readwrite">
      <annotation name="rs.dbus.ArgType" value="codegen_tests::user_type::MyType"/>
    </property>
  </interface>
</node>
"#;

fn write_to_file(code: &str, path: &Path) {
    let mut f = File::create(path).unwrap();
    Write::write_all(&mut f,code.as_bytes()).unwrap();
}

fn generate_code(xml: &str, opts: &GenOpts, outfile: &str) {
    let code = generate(xml, opts).unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let path = Path::new(&out_dir).join(outfile);
    write_to_file(&code, &path);
}

fn main() {
    let ffidisp = GenOpts {
        connectiontype: ConnectionType::Ffidisp,
        ..Default::default()
    };
    generate_code(POLICYKIT_XML, &ffidisp, "policykit.rs");

    let blocking_client = GenOpts {
        connectiontype: ConnectionType::Blocking,
        methodtype: None,
        ..Default::default()
    };
    generate_code(POLICYKIT_XML, &blocking_client, "policykit_blocking.rs");
    generate_code(USER_TYPES_XML, &blocking_client, "user_types_blocking.rs");

    let nonblock_client = GenOpts {
        connectiontype: ConnectionType::Nonblock,
        methodtype: None,
        ..Default::default()
    };
    generate_code(POLICYKIT_XML, &nonblock_client, "policykit_nonblock.rs");
    generate_code(USER_TYPES_XML, &nonblock_client, "user_types_nonblock.rs");

    let mut g = GenOpts {
        methodtype: Some("MTFnMut".into()),
        serveraccess: ServerAccess::AsRefClosure,
        connectiontype: ConnectionType::Ffidisp,
        ..Default::default()
    };
    generate_code(POLICYKIT_XML, &g, "policykit_asref.rs");
    generate_code(DEPRECATED_XML, &g, "deprecated.rs");

    g.methodtype = Some("MTSync".into());
    generate_code(POLICYKIT_XML, &g, "policykit_asref_mtsync.rs");

    g.methodtype = None;
    g.propnewtype = true;
    generate_code(POLICYKIT_XML, &g, "policykit_client.rs");
    generate_code(USER_TYPES_XML, &g, "user_types_client.rs");

    g.crossroads = true;
    g.propnewtype = false;
    g.skipprefix = Some("org.freedesktop".into());
    generate_code(POLICYKIT_XML, &g, "policykit_cr.rs");
    generate_code(DEPRECATED_XML, &g, "deprecated_cr.rs");
    generate_code(USER_TYPES_XML, &g, "user_types_cr.rs");
}
