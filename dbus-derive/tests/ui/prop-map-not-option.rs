use dbus_macros::DbusPropMap;

#[derive(DbusPropMap)]
pub struct PropsArg {
    pub arg1: String,
    pub arg2: i32,
}

fn main() {}
