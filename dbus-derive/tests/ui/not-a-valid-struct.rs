use dbus_macros::DbusStruct;

#[derive(DbusStruct)]
pub enum Arg {
    Opt1,
    Opt2,
}

#[derive(DbusStruct)]
pub struct Arg3;

fn main() {}
