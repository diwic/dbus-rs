use dbus::{
    arg::{Append, Iter, IterAppend, PropMap, Variant},
    Message,
};
use dbus_macros::DbusPropMap;

#[derive(DbusPropMap)]
struct MappedToPropMap {
    field1: Option<String>,
    #[dbus_propmap(rename = "field2-renamed")]
    field2_renamed: Option<String>,
}

#[test]
fn propmap_to_struct() {
    let mut map = PropMap::new();

    let field1 = "foo".to_string();
    let field2 = "bar".to_string();

    map.insert("field1".to_string(), Variant(Box::new(field1.clone())));
    map.insert("field2-renamed".to_string(), Variant(Box::new(field2.clone())));

    let mut m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let mut ia = IterAppend::new(&mut m);
    map.append(&mut ia);

    let mut i = Iter::new(&m);
    let res: MappedToPropMap = i.read().unwrap();

    assert_eq!(Some(field1), res.field1);
    assert_eq!(Some(field2), res.field2_renamed);
}
