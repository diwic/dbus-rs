#![warn(missing_docs)]

//! Procedural macros for [dbus] crate
//!
//! Simplifies definition of complex dbus interfaces
//!
//! [dbus]: ../dbus/index.html

mod derive_args;
mod derive_enum;
mod derive_propmap;
mod derive_struct;
mod util;

use darling::FromDeriveInput;
use proc_macro_error::proc_macro_error;
use syn::{parse_macro_input, DeriveInput};

use crate::derive_args::{derive_args, DbusArgs};
use crate::derive_enum::{derive_enum, DbusEnum};
use crate::derive_propmap::{derive_propmap, DbusPropmap};
use crate::derive_struct::{derive_struct, DbusStruct};
use crate::util::derive_input_style_span;

/// Implements [`Arg`], [`Get`] and [`Append`] for an arbitrary struct.
///
/// Expects every field type to implement [`Arg`], [`Get`] and [`Append`].
///
/// # Examples
/// ```
/// use dbus::arg::Arg;
/// use dbus_macros::DbusStruct;
///
/// // Taken from org.freedesktop.ColorHelper UpdateGamma signal
/// #[derive(DbusStruct)]
/// struct Gamma {
///     red: f64,
///     green: f64,
///     blue: f64
/// }
///
/// assert_eq!(
///     "(ddd)",
///     Gamma::signature().to_string().as_str()
/// );
///
/// ```
///
/// [`Arg`]: ../dbus/arg/trait.Arg.html
/// [`Get`]: ../dbus/arg/trait.Get.html
/// [`Append`]: ../dbus/arg/trait.Append.html
#[proc_macro_derive(DbusStruct, attributes(dbus_struct))]
#[proc_macro_error]
pub fn derive_dbus_struct(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match DbusStruct::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => {
            return err.with_span(&derive_input_style_span(input)).write_errors().into();
        }
    };
    derive_struct(input).into()
}

/// Implements [`ArgAll`], [`ReadAll`] and [`AppendAll`] for an arbitrary struct.
///
/// # Examples
/// ```
/// use dbus_macros::DbusArgs;
/// use dbus::arg::{ArgAll, PropMap};
///
/// // Taken from org.freedesktop.portal.Desktop OpenURI method.
/// #[derive(DbusArgs)]
/// struct OpenURIArgs {
///     parent_window: String,
///     uri: String,
///     options: PropMap
/// }
///
/// let mut openuri_signature = String::new();
/// OpenURIArgs::strs_sig(("parent_window", "uri", "options"), |_, sig| {
///     openuri_signature += &sig.to_string();
/// });
/// assert_eq!(
///     "ssa{sv}",
///     openuri_signature
/// );
///
/// ```
///
/// [`ArgAll`]: ../dbus/arg/trait.ArgAll.html
/// [`ReadAll`]: ../dbus/arg/trait.ReadAll.html
/// [`AppendAll`]: ../dbus/arg/trait.AppendAll.html
#[proc_macro_derive(DbusArgs, attributes(dbus_args))]
#[proc_macro_error]
pub fn derive_dbus_args(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match DbusArgs::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => {
            return err.with_span(&derive_input_style_span(input)).write_errors().into();
        }
    };
    derive_args(input).into()
}

/// Implements [`Arg`], [`Get`] and [`Append`] for an enum that will behave like a different type.
///
/// Expects trait implementation of [`From<EnumType>`] for mapped type and
/// [`TryFrom<MappedType>`] for enum type.
///
/// # Attributes
/// * `#[dbus_enum(as_type = "u8")]`: Maps given enum to [`u8`]
///
/// # Examples
/// ```
/// use dbus_macros::DbusEnum;
/// use dbus::arg::Arg;
/// use std::convert::TryFrom;
///
/// // Taken from org.freedesktop.systemd1.Manager SystemState method.
/// // Removed some options to keep example small.
/// #[derive(DbusEnum, Clone, Copy)]
/// #[dbus_enum(as_type = "String")]
/// enum SystemdSystemState {
///     Starting,
///     Running,
///     Stopping
/// }
///
/// impl From<SystemdSystemState> for String {
///     fn from(value: SystemdSystemState) -> Self {
///         use SystemdSystemState::*;
///         match value {
///             Starting => "starting",
///             Running => "running",
///             Stopping => "stopping"
///         }.to_string()
///     }
/// }
///
/// impl TryFrom<String> for SystemdSystemState {
///     type Error = &'static str;
///
///     fn try_from(value: String) -> Result<Self, Self::Error> {
///         use SystemdSystemState::*;
///         match value.as_str() {
///             "starting" => Ok(Starting),
///             "running" => Ok(Running),
///             "stopping" => Ok(Stopping),
///             _ => Err("Unexpected system state")
///         }
///     }
/// }
///
/// assert_eq!(
///     "s",
///     SystemdSystemState::signature().to_string().as_str()
/// );
/// ```
///
/// [`Arg`]: ../dbus/arg/trait.Arg.html
/// [`Get`]: ../dbus/arg/trait.Get.html
/// [`Append`]: ../dbus/arg/trait.Append.html
/// [`TryFrom<MappedType>`]: std::convert::TryFrom
#[proc_macro_derive(DbusEnum, attributes(dbus_enum))]
#[proc_macro_error]
pub fn derive_dbus_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match DbusEnum::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => {
            return err.with_span(&derive_input_style_span(input)).write_errors().into();
        }
    };
    derive_enum(input).into()
}

/// Implements [`Arg`], [`Get`] and [`Append`] for a struct that behaves like [`PropMap`].
///
/// Can be derived for a struct where every field is an option, uses field name as a key for
/// accessing [`PropMap`].
///
/// # Field attributes
/// * `#[dbus_propmap(rename="key-name")]`: Overrides field name with given string for accessing
/// [`PropMap`].
///
/// # Examples
/// ```
/// use dbus_macros::DbusPropMap;
/// use dbus::arg::Arg;
///
/// // Taken from org.freedesktop.Flatpak.SessionHelper RequestSession method.
/// #[derive(DbusPropMap)]
/// struct FlatpakRequestSessionReturn {
///     path: Option<String>,
///     #[dbus_propmap(rename="pkcs11-socket")]
///     pkcs11_socket: Option<String>
/// }
///
/// assert_eq!(
///     "a{sv}",
///     FlatpakRequestSessionReturn::signature().to_string().as_str()
/// );
/// ```
///
/// [`Arg`]: ../dbus/arg/trait.Arg.html
/// [`Get`]: ../dbus/arg/trait.Get.html
/// [`Append`]: ../dbus/arg/trait.Append.html
/// [`PropMap`]: ../dbus/arg/type.PropMap.html
#[proc_macro_derive(DbusPropMap, attributes(dbus_propmap))]
#[proc_macro_error]
pub fn derive_dbus_propmap(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match DbusPropmap::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => {
            return err.with_span(&derive_input_style_span(input)).write_errors().into();
        }
    };
    derive_propmap(input).into()
}
