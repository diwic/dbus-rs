use std::{
    collections::{BTreeMap, HashMap},
    convert::TryFrom,
    error::Error,
};

use dbus::arg::{Arg, ArgAll};
use dbus_macros::{DbusArgs, DbusEnum, DbusPropMap, DbusStruct};

#[derive(DbusStruct, DbusArgs, Default, Debug)]
pub struct ArgsNamed {
    pub arg_struct: NestedArg,
    pub arg_vec_struct: Vec<NestedArg>,
}

#[derive(DbusStruct, DbusArgs, Default, Debug)]
pub struct ArgsUnnamed(pub NestedArg, pub Vec<NestedArg>);

#[derive(DbusStruct, Default, Debug)]
pub struct NestedArg {
    pub arg_i32: i32,
    pub arg_u32: u32,
    pub arg_string: String,
    pub arg_vec: Vec<f64>,
    pub arg_map: HashMap<i16, u16>,
    pub arg_tree: BTreeMap<i64, u64>,
    pub arg_enum: ChoiceArg,
    pub arg_props: PropsArg,
}

#[derive(DbusEnum, Debug, Default, Clone, Copy)]
#[dbus_enum(as_type = "u8")]
pub enum ChoiceArg {
    #[default]
    Choice0,
    Choice1,
}

impl From<ChoiceArg> for u8 {
    fn from(value: ChoiceArg) -> Self {
        match value {
            ChoiceArg::Choice0 => 0,
            ChoiceArg::Choice1 => 1,
        }
    }
}

impl TryFrom<u8> for ChoiceArg {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChoiceArg::Choice0),
            1 => Ok(ChoiceArg::Choice1),
            _ => Err("Invalid ChoiceArg u8 representation"),
        }
    }
}

#[derive(DbusPropMap, Debug, Default, Clone)]
pub struct PropsArg {
    pub arg1: Option<String>,
    pub arg2: Option<u32>,
}

#[test]
fn signature() -> Result<(), Box<dyn Error>> {
    let nested_sig = "(iusada{nq}a{xt}ya{sv})".to_string();
    let full_sig = format!("{nested_sig}a{nested_sig}");
    let full_sig_struct = format!("({full_sig})");

    let strs = ("arg_struct", "arg_vec_structs");
    let mut args_named_argall_sig = String::new();
    ArgsNamed::strs_sig(strs, |_, sig| {
        args_named_argall_sig += &sig.to_string();
    });
    let mut args_unnamed_argall_sig = String::new();
    ArgsUnnamed::strs_sig(strs, |_, sig| {
        args_unnamed_argall_sig += &sig.to_string();
    });

    assert_eq!(nested_sig, NestedArg::signature().to_string());
    assert_eq!(full_sig, args_named_argall_sig);
    assert_eq!(full_sig, args_unnamed_argall_sig);
    assert_eq!(full_sig_struct, ArgsNamed::signature().to_string());
    assert_eq!(full_sig_struct, ArgsUnnamed::signature().to_string());
    Ok(())
}
