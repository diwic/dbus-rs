# dbus-codegen-rust

This program takes D-Bus XML Introspection data and generates Rust code
for calling and implementing the interfaces in the introspection data.

## Example

From a D-Bus interface like this:

    <node>
        <interface name="org.example.test">
            <method name="Foo">
                <arg type="i" name="bar" direction="in"/>
                <arg type="s" name="baz" direction="out"/>
            </method>
            <signal name="Laundry">
                <arg type="b" name="eaten"/>
            </signal>
        </interface>
     </node>

You can choose to generate one of three things:

 * Client-side code (for calling the interface)
 * Server-side code for implementing the interface using [dbus-crossroads](https://docs.rs/dbus-crossroads)
 * Server-side code for implementing the interface using [dbus-tree](https://docs.rs/dbus-tree)

## Common for client and server sides

 * A trait for calling/implementing the methods of the interfaces, like this:

```rust
pub trait OrgExampleTest {
    fn foo(&self, bar: i32) -> Result<String, dbus::Error>;
}
```

```rust
pub trait OrgExampleTest {
    fn foo(&self, bar: i32) -> Result<String, dbus::MethodErr>;
}
```

For properties, `get_xx` and `set_xx` methods will be generated. There is currently no `get_all` method.

 * A struct for each signal, like this:

```rust
#[derive(Debug, Default)]
pub struct OrgExampleTestLaundry {
    pub eaten: bool,
}

impl dbus::SignalArgs for OrgExampleTestLaundry { /* code here */ }
```

## Client side

 * The trait will be implemented for `blocking::Proxy`, `nonblock::Proxy` or `ffidisp::ConnPath`,
 which makes methods easy to call for a client, like this:

```rust
use OrgExampleTest;
let myString = myProxy.foo(myInteger)?;
```

 * To catch signals emitted from the server, do like this:

```rust
use dbus::SignalArgs;
myConnection.add_match(OrgExampleTestLaundry::match_rule(None, None).into_static(), |laundrySignal| {
  println!("Laundry was eaten: {:?}", laundrySignal.eaten);
})
```

## Server side - dbus-crossroads

 * A method will be generated that registers an `IfaceToken`, like this:

```rust
let token = register_org_example_test(&mut myCrossroads);
myCrossroads.insert("/", &[token], myData);
```

Where myData must be of a type that implements `OrgExampleTest`.

## Server side - dbus-tree

 * A method will be generated, which you can call to get a `tree::Interface`, like this:

```rust
myInterface = orgexampletest_server(&myFactory, ());
```

This interface can then be added to a `tree::ObjectPath`, as shown in the [main page](../README.md#server).


In addition, you also need to implement the interface's methods, like this:

```rust
impl OrgExampleTest for MyStruct {
    type Err = tree::MethodErr;
    fn foo(&self, bar: i32) -> Result<String, Self::Err> {
        /* Your code here */
    }
}
```

I've been experimenting with different ways of how to make the generated server function reach the implementing struct,
this is controlled by the command line parameter `methodaccess`.

 1. If `methodaccess` is `MethodInfo`, then you need to implement the interface for the `MethodInfo` struct, like this:

```rust
impl<M: tree::MethodType<D>, D> OrgExampleTest for tree::MethodInfo<M, D> {
    type Err = tree::MethodErr;
    fn foo(&self, bar: i32) -> Result<String, Self::Err> {
        /* Your code here */
    }
}
```

 2. If `methodaccess` is `RefClosure`, then you need to supply a closure that returns a reference to the implementing struct.
This is a good option if the struct is stored in tree (this means implementing `tree::DataType`).

```rust
myInterface = orgexampletest_server(&myFactory, (), |m| m.path.get_data());
```

 3. If `methodaccess` is `AsRefClosure`, then you need to supply a closure that returns an object which can reference to the implementing struct.
The object is dropped after the method is called. This works well with `Arc`/`Rc`, like this:

```rust
impl AsRef<dyn OrgExampleTest + 'static> for Rc<MyStruct> {
    fn as_ref(&self) -> &(dyn OrgExampleTest + 'static) { &**self }
}

let myRc = Rc::new(myStruct);
myInterface = orgexampletest_server(&myFactory, (), move |_| myRc.clone());
```

There is also a `methodtype` parameter that controls whether the server function will work well with `MTFn`, `MTFnMut` or `MTSync` trees,
or all three (called `Generic`). Or not generate a server function at all (`None`).

 * To emit a signal, you can call `SignalArgs::to_emit_message` or `ConnPath::emit` to get a message which can be sent over the connection.

# Usage

This code can be used both as a library and as a binary executable.

## Binary executable

Once you have installed dbus-codegen-rust (`cargo install dbus-codegen`), use the following command to import your XML:

```
dbus-codegen-rust < mydefinition.xml
```

This will print the generated Rust code to stdout, so you can pipe it into another file if you want:

```
dbus-codegen-rust < mydefinition.xml > mod.rs
```

Dbus-codegen-rust can also fetch the xml definition for you. Here's an example that generates client definitions for PolicyKit:

```
dbus-codegen-rust -s -d org.freedesktop.PolicyKit1 -p "/org/freedesktop/PolicyKit1/Authority" > policykit.rs
```

Dbus-codegen-rust defaults to generating client definitions. Use the `--crossroads` switch to
generate dbus-crossroads server definitions and `--methodtype` to generate dbus-tree definitions.

To see available options:

```
dbus-codegen-rust --help
```

## Library usage

```
let opts = Default::default();
let code = dbus_codegen::generate(xml_str, &opts)?;
```

See [documentation](https://docs.rs/dbus-codegen/) for what options are available.

# Features

The `dbus` feature is enabled by default. If you turn it off (with the `--no-default-features` argument to cargo),
this program (or library) no longer binds to the D-Bus C development headers, meaning you don't need these to be installed.
This also means you can no longer fetch the xml definition from other programs when you run the binary.
