## Bump allocated arena with fast drop support

[![Version](https://img.shields.io/crates/v/memur.svg)](https://crates.io/crates/memur)
[![Build Status](https://travis-ci.org/Nercury/memur.svg?branch=master)](https://travis-ci.org/Nercury/memur)

Arena storage with its own basic allocator, managed drop order, efficient and optional drop execution, universal string, list, array, and custom structure/data support.

See more comprehensive writeup in [crate documentation](https://docs.rs/memur).

Below is a single example demonstrating the usage of several key types provided by memur:

- **UStr** – a universal, zero-terminated UTF-8 string.
- **FixedArray** – a fixed-size array initialized from an exact-size iterator.
- **Array** – a dynamic, Vec-like growable array.
- **List** – a simple, growable collection with non-contiguous storage.

```rust
use memur::{Memory, Arena, UStr, FixedArray, Array, List, MemurIterator};
use std::ffi::CStr;

fn main() {
    // Create a shared memory pool that can be used across threads.
    let mem = Memory::new();

    {
        // Create an arena from the memory pool.
        let arena = Arena::new(&mem).unwrap();

        // --- UStr Example ---
        // UStr represents a zero-terminated UTF-8 string that does not require a drop function.
        // Pros:
        //   - Efficient for handling many strings.
        //   - Can be used directly in C APIs (no conversion needed).
        // Cons:
        //   - Contains a WeakArena reference, so it may not suit cases where a strong drop guarantee is needed.
        let text = UStr::from_str(&arena, "Hello").unwrap();
        assert_eq!("Hello", &text);
        assert_eq!(unsafe { CStr::from_bytes_with_nul_unchecked(b"Hello\0") }, &text);

        // --- FixedArray Example ---
        // FixedArray is a fixed-length array initialized from an iterator.
        // Pros:
        //   - Simple initialization when the number of elements is known exactly.
        //   - Ensures that each element is properly dropped when the arena is cleaned up.
        // Cons:
        //   - Size is fixed at initialization; resizing is not supported.
        let fixed_array = FixedArray::new(&arena, (0..2)).unwrap();
        assert_eq!(fixed_array.len(), Some(2));

        // --- Growable Array (Array) Example ---
        // Array provides a dynamic, Vec-like interface for bump allocation.
        // Pros:
        //   - Supports push/pop operations for dynamic sizing.
        //   - Familiar API for users accustomed to Vec.
        // Cons:
        //   - Items are allocated individually and stored via a pointer table, so they are not contiguous.
        //   - Pointer indirection introduces a slight overhead.
        let mut array = Array::new(&arena).unwrap();
        array.push(42).unwrap();
        array.push(7).unwrap();
        assert_eq!(array.len().unwrap(), 2);
        assert_eq!(array.pop(), Some(7));

        // --- List Example ---
        // List is a growable collection where items are stored non-contiguously.
        // Pros:
        //   - Efficient insertion with metadata interleaved with the item data.
        //   - Suitable for cases where iteration is the primary access method.
        // Cons:
        //   - Does not support indexing or cloning.
        let mut list = List::new(&arena).unwrap();
        list.push(1).unwrap();
        list.push(2).unwrap();
        assert_eq!(list.len(), 2);

        // When the arena goes out of scope, all objects allocated within it
        // are properly dropped. Objects maintain a WeakArena reference to ensure
        // they are not accessed after the arena is cleaned up.
    }
    
    // At this point, when all WeakArena holders are gone, the memory blocks are returned back to Memory.
}
```

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
