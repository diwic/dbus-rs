use std::collections::BTreeMap;
use std::any::{TypeId, Any};
use crate::{Path as PathName, Interface as IfaceName, Member as MemberName, Signature};
use super::info::IfaceInfo;
use super::handlers::{Handlers, DebugWrapper};

#[derive(Default, Debug)]
struct IfaceReg<H: Handlers>(BTreeMap<IfaceName<'static>, (TypeId, IfaceInfo<'static, H>)>);

#[derive(Default)]
pub struct PathData<H: Handlers>(Vec<(TypeId, H::Iface)>);

impl PathData<()> {
    pub fn insert<I: Any + 'static + Send + Sync>(&mut self, i: I) {
        let id = TypeId::of::<I>();
        let t = Box::new(i);
        self.0.push((id, t));
    }
}

impl<H: Handlers> PathData<H> {
    fn new() -> Self { PathData(vec!()) }
}

//#[derive(Default, Debug)]
struct IfacePaths<H: Handlers>(BTreeMap<PathName<'static>, PathData<H>>);

impl<H: Handlers> Default for IfacePaths<H> {
    fn default() -> Self { IfacePaths(BTreeMap::new()) }
}

pub struct Crossroads<H: Handlers> {
    reg: IfaceReg<H>,
    paths: IfacePaths<H>,
}

impl<H: Handlers> Crossroads<H> {
    pub fn new() -> Self { 
        Crossroads {
            reg: IfaceReg(BTreeMap::new()),
            paths: IfacePaths(BTreeMap::new()),
        }
    }
    pub fn register_iface<I: 'static>(&mut self, info: IfaceInfo<'static, H>) -> Option<IfaceInfo<'static, H>> {
        self.reg.0.insert(info.name.clone(), (TypeId::of::<I>(), info)).map(|x| x.1)
    }
    pub fn insert_path<N: Into<PathName<'static>>>(&mut self, name: N, data: PathData<H>) {
        self.paths.0.insert(name.into(), data);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn is_send<T: Send>(_: &T) {}
        fn is_sync<T: Sync>(_: &T) {}
        let c: Crossroads<()> = Crossroads::<()>::new();
//      dbg!(&c);
        is_send(&c);
        is_sync(&c);
   }


    #[test]
    fn simple() {
        let mut cr = Crossroads::<()>::new();

        let mut pdata = PathData::new();
        pdata.insert(7u16);
        cr.insert_path("/Hello", pdata);
    }
}
