mod generated {
    // This is the output of dbus-codegen-rust.
    // Usually, code generated is put in its own file. It is inlined here for convenience.

    #[allow(unused_imports)]
    use dbus::arg;
    use dbus_crossroads as crossroads;

    pub trait OrgExampleTest {
        fn foo(&mut self, bar: i32) -> Result<String, dbus::MethodErr>;
    }

    #[derive(Debug)]
    pub struct OrgExampleTestLaundry {
        pub eaten: bool,
    }

    impl arg::AppendAll for OrgExampleTestLaundry {
        fn append(&self, i: &mut arg::IterAppend) {
            arg::RefArg::append(&self.eaten, i);
        }
    }

    impl arg::ReadAll for OrgExampleTestLaundry {
        fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
            Ok(OrgExampleTestLaundry {
                eaten: i.read()?,
            })
        }
    }

    impl dbus::message::SignalArgs for OrgExampleTestLaundry {
        const NAME: &'static str = "Laundry";
        const INTERFACE: &'static str = "org.example.test";
    }

    pub fn register_org_example_test<T>(cr: &mut crossroads::Crossroads) -> crossroads::IfaceToken<T>
    where T: OrgExampleTest + Send + 'static
    {
        cr.register("org.example.test", |b| {
            b.signal::<(bool,), _>("Laundry", ("eaten",));
            b.method("Foo", ("bar",), ("baz",), |_, t: &mut T, (bar,)| {
                t.foo(bar,)
                    .map(|x| (x,))
            });
        })
    }
}

impl generated::OrgExampleTest for () {
    fn foo(&mut self, bar: i32) -> Result<String, dbus::MethodErr> {
        Ok(format!("You called me with the argument '{}'", bar))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let mut cr = dbus_crossroads::Crossroads::new();
    let token = generated::register_org_example_test(&mut cr);
    cr.insert("/", &[token], ());

    let conn = dbus::blocking::Connection::new_session()?;
    conn.request_name("org.example.test", true, true, true)?;

    cr.serve(&conn)?;
    Ok(())
}
