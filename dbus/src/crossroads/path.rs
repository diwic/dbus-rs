use super::handlers::Handlers;
use crate::strings::Path as PathName;
use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::fmt;

/// Internal helper trait, required due to orphan rules
///
/// Just about anything implements this trait, i e, anything can be put inside a path.
pub trait PathData<T>: Sized {
    /// Internal helper trait
    fn pack(self) -> T;
    fn unpack(t: &T) -> Option<&Self>;
    fn unpack_mut(t: &mut T) -> Option<&mut Self>;
}

impl<S: 'static + Send + Sync> PathData<Box<dyn Any + Send + Sync>> for S {
    fn pack(self) -> Box<dyn Any + Send + Sync> { Box::new(self) }
    fn unpack(t: &Box<dyn Any + Send + Sync>) -> Option<&S> { (&*t).downcast_ref() }
    fn unpack_mut(t: &mut Box<dyn Any + Send + Sync>) -> Option<&mut S> { (&mut *t).downcast_mut() }
}

impl<S: 'static + Send> PathData<Box<dyn Any + Send>> for S {
    fn pack(self) -> Box<dyn Any + Send> { Box::new(self) }
    fn unpack(t: &Box<dyn Any + Send>) -> Option<&S> { (&*t).downcast_ref() }
    fn unpack_mut(t: &mut Box<dyn Any + Send>) -> Option<&mut S> { (&mut *t).downcast_mut() }
}

impl<S: 'static> PathData<Box<dyn Any>> for S {
    fn pack(self) -> Box<dyn Any> { Box::new(self) }
    fn unpack(t: &Box<dyn Any>) -> Option<&S> { (&*t).downcast_ref() }
    fn unpack_mut(t: &mut Box<dyn Any>) -> Option<&mut S> { (&mut *t).downcast_mut() }
}

pub struct Path<H: Handlers> {
    name: PathName<'static>,
    data: HashMap<TypeId, H::Iface>,
}

impl<H: Handlers> Path<H> {
    pub fn new<N: Into<PathName<'static>>>(n: N) -> Self { Path { name: n.into(), data: Default::default() } }

    pub fn name(&self) -> &PathName<'static> { &self.name }

    pub fn insert<I: 'static + PathData<H::Iface>>(&mut self, i: I) {
        let id = TypeId::of::<I>();
        let t = i.pack();
        self.data.insert(id, t);
    }

    pub fn with<I: 'static + PathData<H::Iface>>(mut self, i: I) -> Self {
        self.insert(i); self
    }

    pub fn remove<I: 'static + PathData<H::Iface>>(&mut self) {
        let id = TypeId::of::<I>();
        self.data.remove(&id);
    }

    pub fn get<I: 'static + PathData<H::Iface>>(&self) -> Option<&I> {
        let id = TypeId::of::<I>();
        self.data.get(&id).and_then(|x| { PathData::unpack(x) })
    }

    pub fn get_mut<I: 'static + PathData<H::Iface>>(&mut self) -> Option<&mut I> {
        let id = TypeId::of::<I>();
        self.data.get_mut(&id).and_then(|x| { PathData::unpack_mut(x) })
    }

    pub (super) fn get_from_typeid(&self, id: TypeId) -> Option<&H::Iface> {
        self.data.get(&id)
    }

    pub (super) fn get_from_typeid_mut(&mut self, id: TypeId) -> Option<&mut H::Iface> {
        self.data.get_mut(&id)
    }
}

impl<H: Handlers> fmt::Debug for Path<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Path({:?})", self.name) }
}

#[test]
fn test_path() {
    let mut p: Path<super::handlers::Par> = Path::new("/hello");
    p.insert(7u16);
    *p.get_mut::<u16>().unwrap() += 1;
    let x: u16 = *p.get().unwrap();
    assert_eq!(x, 8u16);
}
