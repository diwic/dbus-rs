= Problems of implementing a D-Bus server in Rust =

I'm writing this down to see if I can get a better idea of how to implement things,
or as a cautionary tale in case I decide to give up.

== Overview ==

Okay, so when there is a D-Bus method call coming in that you're supposed to handle,
that method call comes with three parameters, path, interface and method.

The idea is that you should first find an object path to locate an "Object",
then locate the interface to that object, then the correct method, then call that method.

== But what is an object? ==

Rust is not an object oriented language. So what would be the proper representation for an
"Object" in Rust?

Suppose we want to represent two bananas and two apples at paths `/fruits/banana1`, `/fruits/banana2`,
`/fruits/apple1` and `/fruits/apple2`.
Let's also assume that our user has structs, `Banana` and `Apple`, which the user wants to
model as D-Bus objects.

`Tree`, the current solution, has an `ObjectPath` struct which represents an object, but it's also generic:
the `tree` takes a parameter for something to be stored inside the tree for every `ObjectPath`.
This works, kind of, because the user could make an `enum Fruit { Banana(Banana), Apple(Apple) }` and
but that into the tree.
But it's a bit of boilerplate to make that enum, and since the fruit structs are now stored within the
tree they are not accessible from outside the tree.

Let's consider another approach as well.

Perhaps we could instead store the struct itself, and demand that it has implements a trait `HasInterfaces`
that we can use to get interfaces from. We would then store that as either:

 * `Box<dyn HasInterfaces>` - extra allocation, and we take ownership of the structs,
    so they can't easily be accessed from outside the tree.
 * `Rc<dyn HasInterfaces>`, `Arc<dyn HasInterfaces>` - okay, shared ownership, but now we get mutability
    issues instead.
 * `&dyn HasInterfaces` - okay, no allocation, ownership is somewhere else but...whoa,
    now our tree is no longer static, we need to add `'a` everywhere and we easily we get borrow checker issues.

Every option comes with its own set of downsides, but I guess we could make the tree generic
over `Deref<dyn HasInterfaces>` - or should it be `DerefMut<dyn HasInterfaces>` maybe...or...

And we also have the option to refine the current solution. if `ObjectPath` is a struct, then
that struct could instead of being generic store a `Box<dyn Any>` that could be downcasted to a
`Banana` or `Apple`. Or maybe that should be `Rc<dyn Any>` or `Arc<dyn Any>` or `&dyn Any`...

== Next up: the interface ==

Okay, so we found our path, and a list of interfaces that it supports somehow. But what is a
proper representation of a D-Bus "interface" in Rust? It must be a trait, right? Because it
contains methods, properties and signals...wait, did you say properties? That should be stored
somewhere? Maybe we're better off with a struct anyway to represent the interface?

No, let's go with a trait. The properties can be "get" and "set" methods.
But we need a description of the interface,
every method needs to have information about what arguments it takes and what it returns,
so we can tell D-Bus about this.
Lesson learned: This information needs to be shared between paths. If you have many paths of the
same type, which is common, this information can take up substantial amount of memory if you
copy it between all paths.

Okay, so now there are two routes to go: closures or user defined traits. Closures means something like:

```
make_interface("Peelable").add_method("Peel", |...| {
  // code to peel the banana here
})
```

Whereas user defined traits means something like:

```
trait Peelable {
  fn peel(...);
}

impl Peelable for Banana {
  fn peel(...) {
    // code to peel the banana here
  }
}
```

The user defined traits indeed look more "Rustic", but they come with their own set of problems:

 * With the closure builder approach, the interface description is built automatically.
   We have no RTTI to build an interface description from just a trait.
 * With the closure builder approach, the type of the in/out arguments can be deduced from the closure.
   With the trait approach, we need to do this manually (see below).

The current approach is to use the closure builder, but also provide a code generation tool that
can create the boilerplate code required for the user defined trait approach.

As an option, we could use some kind of proc macro to derive boilerplate code, but
proc macros come with their own set of problems (such as longer compile times e g).

=== Type safety deduced from the closure ===

Due to some clever generics, it's possible to derive the arguments' types from
just the closure, if the argument is something like:

|..., (arg1: u32, arg2: Signature)| -> Result<bool>

Then, by making the closure maker generic, we can insert boilerplate code to read the
arguments from the message and transform them into the desired types before calling the closure.
Likewise we can append arguments to the return message in the same way.

=== What about signals? ===

They are supposed to be sent from the `ObjectPath` to the world to tell that something has changed.
I don't think we have a good representation of "signals" in Rust so let's skip these for the time being.
Some helper functions to emit one will probably do at this point.

