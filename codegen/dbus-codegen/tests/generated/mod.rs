
pub trait OrgFreedesktopDBusProperties {
    fn get(&self, interfacename: &str, propertyname: &str) -> Result<::dbus::arg::Variant<Box<::dbus::arg::RefArg>>, ::dbus::Error>;
    fn get_all(&self, interfacename: &str) -> Result<::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>, ::dbus::Error>;
    fn set(&self, interfacename: &str, propertyname: &str, value: ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>) -> Result<(), ::dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=::dbus::Connection>> OrgFreedesktopDBusProperties for ::dbus::ConnPath<'a, C> {

    fn get(&self, interfacename: &str, propertyname: &str) -> Result<::dbus::arg::Variant<Box<::dbus::arg::RefArg>>, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Get".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(interfacename);
            i.append(propertyname);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: ::dbus::arg::Variant<Box<::dbus::arg::RefArg>> = try!(i.read());
        Ok(a0)
    }

    fn get_all(&self, interfacename: &str) -> Result<::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"GetAll".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(interfacename);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: ::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>> = try!(i.read());
        Ok(a0)
    }

    fn set(&self, interfacename: &str, propertyname: &str, value: ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Set".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(interfacename);
            i.append(propertyname);
            i.append(value);
        }));
        try!(m.as_result());
        Ok(())
    }
}

