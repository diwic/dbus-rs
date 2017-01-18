From a D-Bus interface like this:

    <node>
        <interface name="org.example.test">
            <method name="Foo">
                <arg type="i" name="bar" direction="in"/>
                <arg type="s" name="baz" direction="out"/>
            </method>
        </interface>
     </node>

This code will generate three things.

1. A trait, like this:

    pub trait OrgExampleTest {
        fn foo(&self, bar: i32) -> Result<String, ::dbus::Error>;
    }

2. It will implement this trait for `ConnPath`, which makes the method easy to call for a client, like this:

    use OrgExampleTest;
    let myString = try!(myConnPath.foo(myInteger));

3. It will create a method which returns a `tree::Interface`, given a struct that implements OrgExampleTest.
This is slightly trickier, because you need to hand out a reference to that struct, like this:

    myInterface = orgexampletest_server(&myFactory, (), |minfo| { /* return a reference to the struct here */ }); 

I'm not certain this will be the final design of the server part.

Also, only methods are currently supported. No code is currently generated for properties and signals.

