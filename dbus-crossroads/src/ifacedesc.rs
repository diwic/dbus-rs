use crate::{Context, MethodErr, Crossroads};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Copy, Clone, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub struct IfaceToken(usize);

#[derive(Default, Debug)]
pub struct Registry(Vec<IfaceDesc>);

impl Registry {
    pub fn push(&mut self, x: IfaceDesc) -> IfaceToken {
        self.0.push(x);
        IfaceToken(self.0.len() - 1)
    }
    pub fn find_token(&self, name: Option<&dbus::strings::Interface>, tokens: &HashSet<IfaceToken>) -> Result<IfaceToken, MethodErr> {
        for t in tokens.iter() {
            let desc = &self.0[t.0];
            if desc.name.as_ref() == name { return Ok(*t) }
        }
        Err(name.map(MethodErr::no_interface).unwrap_or_else(|| MethodErr::no_interface("")))
    }

    pub fn take_method(&mut self, t: IfaceToken, name: &dbus::strings::Member<'static>) -> Result<Callback, MethodErr> {
        let mdesc = self.0[t.0].methods.get_mut(name).ok_or_else(|| MethodErr::no_method(name))?;
        let cb = mdesc.cb.take();
        let cb = cb.ok_or_else(|| MethodErr::failed(&format!("Detected recursive call to {}", name)))?;
        Ok(cb.0)
    }

    pub fn give_method(&mut self, t: IfaceToken, name: &dbus::strings::Member<'static>, cb: Callback) {
        let x = self.0[t.0].methods.get_mut(name).unwrap();
        x.cb = Some(CallbackDbg(cb));
    }
}

pub type Callback = Box<dyn FnMut(Context, &mut Crossroads) -> Option<Context> + Send + 'static>;

struct CallbackDbg(Callback);

impl fmt::Debug for CallbackDbg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Fn") }
}

#[derive(Debug)]
struct MethodDesc {
    cb: Option<CallbackDbg>,
}

#[derive(Debug)]
pub struct IfaceDesc {
    name: Option<dbus::strings::Interface<'static>>,
    methods: HashMap<dbus::strings::Member<'static>, MethodDesc>,
}
