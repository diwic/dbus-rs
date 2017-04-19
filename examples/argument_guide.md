The different ways you can append and get message arguments can be a bit bewildering. I've iterated a few times on the design and didn't want to lose backwards compatibility.

This guide is to help you on your way. In addition, many of the examples in the examples directory append and read arguments.

Code generation
---------------

Preamble - what's emerging is code generation. It's far from done, but if you have D-Bus introspection data, run it through the dbus-codegen tool (look in the codegen directory) and get Rust code out. There is pre-generated code for standard D-Bus interfaces in the `stdintf` module. Here's an example:

```rust
let c = try!(Connection::get_private(BusType::Session));

// Make a "ConnPath" struct that just contains a Connection, a destination and a path.
let p = c.with_path("org.mpris.MediaPlayer2.rhythmbox", "/org/mpris/MediaPlayer2", 5000);

// The ConnPath struct implements many traits, e g `org.freedesktop.DBus.Properties`. Bring the trait into scope.
use stdintf::OrgFreedesktopDBusProperties;

// Now we can call org.freedesktop.DBus.Properties.Get just like an ordinary method and get the result back.
let metadata = try!(p.get("org.mpris.MediaPlayer2.Player", "Metadata"));
```

For server side there is no pre-generated code yet and its implementation is slightly more complex.

Codegen isn't really there yet for many use cases though, so let's move on:

Append / get basic types
------------------------

If you just want to get/append simple types, just use `append1` / `append2` / `append3`, and 
`read1` / `read2` / `read3`. The imaginary method below takes one byte parameter and one string parameter, and returns one string parameter and one int parameter.

```rust
let m = try!(Message::new_method_call(dest, path, intf, member)).append2(5u8, "Foo");
let r = try!(c.send_with_reply_and_block(m, 2000));
let (data1, data2): (&str, i32) = try!(c.read2());
```

Arrays and dictionaries
-----------------------

D-Bus arrays and dictionaries usually correspond to `Vec` and `HashMap`. You can just append and get them like basic types:

```rust
let v = vec![3i32, 4i32, 5i32];
let mut map = HashMap::new();
map.insert("Funghi", 5u16);
map.insert("Mold", 8u16);

let m = try!(Message::new_method_call(dest, path, intf, member)).append2(v, map);
let r = try!(c.send_with_reply_and_block(m, 2000));
let (data1, data2): (Vec<i32>, HashMap<&str, u16>) = try!(r.read2());
```

Or combine them as you wish, e g, use a `Vec<Vec<u8>>`, a `HashMap<u64, Vec<String>>` or `HashMap<String, HashMap<String, i32>>` to construct more difficult types.

Slices can sometimes be used as arrays - e g, `&[&str]` can be appended, but only very simple types can be used with `get` and `read`, e g `&[u8]`.

This is the easiest way to get started, but in case you want to avoid the overhead of creating `Vec` or `HashMap`s, the "Array and Dict types" and "Iter / IterAppend" sections offer useful alternatives.

Variants
--------

Things are getting slightly more complex with Variants, because they are not strongly typed and thus not fit as well into Rust's strongly typed as arrays and dicts.

If you know the type beforehand, it's still easy:

```rust
let v = Variant("This is a variant containing a &str");
let m = try!(Message::new_method_call(dest, path, intf, member)).append1(v);
let r = try!(c.send_with_reply_and_block(m, 2000));
let z: Variant<i32> = try!(r.read1());
println!("Method returned {}", z.0);
```

But sometimes you don't know the type beforehand. We can solve this in two ways (choose whichever is more appropriate for your use case), either through the trait object `Box<RefArg>` or through `Iter` / `IterAppend` (see later sections).

Through trait objects:

```rust
let x = Box::new(5000i32) as Box<RefArg>;
let m = try!(Message::new_method_call(dest, path, intf, member)).append1(Variant(x));
let r = try!(c.send_with_reply_and_block(m, 2000));
let z: Variant<Box<RefArg>> = try!(r.read1());
```

Ok, so we retrieved our `Box<RefArg>`. We now need to use the `RefArg` methods to probe it, to see what's inside. Easiest is to use `as_i64` or `as_str` if you want to test for integer or string types. Use `arg::cast` for floating point values, and `as_iter` if the variant contains a complex type you need to iterate through. Or match over `arg_type` if you need to know the exact type.

Dicts and variants are sometimes combined, e g, you might need to read a D-Bus dictionary of String to Variants. You can then read these as `HashMap<String, Variant<Box<RefArg>>>`.

Structs
-------

D-Bus structs are implemented as Rust tuples. You can append and get tuples like you do with other types of arguments.

TODO: Example

Declare method arguments
------------------------

When you make a `Tree`, you want to declare what input and output arguments your method expects - so that correct D-Bus introspection data can be generated. You'll use the same types as you learned earlier in this guide:

```rust
factory.method( /* ... */ )
.inarg::<HashMap<i32, Vec<(i32, bool, String)>>,_>("request")
.outarg::<&str,_>("reply")
```

The types are just for generating a correct signature, they are never instantiated. Many different types can generate the same signature - e g, `Array<u8, _>`, `Vec<u8>` and `&[u8]` will all generate the same signature. `Variant` will generate the same type signature regardless of what's inside, so just write `Variant<()>` for simplicity.


Iter / IterAppend
-----------------

Iter and IterAppend are more low-level, direct methods to get and append arguments. They can, e g, come handy if you have more than five arguments to read.

E g, for appending a variant with IterAppend you can use `IterAppend::new(&msg).append_variant(|i| i.append(5000i32))` to append what you need to your variant inside the closure.
To read a variant you can use `let i = try!(msg.read1::<Variant<Iter>>::())` and then examine the methods on `i.0` to probe the variant.

Array and Dict types
--------------------

These provide slightly better flexibility than using `Vec` and `HashMap` by instead integrating with `Iterator`. Here's an example where you can append and get a dictionary without having to create a HashMap:

```rust
let x = &[("Hello", true), ("World", false)];
let m = try!(Message::new_method_call(dest, path, intf, member)).append1(Dict::new(x));
let r = try!(c.send_with_reply_and_block(m, 2000));
let z: Dict<i32, &str, _> = try!(r.read1());
for (key, value) in z { /* do something */ }
```

An edge case where this is necessary is having floating point keys in a dictionary - this is supported in D-Bus but not in Rust's `HashMap`. I have never seen this in practice, though.

Unusual types
-------------

The types `Path`, `Signature` and `OwnedFd` are not often used, but they can be appended and read as other argument types. `Path` and `Signature` will return strings with a borrowed lifetime - use `.into_static()` if you want to untie that lifetime.

For `OwnedFd`, which a wrapper around a file descriptor, remember that the file descriptor will be closed when it goes out of scope.

MessageItem
-----------

MessageItem was the first design - an enum representing a D-Bus argument. It still works, but I doubt you'll ever need to use it. Newer methods provide better type safety, speed, and ergonomics.


