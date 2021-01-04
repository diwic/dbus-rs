#[allow(dead_code)]
#[deny(trivial_casts)]
mod policykit_client;

use dbus::arg::{RefArg, Variant};
use policykit_client::OrgFreedesktopPolicyKit1AuthorityProperties;
use std::collections::HashMap;

#[test]
fn test_parse_properties() {
    let mut properties: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
    properties.insert("BackendFeatures".to_string(), Variant(Box::new(42u32)));
    properties.insert(
        "BackendName".to_string(),
        Variant(Box::new("name".to_string())),
    );
    let mut interfaces = HashMap::new();
    interfaces.insert(
        "org.freedesktop.PolicyKit1.Authority".to_string(),
        properties,
    );

    let authority_properties =
        OrgFreedesktopPolicyKit1AuthorityProperties::from_interfaces(&interfaces).unwrap();
    assert_eq!(authority_properties.backend_features(), Some(42));
    assert_eq!(
        authority_properties.backend_name().cloned(),
        Some("name".to_string())
    );
    assert_eq!(authority_properties.backend_version(), None);
}
