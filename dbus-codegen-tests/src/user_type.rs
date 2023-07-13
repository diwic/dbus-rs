use dbus::arg::{Append, IterAppend, Iter, Arg, ArgType, Get, RefArg};
use dbus::strings::Signature;
use std::any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyType
{
    f: (String, String),
}

impl MyType {
    pub fn new(s1: String, s2: String) -> Self {
        MyType{f:(s1, s2)}
    }
}

impl Append for MyType {
    fn append_by_ref(&self, i: &mut IterAppend) {
        self.f.append_by_ref(i);
    }
}

impl Arg for MyType {
    const ARG_TYPE: ArgType = ArgType::Struct;
    fn signature() -> Signature<'static> { 
        Signature::new("(ss)".to_string()).unwrap()
    }
}

impl<'a> Get<'a> for MyType {
    fn get(i: &mut Iter<'a>) -> Option<MyType> { 
        let f = <(String, String)>::get(i)?;
        Some(MyType{f})
     }
}

impl RefArg for MyType {
    fn arg_type(&self) -> ArgType {
        <MyType as Arg>::ARG_TYPE
    }
    fn signature(&self) -> Signature<'static> {
        <MyType as Arg>::signature()
    }
    fn append(&self, i: &mut IterAppend) {
        <MyType as Append>::append_by_ref(self, i)
    }
    fn as_any(&self) -> &dyn any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn any::Any {
        self
    }
    fn as_i64(&self) -> Option<i64> {
        None
    }
    fn as_u64(&self) -> Option<u64> {
        None
    }
    fn as_f64(&self) -> Option<f64> {
        None
    }
    fn as_str(&self) -> Option<&str> {
        None
    }
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(self.clone())
    }
    fn array_clone(v: &[Self]) -> Option<Box<dyn RefArg + 'static>>
    where
        Self: Sized,
    {
        Some(Box::new(v.to_vec()))
    }
}
