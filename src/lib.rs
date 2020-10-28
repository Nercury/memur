//! Glow-only Arena implementation for structures of any type that also ensures fast end efficient
//! drop order. It also has some common types that makes efficient use of `Arena` properties.
//!
//! ## What is Arena?
//!
//! There are several use cases when arena allocation pattern is desired.
//!
//! One of them is when we do not want to track the value ownership and lifetimes. Instead, we
//! have a known point when all the data inside the arena should be deallocated.
//! As an example, consider a game level. It may contain many objects, but we know we will
//! deallocate them all at the same time when the level is no longer in use, and kind of don't
//! care anymore about the objects contain.
//!
//! Another use case is when we want to ensure that objects are nearby in the memory.
//! This kind of arena copies the value contents into a memory block and then only allows us to
//! access the value over a pointer.
//!
//! `memur` cares about both of these use-cases. It allows us to place any type of object into the
//! `Arena`, and ensures their `Drop` function is executed. It is also possible to explicitly
//! place a struct into the `Arena` that has no drop function. One of such built-in structures is
//! `UStr` type that holds a string.
//!
//! ## `memur` is grow-only Arena
//!
//! While `memur` will take care of dropping the values once there are no remaining `Arena`
//! references, re-claiming the memory is a no-goal of this library. Instead, the idea is to
//! create another `Arena`, and place a fresh set of values there.
//!
//! Also, the underlying `Memory` container that issues memory blocks to `Arena` never
//! automatically deallocates memory. Instead, the user of this library should know best when
//! it is the time for a cleanup, and call the `cleanup` function.
//!
//! ## Some `memur` features
//!
//! ### `Memory` can be cloned between threads, `Arena` and collection objects can not
//!
//! The `Memory` is "issuer of memory blocks", or a Pool. It can be cloned and it will still
//! reference the same internal implementation. It can be shared between threads as needed.
//!
//! The `Arena` is a "user of memory blocks". It draws new memory blocks as required from the
//! `Memory` pool.
//!
//! Its sibling the `WeakArena` is used to avoid reference cycles and can be stored inside
//! the structures to get a quick access to `Arena`. However, this will return `None` when
//! the `Arena` goes out of scope.
//!
//! `Arena` and `WeakArena` can also be cloned, but can not be passed to another thread.
//!
//! ### Efficient droplists
//!
//! When a value is placed into the `Arena` memory block, a pointer is also added to a function
//! that will drop this value once the `Arena` is no longer in use. This function is placed
//! into an empty droplist slot. The `Arena` keeps track of the first and last droplists.
//! Last droplist is used to push another function as mentioned, and the first droplist is
//! used to execute drop for all arena objects. The droplists themselves are daisy-chained together
//! a linked list and end up interleaved in the memory between the objects to be dropped, making
//! their execution efficient.
//!
//! ### No-drop universal string type `UStr`
//!
//! UStr holds an UTF8 string that is zero-terminated. Instead of converting between `String` and
//! `CString` types, `UStr` can be safely interpreted as both. In addition to that, `UStr` does
//! not add a drop function to arena, perfect for applications with tons of strings of different
//! lengths. The downside of `UStr` is that it contains the `WeakArena` reference inside to ensure
//! safety.
//!
//! ```
//! use memur::{Memory, Arena, UStr};
//! use std::ffi::CStr;
//!
//! let mem = Memory::new();
//!
//! {
//!     let text = {
//!         let arena = Arena::new(&mem).unwrap();
//!
//!         let text = UStr::from_str(&arena, "Hello").unwrap();
//!
//!         assert_eq!("Hello", &text);
//!         assert_eq!(unsafe { CStr::from_bytes_with_nul_unchecked(b"Hello\n") }, &text);
//!
//!         // The arena is dropped here, but since the UStr holds WeakArena,
//!         // it can still be used.
//!
//!         text
//!     };
//!
//!     assert_eq!("Hello", &text);
//!     assert_eq!(unsafe { CStr::from_bytes_with_nul_unchecked(b"Hello\n") }, &text);
//!
//!     // The memory is reclaimed here since the last instance of `WeakArena` is gone
//! }
//! ```
//!
//! ### Control of the drop order with `N<T>`
//!
//! There is a seemingly useless type that allows uploading a struct to arena. But in addition to
//! that, it can also be used to ensure that a struct will be dropped after a previously added
//! struct. Consider this example:
//!
//! ```
//! use memur::{Memory, Arena, N};
//!
//! let mem = Memory::new();
//! let order = std::cell::RefCell::new(Vec::new()); // pardon my use of RefCell
//!
//! {
//!     let arena = Arena::new(&mem).unwrap();
//!
//!     let a = N::new(&arena, Wrapper::new(|| order.borrow_mut().push("dropped a"))).unwrap();
//!     let b = N::new(&arena, Wrapper::new(|| order.borrow_mut().push("dropped b"))).unwrap();
//! }
//!
//! assert_eq!("dropped a", order.borrow()[0]);
//! assert_eq!("dropped b", order.borrow()[1]);
//!
//! // Testing this drop functionality requires creating some example structure that executes
//! // our closure when it is dropped:
//!
//! struct Wrapper<F: FnMut()> {
//!     execute_on_drop: F,
//! }
//!
//! impl<F: FnMut()> Wrapper<F> {
//!     pub fn new(execute_on_drop: F) -> Wrapper<F> {
//!         Wrapper { execute_on_drop }
//!     }
//! }
//!
//! impl<F: FnMut()> Drop for Wrapper<F> {
//!     fn drop(&mut self) {
//!         (self.execute_on_drop)();
//!     }
//! }
//! ```
//!
//! So, the above succeeds because droplists drop items sequentialy.
//! If we wanted to ensure that `a` is dropped after `b`, we can do this instead:
//!
//! ```
//! use memur::{Memory, Arena, N};
//!
//! let mem = Memory::new();
//! let order = std::cell::RefCell::new(Vec::new()); // pardon my use of RefCell
//!
//! {
//!     let arena = Arena::new(&mem).unwrap();
//!
//!     let a = N::new(&arena, Wrapper::new(|| order.borrow_mut().push("dropped a"))).unwrap();
//!     let b = a.outlives(Wrapper::new(|| order.borrow_mut().push("dropped b")));
//! }
//!
//! assert_eq!("dropped b", order.borrow()[0]);
//! assert_eq!("dropped a", order.borrow()[1]);
//! #
//! # struct Wrapper<F: FnMut()> {
//! #     execute_on_drop: F,
//! # }
//! #
//! # impl<F: FnMut()> Wrapper<F> {
//! #     pub fn new(execute_on_drop: F) -> Wrapper<F> {
//! #         Wrapper { execute_on_drop }
//! #     }
//! # }
//! #
//! # impl<F: FnMut()> Drop for Wrapper<F> {
//! #     fn drop(&mut self) {
//! #         (self.execute_on_drop)();
//! #     }
//! # }
//! ```
//!
//! You can imagine this being useful when wrapping low level graphics APIs. Also everything that is
//! needed to perform this is contained in the same memory block with no additional alocations.
//!
//! ### Array
//!
//! A fixed-length array. It can't be cloned (and point to the same memory).
//! There are three ways to initialize this array. One of them is unsafe. All are efficient.
//! First, it can be initialized from a fixed-size iterator:
//!
//! ```
//! use memur::{Memory, Arena, Array};
//!
//! let mem = Memory::new();
//!
//! let a = {
//!     let arena = Arena::new(&mem).unwrap();
//!
//!     // this `into_iter` returns fixed size iterator
//!     // but you probably would not use vecs together with memur
//!     let a = Array::new(&arena, vec![1, 2].into_iter()).unwrap();
//!
//!     assert_eq!(a.len(), Some(2));
//!
//!     a
//! };
//!
//! assert_eq!(a.len(), None); // when arena goes out of scope, the len can not be retrieved
//! ```
//!
//! The `Array` properly drops items when the arena goes out of scope. This means that unlike `UStr`,
//! all attempts to access the `Array` are checked (because the struct drop functions might
//! have executed). That's why the `len` and many other functions wrap results in the `Option`.
//!
//! Another safe way to initalize the array is to use the initializer:
//!
//! ```
//! use memur::{Memory, Arena, Array};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//!
//! let uninitialized_array = Array::with_capacity(&arena, 2).unwrap();
//! let mut initializer = uninitialized_array.start_initializer();
//!
//! initializer.push(1);
//! initializer.push(2);
//!
//! let a = initializer.initialized().unwrap(); // number of pushes must be lower or equal capacity
//!
//! assert_eq!(a.len(), Some(2));
//! ```
//!
//! The unsafe, or "C-way" is useful to allow some other code to fill the array contents:
//!
//! ```
//! use memur::{Memory, Arena, Array};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//!
//! let mut uninitialized_array = Array::<i32>::with_capacity(&arena, 2).unwrap();
//!
//! unsafe { *(uninitialized_array.data_mut().offset(0)) = 1 }
//! unsafe { *(uninitialized_array.data_mut().offset(1)) = 2 }
//!
//! let a = unsafe { uninitialized_array.initialized_to_len(2) };
//!
//! assert_eq!(a.len(), Some(2));
//! assert_eq!(a[0], 1);
//! assert_eq!(a[1], 2);
//! ```
//!
//! `Array` guarantees that all items are in a continuous memory location.
//!
//! ### List
//!
//! List can be grown, but the items are not in a continuous memory location. Instead,
//! item data pointers are stored in the fixed size metadata blocks, interleaved with
//! the items themselves:
//!
//! ```text ignore
//! meta1[*item1 .. *itemN *meta2] item1 .. itemN meta2[*itemN+1 .. emptyslotM *null]
//! ```
//!
//! There is metadata that contains a pointer to actual item data that may not follow the metadata:
//! it all depends when a new item was pushed to the list. But generally, this should have a property
//! of keeping the item data close to the each metadata block. If the item itself is a list or
//! array, you can imagine all the data ending up nearby.
//!
//! List usage is simpler than implementation:
//!
//! ```
//! use memur::{Memory, Arena, List};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//!
//! let mut list = List::new(&arena).unwrap();
//!
//! list.push(1).unwrap();
//! list.push(2).unwrap();
//!
//! assert_eq!(list.len(), 2); // list len is not stored in arena, unlike vec len
//! assert_eq!(list.iter().skip(0).next(), Some(&1));
//! assert_eq!(list.iter().skip(1).next(), Some(&2));
//! ```
//!
//! There are few downsides of `List`: it can't be indexed into, and it can't be cloned.
//!
//! ### `collect` helpers
//!
//! The `std` lib `collect` can not work with this library, because creating a new `List` or `Array`
//! requires knowledge of which `Arena` to use.
//!
//! That's why there is a helper trait for that, which so far has the simple `collect_list` and
//! `collect_array`, nothing fancy.
//!
//! ```
//! use memur::{Memory, Arena, List, MemurIterator};
//!
//! let mem = Memory::new();
//! let arena = Arena::new(&mem).unwrap();
//!
//! let mut list = List::new(&arena).unwrap();
//! list.push(1).unwrap();
//! list.push(2).unwrap();
//!
//! let a = list.iter().cloned().collect_array(&arena).unwrap();
//! assert_eq!(a.len(), Some(2));
//! assert_eq!(a[0], 1);
//! assert_eq!(a[1], 2);
//!
//! let list2 = a.iter().cloned().collect_list(&arena).unwrap();
//! assert_eq!(list2.len(), 2);
//! ```
//!
//! ### Custom structures
//!
//! It should be possible to implement custom structures for `memur`, all unsafe machinery
//! should be accessible.

mod droplist;
mod dontdothis;
mod block;
mod arena;
mod memory;
mod list;
mod array;
mod ustr;
mod n;
mod traits;
mod iter;

pub use memory::{Memory, MemoryBuilder};
pub use list::List;
pub use array::{Array, UninitArray, ArrayInitializer};
pub use ustr::{UStr, UStrError};
pub use arena::{WeakArena, Arena, UploadError};
pub use n::N;
pub use traits::MemurIterator;
pub use droplist::{DropFn, DropItem};

#[cfg(test)]
pub mod dropflag;