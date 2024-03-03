use dbus_macros::DbusStruct;

#[derive(DbusStruct)]
pub struct DerivesDbusStruct {
    pub implements_traits: i32,
    pub doesnt_implement: DoesntDeriveDbusStruct,
}

pub struct DoesntDeriveDbusStruct {
    pub x: i32,
    pub y: i32,
}

fn main() {}
