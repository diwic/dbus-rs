use std::pin::Pin;
use std::sync::Arc;
use dbus::channel::Sender;
use std::future::Future;
use std::marker::PhantomData;
use crate::{Context, MethodErr, IfaceBuilder, stdimpl};
use crate::ifacedesc::Registry;
use std::collections::{BTreeMap, HashSet};
use std::any::Any;
use std::fmt;
use crate::utils::Dbg;

const INTROSPECTABLE: usize = 0;
const PROPERTIES: usize = 1;
const OBJECT_MANAGER: usize = 2;

/// Contains a reference to a registered interface.
pub struct IfaceToken<T: Send + 'static>(usize, PhantomData<&'static T>);

impl<T: Send + 'static> Clone for IfaceToken<T> {
    fn clone(&self) -> Self { IfaceToken(self.0, PhantomData) }
}
impl<T: Send + 'static> Copy for IfaceToken<T> {}
impl<T: Send + 'static> Eq for IfaceToken<T> {}
impl<T: Send + 'static> PartialEq for IfaceToken<T> {
    fn eq(&self, a: &Self) -> bool { self.0 == a.0 }
}
impl<T: Send + 'static> Ord for IfaceToken<T> {
    fn cmp(&self, a: &Self) -> std::cmp::Ordering { self.0.cmp(&a.0) }
}
impl<T: Send + 'static> PartialOrd for IfaceToken<T> {
    fn partial_cmp(&self, a: &Self) -> Option<std::cmp::Ordering> { Some(self.0.cmp(&a.0)) }
}
impl<T: Send + 'static> fmt::Debug for IfaceToken<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IfaceToken({})", self.0)
    }
}


#[derive(Debug)]
struct Object {
    ifaces: HashSet<usize>,
    data: Box<dyn Any + Send + 'static>
}

pub type BoxedSpawn = Box<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send + 'static>>) + Send + 'static>;

struct AsyncSupport {
    sender: Arc<dyn Sender + Send + Sync + 'static>,
    spawner: BoxedSpawn,
}

impl fmt::Debug for AsyncSupport {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "AsyncSupport") }
}

/// Crossroads is the "main" object, containing object paths, a registry of interfaces, and
/// a crossreference of which object paths implement which interfaces.
///
/// You can store some arbitrary data with every object path if you like. This data can then be
/// accessed from within the method callbacks. If you do not want this, just pass `()` as your data.
///
/// Crossroads can contain callbacks and data which is Send, but Sync is not required. Hence
/// Crossroads itself is Send but not Sync.
#[derive(Debug)]
pub struct Crossroads {
    map: BTreeMap<dbus::Path<'static>, Object>,
    registry: Registry,
    add_standard_ifaces: bool,
    async_support: Option<AsyncSupport>,
    object_manager_support: Option<Dbg<Arc<dyn Sender + Send + Sync + 'static>>>,
}

impl Crossroads {
    /// Create a new Crossroads instance.
    pub fn new() -> Crossroads {
        let mut cr = Crossroads {
            map: Default::default(),
            registry: Default::default(),
            add_standard_ifaces: true,
            async_support: None,
            object_manager_support: None,
        };
        let t0 = stdimpl::introspectable(&mut cr);
        let t1 = stdimpl::properties(&mut cr);
        let t2 = stdimpl::object_manager(&mut cr);
        debug_assert_eq!(t0.0, INTROSPECTABLE);
        debug_assert_eq!(t1.0, PROPERTIES);
        debug_assert_eq!(t2.0, OBJECT_MANAGER);

        // Add the root path and make it introspectable. This helps D-Bus debug tools
        cr.insert("/", &[], ());
        cr
    }

    /// If set to true (the default), will make paths implement the standard "Introspectable" and,
    /// if the path has interfaces with properties, the "Properties" interfaces.
    pub fn set_add_standard_ifaces(&mut self, enable: bool) {
        self.add_standard_ifaces = enable;
    }

