## Arena storage with bells and whistles

[![Version](https://img.shields.io/crates/v/memur.svg)](https://crates.io/crates/memur)
[![Build Status](https://travis-ci.org/Nercury/memur.svg?branch=master)](https://travis-ci.org/Nercury/memur)

Arena storage with its own basic allocator, managed drop order, efficient and optional
drop execution, universal string, list, array, and custom structure/data support. 

See more comprehensive writeup in crate documentation.

Some idea of how the usage looks like:

```rust
use memur::{Memory, Arena, UStr};

fn main() {
    // memory pool which can be shared between threads
    let mem = Memory::new(); 

    {
        let arena = Arena::new(&mem).unwrap();
    
        // Zero-terminated utf-8 string that does not need 
        // to be dropped:
        let text = UStr::from_str(&arena, "Hello").unwrap();

        assert_eq!("Hello", &text);

        // Since it is zero-terminated, 
        // it can be used in C APIs without conversion:
        assert_eq!(unsafe { CStr::from_bytes_with_nul_unchecked(b"Hello\n") }, &text);

        // Simple struct wrapper, items added this way end up
        // near each other in memory:
        let a = N::new(&arena, "hello").unwrap();
        let b = N::new(&arena, "world").unwrap();
        
        // This also adds another item "c" to the same arena, which
        // will be dropped after the "a" is dropped.
        // This is useful for ensuring the drop order.
        let c = a.outlives("c").unwrap();
        
        // Fixed size array example, initialized 
        // from exact size iterator:
        let a = Array::new(&arena, (0..2).into_iter()).unwrap();
        assert_eq!(a.len(), Some(2));
        
        // The unsafe C-way to initialize array is also available:
        let mut uninitialized_array = Array::<i32>::with_capacity(&arena, 2).unwrap();
        
        unsafe { *(uninitialized_array.data_mut().offset(0)) = 1 }
        unsafe { *(uninitialized_array.data_mut().offset(1)) = 2 }
        
        let a = unsafe { uninitialized_array.initialized_to_len(2) };
        assert_eq!(a.len(), Some(2));
        assert_eq!(a[0], 1);
        assert_eq!(a[1], 2);               

        // The drop functions are executed when the Arena goes out os scope,
        // and the created objects are aware of that because they keep
        // WeakArena inside.
        
        // The memory blocks are returned back to Memory when
        // all WeakArena holders go out of scope.
    }
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
