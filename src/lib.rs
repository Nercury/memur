//! A Grow-Only Arena Library for Bump Allocation with Proper Drop Handling
//!
//! This library is primarily intended for use cases where bump allocation is desired. Additionally,
//! memur ensures that every item added to the arena has its
//! drop function executed. Because running drop for a large number of items cause many cache misses,
//! memur stores its droplists inline within the arena, close to the data to be dropped.
//!
//! ## Overview
//!
//! The library provides several types to manage memory using bump allocation:
//!
//! - **Memory & Arena** – `Memory` serves as a shared memory pool (which can be cloned and used
//!   across threads), while `Arena` draws blocks from it. Note that an `Arena` does not reclaim
//!   memory on its own; to clean up, a new `Arena` is typically created.
//!
//! - **UStr** – A universal string type that holds a zero-terminated UTF8 string. `UStr` does not
//!   add a drop function to the arena, making it efficient for applications that use many strings.
//!
//! - **FixedArray** – A fixed-length array with several initialization strategies:
//!   - Initialization from a fixed-size iterator.
//!   - A safe initializer that allows incremental insertion.
//!   - An unsafe “C-style” initialization.
//!   All elements in a `FixedArray` are dropped when the arena is dropped.
//!
//! - **Growable Array (Array)** – A dynamic, Vec-like array implementation.
//!
//!   **Pros:**
//!     - Provides push and pop operations with full drop safety.
//!     - Familiar API for users of Vec.
//!
//!   **Cons:**
//!     - Items are allocated individually, so they are not stored contiguously.
//!     - Pointer indirection incurs a slight overhead compared to a contiguous FixedArray or List.
//!
//! - **List** – A simple, growable list where items are stored non-contiguously.
//!   It keeps related metadata close to the data, but it does not support indexing or cloning.
//!
//! Additionally, helper `MemurIterator` trait is provided to collect iterators into a `List` `FixedArray`, or `Array`
//! (since the standard library’s `collect` does not work directly with arena types).
//!
//! ## Memory, Arena, and Drop Behavior
//!
//! The core types `Memory` and `Arena` manage the allocation and deallocation of objects.
//! When an object is placed into the arena, a pointer to its drop function is stored in an
//! inlined droplist. This ensures that when the `Arena` is dropped, all items are properly cleaned up.
//!
//! Example:
//!
//! ```rust
//! use memur::{Memory, Arena};
//!
//! let mem = Memory::new();
//! {
//!     let arena = Arena::new(&mem).unwrap();
//!     // Allocate objects within the arena.
//! }
//! // When the arena goes out of scope, all allocated objects are dropped.
//! ```
//!
//! ## UStr
//!
//! `UStr` holds a zero-terminated UTF8 string and can be interpreted as both a Rust string and a C string.
//! It avoids the overhead of a drop function, making it suitable for applications with many strings.
//!
//! ```rust
//! use memur::{Memory, Arena, UStr};
//! use std::ffi::CStr;
//!
//! let mem = Memory::new();
//! {
//!     let arena = Arena::new(&mem).unwrap();
//!     let text = UStr::from_str(&arena, "Hello").unwrap();
//!
//!     assert_eq!("Hello", &text);
//!     assert_eq!(unsafe { CStr::from_bytes_with_nul(b"Hello\0") }
//!                    .unwrap()
//!                    .to_str()
//!                    .unwrap(), &text);
//! }
//! // The memory is reclaimed when the arena is dropped.
//! ```
//!
//! ## FixedArray
//!
//! `FixedArray` provides a fixed-length array with multiple initialization options.
//! All elements are dropped when the arena is dropped. Access functions return `Option`
//! to indicate that data may no longer be available after cleanup.
//!
//! ```rust
//! use memur::{Memory, Arena, FixedArray};
//!
//! let mem = Memory::new();
//! let array = {
//!     let arena = Arena::new(&mem).unwrap();
//!     let array = FixedArray::new(&arena, (0..2)).unwrap();
//!     assert_eq!(array.len(), Some(2));
//!     array
//! };
//!
//! // After the arena is dropped, accessing the array returns None.
//! assert_eq!(array.len(), None);
//! ```
//!
//! ## Growable Array (Array)
//!
//! The new `Array` type offers a dynamic, Vec-like interface for arena allocation.
//! Items are allocated individually and stored via a pointer table that is resized as needed.
//!
//! **Pros:**
//! - Dynamic sizing with push/pop operations.
//! - Familiar API for those accustomed to Vec.
//! - Full drop safety for each element.
//!
//! **Cons:**
//! - Items are not stored contiguously, which may limit certain operations (e.g., slicing).
//! - Additional overhead due to pointer indirection.
//!
//! Example:
//!
//! ```rust
//! use memur::{Memory, Arena, Array};
//!
//! let mem = Memory::new();
//! let mut arena = Arena::new(&mem).unwrap();
//! let mut array = Array::new(&arena).unwrap();
//! array.push(42).unwrap();
//! array.push(7).unwrap();
//! assert_eq!(array.len().unwrap(), 2);
//! assert_eq!(array.pop(), Some(7));
//! ```
//!
//! As mentioned before, no actual memory is reclaimed until the whole Arena is dropped.
//!
//! ## List
//!
//! `List` is a simple growable collection in which items are not stored contiguously.
//! It is efficient and keeps related metadata near the data but does not support indexing
//! or cloning.
//!
//! ```rust
//! use memur::{Memory, Arena, List};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//! let mut list = List::new(&arena).unwrap();
//! list.push(1).unwrap();
//! list.push(2).unwrap();
//! assert_eq!(list.len(), 2);
//! ```
//!
//! ## Collection Helpers
//!
//! Because creating a new `List`, `Array` or `FixedArray` requires specifying the `Arena`, standard
//! collection methods cannot be used directly. memur provides MemurIterator trait to collect
//! iterators into these types.
//!
//! ```rust
//! use memur::{Memory, Arena, MemurIterator};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//! let array = (0..10).collect_fixed_array(&arena).unwrap();
//! assert_eq!(array.len().unwrap(), 10);
//! ```
//!
//! ## Summary
//!
//! memur is designed for scenarios where bump allocation is desired and proper drop
//! execution is required. Its inlined droplists reduce cache misses during cleanup, making
//! drop operations more efficient. Users should evaluate the following trade-offs:
//!
//! - **FixedArray and List:** Offer efficient, drop-safe allocation with minimal overhead,
//!   but are limited to fixed or simple growable collection semantics.
//!
//! - **Array (Growable Array):** Provides a dynamic, Vec-like interface with push/pop support
//!   and full drop safety, at the cost of non-contiguous storage and additional pointer indirection.
//!
//! Choose the type that best suits your application's needs.

mod logging;
mod droplist;
mod dontdothis;
mod block;
mod arena;
mod memory;
mod list;
mod array;
mod array_fixed;
mod array_uninit;
mod ustr;
mod n;
mod traits;
mod iter;

pub use memory::{Memory, MemoryBuilder};
pub use list::List;
pub use array::{Array, ArrayIter, ArrayIterMut};
pub use array_fixed::{FixedArray, ArrayInitializer};
pub use array_uninit::{UninitArray};
pub use ustr::{UStr, UStrError};
pub use arena::{WeakArena, Arena, UploadError};
pub use n::N;
pub use traits::{MemurIterator, ToArenaArray, ToArenaFixedArray, ToArenaList};
pub use droplist::{DropFn, DropItem};

#[cfg(test)]
pub mod dropflag;