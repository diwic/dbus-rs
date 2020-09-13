//! Contains functionality for dispatching methods on a D-Bus "server".
//!
//! # Example
//! ```rust,no_run
//! use dbus_tree::Factory;
//! use dbus::ffidisp::Connection;
//! let f = Factory::new_fn::<()>();
//! /* Add a method returning "Thanks!" on interface "com.example.dbus.rs"
//!    on object path "/example". */
//! let t = f.tree(()).add(f.object_path("/example", ()).introspectable()
//!     .add(f.interface("com.example.dbus.rs", ())
//!         .add_m(f.method("CallMe", (), |m| {
//!             Ok(vec!(m.msg.method_return().append1("Thanks!"))) }
//!         ).out_arg("s"))
//! ));
//!
//! let c = Connection::new_session().unwrap();
//! t.set_registered(&c, true).unwrap();
//! c.add_handler(t);
//! /* Run forever */
//! loop { c.incoming(1000).next(); }
//! ```
//!
//! See `examples/server.rs` and `examples/adv_server.rs` for more thorough examples.

mod utils;
mod methodtype;
mod leaves;
mod objectpath;
mod factory;

pub use dbus::MethodErr;

pub use self::utils::{Argument, Iter};
pub use self::methodtype::{MethodInfo, PropInfo, MethodResult, MethodType, DataType, MTFn, MTFnMut, MTSync};
pub use self::leaves::{Method, Signal, Property, Access, EmitsChangedSignal};
pub use self::objectpath::{Interface, ObjectPath, Tree, TreeServer};
pub use self::factory::Factory;