pub fn orgfreedesktop_dbus_properties_server<F, T, D>(factory: &::dbus::tree::Factory<::dbus::tree::MTFn<D>, D>, data: D::Interface, f: F) -> ::dbus::tree::Interface<::dbus::tree::MTFn<D>, D>
where D: ::dbus::tree::DataType, D::Method: Default, T: OrgFreedesktopDBusProperties, 
    F: 'static + for <'z> Fn(& 'z ::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>) -> & 'z T {
    let i = factory.interface("org.freedesktop.DBus.Properties", data);
    let f = ::std::sync::Arc::new(f);
    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let a1: &str = try!(i.read());
        let d = fclone(minfo);
        let r0 = try!(d.get(a0, a1));
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("Get", Default::default(), h);
    let m = m.in_arg(("interface_name", "s"));
    let m = m.in_arg(("property_name", "s"));
    let m = m.out_arg(("", "v"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let d = fclone(minfo);
        let r0 = try!(d.get_all(a0));
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("GetAll", Default::default(), h);
    let m = m.in_arg(("interface_name", "s"));
    let m = m.out_arg(("", "a{sv}"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let a1: &str = try!(i.read());
        let a2: ::dbus::arg::Variant<Box<::dbus::arg::RefArg>> = try!(i.read());
        let d = fclone(minfo);
        try!(d.set(a0, a1, a2));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("Set", Default::default(), h);
    let m = m.in_arg(("interface_name", "s"));
    let m = m.in_arg(("property_name", "s"));
    let m = m.in_arg(("value", "v"));
    let i = i.add_m(m);
    i
}

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

pub fn orgfreedesktop_dbus_introspectable_server<F, T, D>(factory: &::dbus::tree::Factory<::dbus::tree::MTFn<D>, D>, data: D::Interface, f: F) -> ::dbus::tree::Interface<::dbus::tree::MTFn<D>, D>
where D: ::dbus::tree::DataType, D::Method: Default, T: OrgFreedesktopDBusIntrospectable, 
    F: 'static + for <'z> Fn(& 'z ::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>) -> & 'z T {
    let i = factory.interface("org.freedesktop.DBus.Introspectable", data);
    let f = ::std::sync::Arc::new(f);
    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let d = fclone(minfo);
        let r0 = try!(d.introspect());
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("Introspect", Default::default(), h);
    let m = m.out_arg(("", "s"));
    let i = i.add_m(m);
    i
}

pub trait OrgFreedesktopDBusPeer {
    fn ping(&self) -> Result<(), ::dbus::Error>;
    fn get_machine_id(&self) -> Result<String, ::dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=::dbus::Connection>> OrgFreedesktopDBusPeer for ::dbus::ConnPath<'a, C> {

    fn ping(&self) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"Ping".into(), |_| {
        }));
        try!(m.as_result());
        Ok(())
    }

    fn get_machine_id(&self) -> Result<String, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"GetMachineId".into(), |_| {
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: String = try!(i.read());
        Ok(a0)
    }
}

pub fn orgfreedesktop_dbus_peer_server<F, T, D>(factory: &::dbus::tree::Factory<::dbus::tree::MTFn<D>, D>, data: D::Interface, f: F) -> ::dbus::tree::Interface<::dbus::tree::MTFn<D>, D>
where D: ::dbus::tree::DataType, D::Method: Default, T: OrgFreedesktopDBusPeer, 
    F: 'static + for <'z> Fn(& 'z ::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>) -> & 'z T {
    let i = factory.interface("org.freedesktop.DBus.Peer", data);
    let f = ::std::sync::Arc::new(f);
    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let d = fclone(minfo);
        try!(d.ping());
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("Ping", Default::default(), h);
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let d = fclone(minfo);
        let r0 = try!(d.get_machine_id());
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("GetMachineId", Default::default(), h);
    let m = m.out_arg(("", "s"));
    let i = i.add_m(m);
    i
}

pub trait OrgFreedesktopPolicyKit1Authority {
    fn enumerate_actions(&self, locale: &str) -> Result<Vec<(String, String, String, String, String, String, u32, u32, u32, ::std::collections::HashMap<String, String>)>, ::dbus::Error>;
    fn check_authorization(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), actionid: &str, details: ::std::collections::HashMap<&str, &str>, flags: u32, cancellationid: &str) -> Result<(bool, bool, ::std::collections::HashMap<String, String>), ::dbus::Error>;
    fn cancel_check_authorization(&self, cancellationid: &str) -> Result<(), ::dbus::Error>;
    fn register_authentication_agent(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), locale: &str, objectpath: &str) -> Result<(), ::dbus::Error>;
    fn register_authentication_agent_with_options(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), locale: &str, objectpath: &str, options: ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) -> Result<(), ::dbus::Error>;
    fn unregister_authentication_agent(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), objectpath: &str) -> Result<(), ::dbus::Error>;
    fn authentication_agent_response(&self, cookie: &str, identity: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error>;
    fn authentication_agent_response2(&self, uid: u32, cookie: &str, identity: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error>;
    fn enumerate_temporary_authorizations(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<Vec<(String, String, (String, ::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), u64, u64)>, ::dbus::Error>;
    fn revoke_temporary_authorizations(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error>;
    fn revoke_temporary_authorization_by_id(&self, id: &str) -> Result<(), ::dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=::dbus::Connection>> OrgFreedesktopPolicyKit1Authority for ::dbus::ConnPath<'a, C> {

    fn enumerate_actions(&self, locale: &str) -> Result<Vec<(String, String, String, String, String, String, u32, u32, u32, ::std::collections::HashMap<String, String>)>, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"EnumerateActions".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(locale);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: Vec<(String, String, String, String, String, String, u32, u32, u32, ::std::collections::HashMap<String, String>)> = try!(i.read());
        Ok(a0)
    }

    fn check_authorization(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), actionid: &str, details: ::std::collections::HashMap<&str, &str>, flags: u32, cancellationid: &str) -> Result<(bool, bool, ::std::collections::HashMap<String, String>), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"CheckAuthorization".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
            i.append(actionid);
            i.append(details);
            i.append(flags);
            i.append(cancellationid);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: (bool, bool, ::std::collections::HashMap<String, String>) = try!(i.read());
        Ok(a0)
    }

    fn cancel_check_authorization(&self, cancellationid: &str) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"CancelCheckAuthorization".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(cancellationid);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn register_authentication_agent(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), locale: &str, objectpath: &str) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"RegisterAuthenticationAgent".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
            i.append(locale);
            i.append(objectpath);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn register_authentication_agent_with_options(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), locale: &str, objectpath: &str, options: ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"RegisterAuthenticationAgentWithOptions".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
            i.append(locale);
            i.append(objectpath);
            i.append(options);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn unregister_authentication_agent(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), objectpath: &str) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"UnregisterAuthenticationAgent".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
            i.append(objectpath);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn authentication_agent_response(&self, cookie: &str, identity: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"AuthenticationAgentResponse".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(cookie);
            i.append(identity);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn authentication_agent_response2(&self, uid: u32, cookie: &str, identity: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"AuthenticationAgentResponse2".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(uid);
            i.append(cookie);
            i.append(identity);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn enumerate_temporary_authorizations(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<Vec<(String, String, (String, ::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), u64, u64)>, ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"EnumerateTemporaryAuthorizations".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let a0: Vec<(String, String, (String, ::std::collections::HashMap<String, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>), u64, u64)> = try!(i.read());
        Ok(a0)
    }

    fn revoke_temporary_authorizations(&self, subject: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>)) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"RevokeTemporaryAuthorizations".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(subject);
        }));
        try!(m.as_result());
        Ok(())
    }

    fn revoke_temporary_authorization_by_id(&self, id: &str) -> Result<(), ::dbus::Error> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.PolicyKit1.Authority".into(), &"RevokeTemporaryAuthorizationById".into(), |msg| {
            let mut i = ::dbus::arg::IterAppend::new(msg);
            i.append(id);
        }));
        try!(m.as_result());
        Ok(())
    }
}