=== The interface name is actually optional ===

In case an interface is not supplied with the method call, the D-Bus spec is intentionally
vague on what do to. You could check all interfaces, have a default interface, or something.
A minor complication, but still something to remember.)

== Next up: Calling the method ==

Okay, so now we're supposed to dispatch a method, but with what arguments?

=== Mutability ===

If we're calling a method on the struct itself, it would be very beneficial if that
method could take `&mut self` instead of just `&self`. After all, peeling a banana
changes that banana in some way, right? Not having to resort to interior mutability would be nice.

But getting there needs really careful code design not to lose the mutability at some step.
First exchanging the `&mut Tree` to a `&mut ObjectPath`, but then we need a reference to the
description to know what method we're going to call, without losing mutability to the actual data.

And if we used `Rc` or `Arc` in the "Object" above, then we lost mutability already...

But that is not all. If we call a closure, that closure could in itself have an environment.
Should that be changeable as well, i e, an `FnMut` closure, or is an `Fn` closure enough?
`FnMut` closures are nicer if possible but retaining mutability can be tricky here.

=== Send, Sync or Local? ===

Okay, so no we're supposed to call a closure, should we require `Send`? `Sync`? `'static`?

Somebody want to dispatch several methods in parallel from different threads, so that means
`Box<Fn() + Send + Sync + 'static>`. Someone else runs single threaded and does not want
the closure's environment to be restricted, there we could do with `Box<FnMut()>`.
Someone in the middle would perhaps prefer `Box<FnMut() + Send + 'static>`.

Oh, and whatever we store inside the tree, should also be the same combination of
`Send`, `Sync` and `'static`.

We could make things generic, but making things generic comes with the cost of complexity.

And moving things into a closure is in itself problematic: those things cannot
be retrieved again, and there is no `into_inner` for a boxed closure, which brings
whatever you moved into the closure back out.
And borrowing things into a closure is also problematic because now the tree is suddenly
not static which means that you easily run into borrow check issues.

== Changing the tree itself from a method ==

But wait, there's more. Actually, it's not that uncommon that a method changes the tree,
e g by adding or removing an interface to the same path, or even adding or removing other
paths! This means another issue w r t mutability. I e, peeling the banana might cause another
`/shells/banana1` object to show up.

== Async ==

I almost forgot. Async methods is the new big thing. It means that methods could
await things before they reply. This and mutability combined things even more complex.

It also means borrowing the `Connection` for the duration of the method running, or
perhaps holding an `Arc`/`Rc` to it. Because how would you else send the reply once
you're done?

=== Async + GetAll ===

Here's another problem. The user should not have to implement GetAll, it should be
implemented by calling Get for all properties. But the Context can not be easily
cloned (unless we wrap everything inside an Arc<Mutex>??).

Maybe the best trade-off is to let `Get` borrow the `Context` until the first await
point only. We then kick off all property gets in async-parallel fashion.

Then have a separate small `GetCtx` that can be used during the awaiting, which is actually an Arc internally,
shared between the different `GetAll`s.

However, retreival of properties can also happen as the result of wanting to send
ObjectManagerInterfacesAdded. In which case we don't have a message to start with.


== Mocking and client/server ==

TBD

= Solution decisions =

 * At this point let's make the tree `Send + 'static` (but not `Sync`). This seems
   to be the most common combination.
 * Let's make `Context` be these things:
   * `Message`
   * Clones of strings for path/interface/method
   * Storage for the reply, signals etc
   * `IfaceToken` (because why not)
   * For async: a `Arc<dyn Sender>` so the replies can be sent
   * For getprop / getpropall: a message to append arguments to
 * Let's make a `Path` be these things:
   * A name
   * A list of supported interfaces `HashSet<IfaceToken>`
   * An optional random object `Option<Box<Any + Send + 'static>>`
 * Let's make an `IfaceDesc` be the name, list of methods, properties and signals
 * Let's make the method be this:
   * `Fn(Context, &mut Tree) -> Option<Context> + Send + 'static`
   * Same for get_prop / set_prop
 * Use clever generics (or possibly macros) to:
   * look up the path
   * downcast the random object
   * type cast arguments
   * call method on the random object
   * check for errors (potentially after async)
   * type cast reply (potentially after async)          



Cost for calling a method:
 * Looking up the path twice (three for getprop/setprop) (btreemap)
 * Looking up the interface once (twice for getprop/setprop) (vec/linear)
 * Looking up the method once (hashmap)
 * One `Arc` ref up/down for regular methods, one more for async
 *  
