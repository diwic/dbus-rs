use crate::IfaceDesc;

pub fn introspectable() -> IfaceDesc {
    IfaceDesc::new("org.freedesktop.DBus.Introspectable", None)
}
