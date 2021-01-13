use dbus_crossroads::Crossroads;

#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_cr;

impl policykit_cr::DBusProperties for () {
    fn get(&self, interfacename: String, propertyname: String) -> Result<dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>, dbus::MethodErr> {
        assert_eq!(interfacename, "Interface.Name");
        assert_eq!(propertyname, "Property.Name");
        Ok(::dbus::arg::Variant(Box::new(5u8)))
    }

    fn get_all(&self, _interfacename: String) ->
    Result<::std::collections::HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>, dbus::MethodErr> { unimplemented!() }

    fn set(&self, _interfacename: String, _propertyname: String, value: dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>) -> Result<(), dbus::MethodErr> {
        assert_eq!(dbus::arg::RefArg::as_str(&value), Some("Hello"));
        Err(("A.B.C", "Error.Message").into())
    }
}


#[test]
fn test_cr() {
    let mut cr = Crossroads::new();
    let token = policykit_cr::register_dbus_properties::<()>(&mut cr);
    cr.insert("/", &[token], ());
    // TODO
}
