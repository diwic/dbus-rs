//! This module contains some standard interfaces and an easy way to call them.
//!
//! See the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces) for more information about these standard interfaces.
//! 
//! The code here was originally created by dbus-codegen.
//! Signal handling is not yet implemented here - use `Message::signal` to emit or `Connection.add_match` to receive, as usual.
//!
//! # Example
//! ```
//! use dbus::{Connection, BusType};
//! use dbus::stdintf::OrgFreedesktopDBusIntrospectable;
//! let c = Connection::get_private(BusType::Session).unwrap();
//! let p = c.with_path("org.freedesktop.DBus", "/", 10000);
//! println!("Introspection XML: {}", p.introspect().unwrap());
//! ```
//!

#![allow(missing_docs)]

use arg;

/// Methods of the [org.freedesktop.DBus.Peer](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-peer) interface.
pub trait OrgFreedesktopDBusPeer {
    type Err;
    fn ping(&self) -> Result<(), Self::Err>;
    fn get_machine_id(&self) -> Result<String, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=super::Connection>> OrgFreedesktopDBusPeer for super::ConnPath<'a, C> {
    type Err = super::Error;

    fn ping(&self) -> Result<(), Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"Ping".into(), |_| {
        }));
        try!(m.as_result());
        Ok(())
    }

    fn get_machine_id(&self) -> Result<String, Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"GetMachineId".into(), |_| {
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let machine_uuid: String = try!(i.read());
        Ok(machine_uuid)
    }
}

/// Method of the [org.freedesktop.DBus.Introspectable](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-introspectable) interface.
pub trait OrgFreedesktopDBusIntrospectable {
    type Err;
    fn introspect(&self) -> Result<String, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=super::Connection>> OrgFreedesktopDBusIntrospectable for super::ConnPath<'a, C> {
    type Err = super::Error;

    fn introspect(&self) -> Result<String, Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Introspectable".into(), &"Introspect".into(), |_| {
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let arg0: String = try!(i.read());
        Ok(arg0)
    }
}

/// Methods of the [org.freedesktop.DBus.Properties](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties) interface.
pub trait OrgFreedesktopDBusProperties {
    type Err;
    fn get(&self, interface_name: &str, property_name: &str) -> Result<arg::Variant<Box<arg::RefArg>>, Self::Err>;
    fn get_all(&self, interface_name: &str) -> Result<::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>>, Self::Err>;
    fn set(&self, interface_name: &str, property_name: &str, value: arg::Variant<Box<arg::RefArg>>) -> Result<(), Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=super::Connection>> OrgFreedesktopDBusProperties for super::ConnPath<'a, C> {
    type Err = super::Error;

    fn get(&self, interface_name: &str, property_name: &str) -> Result<arg::Variant<Box<arg::RefArg>>, Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Get".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
            i.append(property_name);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let value: arg::Variant<Box<arg::RefArg>> = try!(i.read());
        Ok(value)
    }

    fn get_all(&self, interface_name: &str) -> Result<::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>>, Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"GetAll".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let properties: ::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>> = try!(i.read());
        Ok(properties)
    }

    fn set(&self, interface_name: &str, property_name: &str, value: arg::Variant<Box<arg::RefArg>>) -> Result<(), Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Set".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
            i.append(property_name);
            i.append(value);
        }));
        try!(m.as_result());
        Ok(())
    }
}

/// Method of the [org.freedesktop.DBus.ObjectManager](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-objectmanager) interface.
pub trait OrgFreedesktopDBusObjectManager {
    type Err;
    fn get_managed_objects(&self) -> Result<::std::collections::HashMap<super::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>>>>, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=super::Connection>> OrgFreedesktopDBusObjectManager for super::ConnPath<'a, C> {
    type Err = super::Error;

    fn get_managed_objects(&self) -> Result<::std::collections::HashMap<super::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>>>>, Self::Err> {
        let mut m = try!(self.method_call_with_args(&"org.freedesktop.DBus.ObjectManager".into(), &"GetManagedObjects".into(), |_| {
        }));
        try!(m.as_result());
        let mut i = m.iter_init();
        let objects: ::std::collections::HashMap<super::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<arg::RefArg>>>>> = try!(i.read());
        Ok(objects)
    }
}

