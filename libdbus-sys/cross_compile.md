Cross compiling dbus
====================

Disclaimer: I'm writing this because people ask me how to do it, not because I'm a subject matter expert. I hope that someone more knowledgable will submit a PR which fills in some of the blanks here as I haven't fully succeeded myself yet. 

(The examples below all assume you're trying to compile for Raspberry Pi 2 or 3 running Raspbian. Adjust target triples accordingly if your target is something else.) 

A cross linker
--------------

Apparently, `rustc` in itself can generate code for many archs but not assemble the generated code into the final executable. Hence you need a cross linker.

**Install it** - here follow whatever guide you have for the target arch. Distributions may also ship with cross toolchains. Example for Ubuntu 18.04:

`sudo apt install gcc-8-multilib-arm-linux-gnueabihf`

**Tell rustc where to find it** - in [.cargo/config](https://doc.rust-lang.org/cargo/reference/config.html) add the following:

```
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc-8"
```

Target rust std
---------------

This one's easy, just run rustup:

`rustup target add armv7-unknown-linux-gnueabihf`


Target dbus libraries
---------------------

**Installing the library**

Now to the more challenging part. Since we link to a C library `libdbus-1.so`, we also need the target version of that library. However, `libdbus-1.so` in itself links to a systemd library (at least it does so here) which in turn links to other libraries etc, so we need target versions of those libraries too.

Getting an entire rootfs/image is probably the easiest option. The rootfs needs to have `libdbus-1-dev` installed.

[//]: # (Ugly hack: sudo mount -o loop,offset=50331648 2019-04-08-raspbian-stretch-lite.img /tmp/mnt ) 
[//]: # (cd /tmp/mnt/usr/lib/arm-linux-gnueabihf && ln -s ../../../lib/arm-linux-gnueabihf/libdbus-1.so.3 libdbus-1.so ) 

**Finding the library**

When not cross compiling, finding the right library is done by a `build.rs` script which calls `pkg-config`. This will not work when cross compiling because it will point to the `libdbus-1.so` on the host, not the `libdbus-1.so` of the target.
Maybe it is possible to teach `pkg-config` how to return the target library instead, but I have not tried this approach. Instead we can override build script altogether and provide the same info manually. This is possible because `libdbus-sys` has a `links = dbus` line.

For the example below we assume that we have mounted a Raspbian rootfs on `/tmp/mnt`.

```
[target.armv7-unknown-linux-gnueabihf.dbus]
rustc-link-search = ["/tmp/mnt/usr/lib/arm-linux-gnueabihf"]
rustc-link-lib = ["dbus-1"]
```


Finally
-------

If we are all set up, you should be able to successfully compile with:

`cargo build --target=armv7-unknown-linux-gnueabihf`

...but when I tried this, I got the following error:

```
cannot find /lib/arm-linux-gnueabihf/libpthread.so.0
cannot find /usr/lib/arm-linux-gnueabihf/libpthread_nonshared.a
```

...which I, so far, have not been able to resolve. Let me know if you have any ideas!