pub fn orgfreedesktop_policy_kit1_authority_server<F, T, D>(factory: &::dbus::tree::Factory<::dbus::tree::MTFn<D>, D>, data: D::Interface, f: F) -> ::dbus::tree::Interface<::dbus::tree::MTFn<D>, D>
where D: ::dbus::tree::DataType, D::Method: Default, T: OrgFreedesktopPolicyKit1Authority, 
    F: 'static + for <'z> Fn(& 'z ::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>) -> & 'z T {
    let i = factory.interface("org.freedesktop.PolicyKit1.Authority", data);
    let f = ::std::sync::Arc::new(f);
    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let d = fclone(minfo);
        let r0 = try!(d.enumerate_actions(a0));
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("EnumerateActions", Default::default(), h);
    let m = m.in_arg(("locale", "s"));
    let m = m.out_arg(("", "a(ssssssuuua{ss})"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let a1: &str = try!(i.read());
        let a2: ::std::collections::HashMap<&str, &str> = try!(i.read());
        let a3: u32 = try!(i.read());
        let a4: &str = try!(i.read());
        let d = fclone(minfo);
        let r0 = try!(d.check_authorization(a0, a1, a2, a3, a4));
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("CheckAuthorization", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let m = m.in_arg(("action_id", "s"));
    let m = m.in_arg(("details", "a{ss}"));
    let m = m.in_arg(("flags", "u"));
    let m = m.in_arg(("cancellation_id", "s"));
    let m = m.out_arg(("", "(bba{ss})"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let d = fclone(minfo);
        try!(d.cancel_check_authorization(a0));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("CancelCheckAuthorization", Default::default(), h);
    let m = m.in_arg(("cancellation_id", "s"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let a1: &str = try!(i.read());
        let a2: &str = try!(i.read());
        let d = fclone(minfo);
        try!(d.register_authentication_agent(a0, a1, a2));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("RegisterAuthenticationAgent", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let m = m.in_arg(("locale", "s"));
    let m = m.in_arg(("object_path", "s"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let a1: &str = try!(i.read());
        let a2: &str = try!(i.read());
        let a3: ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>> = try!(i.read());
        let d = fclone(minfo);
        try!(d.register_authentication_agent_with_options(a0, a1, a2, a3));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("RegisterAuthenticationAgentWithOptions", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let m = m.in_arg(("locale", "s"));
    let m = m.in_arg(("object_path", "s"));
    let m = m.in_arg(("options", "a{sv}"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let a1: &str = try!(i.read());
        let d = fclone(minfo);
        try!(d.unregister_authentication_agent(a0, a1));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("UnregisterAuthenticationAgent", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let m = m.in_arg(("object_path", "s"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let a1: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let d = fclone(minfo);
        try!(d.authentication_agent_response(a0, a1));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("AuthenticationAgentResponse", Default::default(), h);
    let m = m.in_arg(("cookie", "s"));
    let m = m.in_arg(("identity", "(sa{sv})"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: u32 = try!(i.read());
        let a1: &str = try!(i.read());
        let a2: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let d = fclone(minfo);
        try!(d.authentication_agent_response2(a0, a1, a2));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("AuthenticationAgentResponse2", Default::default(), h);
    let m = m.in_arg(("uid", "u"));
    let m = m.in_arg(("cookie", "s"));
    let m = m.in_arg(("identity", "(sa{sv})"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let d = fclone(minfo);
        let r0 = try!(d.enumerate_temporary_authorizations(a0));
        let rm = minfo.msg.method_return();
        let rm = rm.append1(r0);
        Ok(vec!(rm))
    };
    let m = factory.method("EnumerateTemporaryAuthorizations", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let m = m.out_arg(("", "a(ss(sa{sv})tt)"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: (&str, ::std::collections::HashMap<&str, ::dbus::arg::Variant<Box<::dbus::arg::RefArg>>>) = try!(i.read());
        let d = fclone(minfo);
        try!(d.revoke_temporary_authorizations(a0));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("RevokeTemporaryAuthorizations", Default::default(), h);
    let m = m.in_arg(("subject", "(sa{sv})"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &::dbus::tree::MethodInfo<::dbus::tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let a0: &str = try!(i.read());
        let d = fclone(minfo);
        try!(d.revoke_temporary_authorization_by_id(a0));
        let rm = minfo.msg.method_return();
        Ok(vec!(rm))
    };
    let m = factory.method("RevokeTemporaryAuthorizationById", Default::default(), h);
    let m = m.in_arg(("id", "s"));
    let i = i.add_m(m);
    i
}