    /// Registers a new interface into the interface registry. The closure receives an
    /// IfaceBuilder that you can add methods, signals and properties to.
    pub fn register<T, N, F>(&mut self, name: N, f: F) -> IfaceToken<T>
    where T: Send + 'static, N: Into<dbus::strings::Interface<'static>>,
    F: FnOnce(&mut IfaceBuilder<T>)
    {
        let iface = IfaceBuilder::build(Some(name.into()), f);
        let x = self.registry.push(iface);
        IfaceToken(x, PhantomData)
    }

    /// Access the data of a certain path.
    ///
    /// Will return none both if the path was not found, and if the found data was of another type.
    pub fn data_mut<D: Any + Send + 'static>(&mut self, name: &dbus::Path<'static>) -> Option<&mut D> {
        let obj = self.map.get_mut(name)?;
        obj.data.downcast_mut()
    }

    /// Inserts a new path.
    ///
    /// If the path already exists, it is overwritten.
    pub fn insert<'z, D, I, N>(&mut self, name: N, ifaces: I, data: D)
    where D: Any + Send + 'static, N: Into<dbus::Path<'static>>, I: IntoIterator<Item = &'z IfaceToken<D>>
    {
        let ifaces = ifaces.into_iter().map(|x| x.0);
        let mut ifaces: HashSet<usize> = std::iter::FromIterator::from_iter(ifaces);
        if self.add_standard_ifaces {
            ifaces.insert(INTROSPECTABLE);
            if ifaces.iter().any(|u| self.registry().has_props(*u)) {
                ifaces.insert(PROPERTIES);
            }
        }
        let name = name.into();
        self.map.insert(name.clone(), Object { ifaces, data: Box::new(data)});
        if let Some(oms) = self.object_manager_support.as_ref() {
            stdimpl::object_manager_path_added(oms.0.clone(), &name, self);
        }
    }

    /// Returns true if the path exists and implements the interface
    pub fn has_interface<D: Send>(&self, name: &dbus::Path<'static>, token: IfaceToken<D>) -> bool {
        self.map.get(name).map(|x| x.ifaces.contains(&token.0)).unwrap_or(false)
    }

    /// Removes an existing path.
    ///
    /// Returns None if the path was not found.
    /// In case of a type mismatch, the path will be removed, but None will be returned.
    pub fn remove<D>(&mut self, name: &dbus::Path<'static>) -> Option<D>
    where D: Any + Send + 'static {
        if let Some(oms) = self.object_manager_support.as_ref() {
            if self.map.contains_key(name) {
                stdimpl::object_manager_path_removed(oms.0.clone(), &name, self);
            }
        }
        let x = self.map.remove(name)?;
        let r: Box<D> = x.data.downcast().ok()?;
        Some(*r)
    }

    pub (crate) fn find_iface_token(&self,
        path: &dbus::Path<'static>,
        interface: Option<&dbus::strings::Interface<'static>>)
    -> Result<usize, MethodErr> {
        let obj = self.map.get(path).ok_or_else(|| MethodErr::no_path(path))?;
        self.registry.find_token(interface, &obj.ifaces)
    }

    pub (crate) fn registry(&mut self) -> &mut Registry { &mut self.registry }

    pub (crate) fn registry_and_ifaces(&self, path: &dbus::Path<'static>)
    -> (&Registry, &HashSet<usize>) {
        let obj = self.map.get(path).unwrap();
        (&self.registry, &obj.ifaces)
    }

    pub (crate) fn get_children(&self, path: &dbus::Path<'static>) -> Vec<&str> {
        use std::ops::Bound;
        let mut range = self.map.range((Bound::Excluded(path), Bound::Unbounded));
        let p2 = path.as_bytes();
        let substart = if &p2 == &b"/" { 0 } else { p2.len() };
        let mut r = vec!();
        while let Some((c, _)) = range.next() {
            if !c.as_bytes().starts_with(p2) { break; }
            let csub: &str = &c[substart..];
            if csub.len() == 0 || csub.as_bytes()[0] != b'/' { continue; }
            r.push(&csub[1..]);
        };
        r
    }

    pub (crate) fn run_async_method<F, R>(&mut self, f: F)
    where F: FnOnce(Arc<dyn Sender + Send + Sync + 'static>, &mut Crossroads) -> R,
    R: Future<Output=()> + Send + 'static
    {
        let sender = self.async_support.as_ref().expect("Async support not set").sender.clone();
        let future = f(sender, self);
        let spawner = &self.async_support.as_ref().expect("Async support not set").spawner;
        let boxed = Box::pin(async move { future.await });
        (spawner)(boxed)
    }

    fn handle_message_inner(&mut self, mut ctx: Context) -> Option<Context> {
        let (itoken, mut cb) = match ctx.check(|ctx| {
            let itoken = self.find_iface_token(ctx.path(), ctx.interface())?;
            let cb = self.registry.take_method(itoken, ctx.method())?;
            Ok((itoken, cb))
        }) {
            Ok(x) => x,
            Err(_) => return Some(ctx)
        };
        // No failure paths before method is given back!
        let methodname = ctx.method().clone();
        let ctx = cb(ctx, self);
        self.registry.give_method(itoken, &methodname, cb);
        ctx
    }

    /// Handles an incoming message call.
    ///
    /// Returns Err if the message is not a method call.
    pub fn handle_message<S: dbus::channel::Sender>(&mut self, message: dbus::Message, conn: &S) -> Result<(), ()> {
        let ctx = Context::new(message).ok_or(())?;
        if let Some(mut ctx) = self.handle_message_inner(ctx) {
            let _ = ctx.flush_messages(conn);
        }
        Ok(())
    }

    /// The token representing the built-in implementation of "org.freedesktop.DBus.Introspectable".
    pub fn introspectable<T: Send + 'static>(&self) -> IfaceToken<T> { IfaceToken(INTROSPECTABLE, PhantomData) }

    /// The token representing the built-in implementation of "org.freedesktop.DBus.Properties".
    pub fn properties<T: Send + 'static>(&self) -> IfaceToken<T> { IfaceToken(PROPERTIES, PhantomData) }

    /// The token representing the built-in implementation of "org.freedesktop.DBus.ObjectManager".
    ///
    /// You can add this to a path without enabling "set_object_manager_support", but no signals will
    /// be sent.
    pub fn object_manager<T: Send + 'static>(&self) -> IfaceToken<T> { IfaceToken(OBJECT_MANAGER, PhantomData) }

    /// Enables this crossroads instance to run asynchronous methods (and setting properties).
    ///
    /// Incoming method calls are spawned as separate tasks if necessary. This provides the necessary
    /// abstractions needed to spawn a new tasks, and to send the reply when the task has finished.
    pub fn set_async_support(&mut self, x: Option<(Arc<dyn Sender + Send + Sync + 'static>, BoxedSpawn)>) -> Option<(Arc<dyn Sender + Send + Sync + 'static>, BoxedSpawn)> {
        let a = self.async_support.take();
        self.async_support = x.map(|x| AsyncSupport {
            sender: x.0,
            spawner: x.1
        });
        a.map(|x| (x.sender, x.spawner))
    }

    /// Enables this crossroads instance to send signals when paths are added and removed.
    ///
    /// The added/removed path is a subpath of a path which implements an object manager instance.
    pub fn set_object_manager_support(&mut self, x: Option<Arc<dyn Sender + Send + Sync + 'static>>) -> Option<Arc<dyn Sender + Send + Sync + 'static>> {
        let x = x.map(|x| Dbg(x));
        std::mem::replace(&mut self.object_manager_support, x).map(|x| x.0)
    }

    /// Serve clients forever on a blocking Connection.
    ///
    /// This is a quick one-liner for the simplest case. In more advanced scenarios, you
    /// probably have to write similar code yourself.
    pub fn serve(mut self, connection: &dbus::blocking::Connection) -> Result<(), dbus::Error> {
        // We add the Crossroads instance to the connection so that incoming method calls will be handled.
        use dbus::channel::MatchingReceiver;
        connection.start_receive(dbus::message::MatchRule::new_method_call(), Box::new(move |msg, conn| {
            self.handle_message(msg, conn).unwrap();
            true
        }));

        // Serve clients forever.
        loop { connection.process(std::time::Duration::from_millis(1000))?; }
    }
}
