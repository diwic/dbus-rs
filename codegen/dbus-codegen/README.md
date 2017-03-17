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

```rust
pub trait OrgExampleTest {
    type Err;
    fn foo(&self, bar: i32) -> Result<String, Self::Err>;
}
```

2. It will implement this trait for `ConnPath`, which makes the method easy to call for a client, like this:

```rust
use OrgExampleTest;
let myString = try!(myConnPath.foo(myInteger));
```

3. It will create a method which returns a `tree::Interface`, given a struct that implements OrgExampleTest.
This is slightly trickier, because you need to hand out a reference to that struct, like this:

```rust
myInterface = orgexampletest_server(&myFactory, (), |minfo| { /* return a reference to the struct here */ }); 
```

I'm not certain this will be the final design of the server part.

4. For properties, get_xxx and set_xxx methods are being generated. There is currently no get_all method.

5. For signals, there is some simple code generated for emitting them. There's no code is currently generated for matching/receiving signals.

