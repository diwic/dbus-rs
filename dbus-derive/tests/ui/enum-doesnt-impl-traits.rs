use dbus_macros::DbusEnum;

#[derive(DbusEnum)]
#[dbus_enum(as_type = "u8")]
pub enum EnumArg {
    Opt0,
    Opt1,
}

fn main() {}
