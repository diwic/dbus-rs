//! Types and traits for easily getting a message's arguments, or appening a message with arguments.
//!
//! Using this module should be faster than
//! using MessageItem, especially when large arrays need to be appended.
//! It also encodes more of D-Bus restrictions into Rust's type system, so
//! trying to append anything that D-Bus would not allow should result in a
//! compile-time error.
//!
//! A message has `get1`, `get2` etc, and `append1`, `append2` etc, which is your
//! starting point into this module's types. 
//!
//! **Append a**:
//!
//! `bool, u8, u16, u32, u64, i16, i32, i64, f64` - the corresponding D-Bus basic type
//!
//! `&str` - a D-Bus string. D-Bus strings do not allow null characters, so 
//! if the string contains null characters, it will be cropped
//! to only include the data before the null character. (Tip: This allows for skipping an
//! allocation by writing a string literal which ends with a null character.)
//!
//! `&[T] where T: Append` - a D-Bus array. Note: can use an efficient fast-path in case of 
//! T being an FixedArray type.
//!
//! `Array<T, I> where T: Append, I: Iterator<Item=T>` - a D-Bus array, maximum flexibility.
//!
//! `Variant<T> where T: Append` - a D-Bus variant.
//!
//! `(T1, T2) where T1: Append, T2: Append` - tuples are D-Bus structs. Implemented up to 12.
//!
//! `Dict<K, V, I> where K: Append + DictKey, V: Append, I: Iterator<Item=(&K, &V)>` - A D-Bus dict (array of dict entries).
//!
//! `ObjectPath` - a D-Bus object path.
//!
//! `Signature` - a D-Bus signature.
//!
//! `OwnedFd` - shares the file descriptor with the remote side.
//!
//! **Get a**:
//!
//! `bool, u8, u16, u32, u64, i16, i32, i64, f64` - the corresponding D-Bus basic type
//!
//! `&str`, `&CStr` - a D-Bus string. D-Bus strings are always UTF-8 and do not contain null characters.
//!
//! `&[T] where T: FixedArray` - a D-Bus array of integers or f64.
//!
//! `Array<T, Iter> where T: Get` - a D-Bus array, maximum flexibility. Implements Iterator so you can easily
//! collect it into, e g, a `Vec`.
//!
//! `Variant<T> where T: Get` - a D-Bus variant. Use this type of Variant if you know the inner type.
//!
//! `Variant<Iter>` - a D-Bus variant. This type of Variant allows you to examine the inner type.
//!
//! `(T1, T2) where T1: Get, T2: Get` - tuples are D-Bus structs. Implemented up to 12.
//!
//! `Dict<K, V, Iter> where K: Get + DictKey, V: Get` - A D-Bus dict (array of dict entries). Implements Iterator so you can easily
//! collect it into, e g, a `HashMap`.
//!
//! `ObjectPath` - a D-Bus object path.
//!
//! `Signature` - a D-Bus signature.
//!
//! `OwnedFd` - a file descriptor sent from the remote side.
//!

mod msgarg;

pub use self::msgarg::{Arg, FixedArray, Get, DictKey, Append};
pub use self::msgarg::{Iter, TypeMismatchError, IterAppend, Array, Variant, Dict};

