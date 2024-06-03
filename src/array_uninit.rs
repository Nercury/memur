use std::ptr::null_mut;
use crate::{Array, ArrayInitializer, WeakArena};

/// Continuous memory block containing uninitialized elements of the same type, and can be used to
/// initialize the `Array`.
pub struct UninitArray<T> where T: Sized {
    pub (crate) _arena: WeakArena,
    pub (crate) _capacity: usize,
    pub (crate) _metadata: *mut ArrayMetadata<T>,
}

impl<T> UninitArray<T> where T: Sized {
    /// Returns the number of initialized items in array if the `Arena` is alive.
    pub fn len(&self) -> usize {
        if self._arena.is_alive() {
            unsafe { (*self._metadata)._len }
        } else {
            0
        }
    }

    /// Returns the capacity, or maximum allowed items in array if the `Arena` is alive.
    pub fn capacity(&self) -> usize {
        self._capacity
    }

    /// A pointer to array contents to unsafely initialize the items to appropriate values.
    /// Call `initialized_to_len` to finalize initialization.
    /// Alternatively, use `start_initializer` for safe initialization.
    pub unsafe fn data_mut(&mut self) -> *mut T {
        (*self._metadata)._data
    }

    /// This function assumes the `len` items in `UninitArray` are properly initialized
    /// and returns `Array` that points to the same memory. Any uninitialized items are not
    /// re-claimed.
    pub unsafe fn initialized_to_len(self, len: usize) -> Array<T> {
        if len > self._capacity {
            panic!("set_len exceeds capacity");
        }
        (*self._metadata)._len = len;
        Array {
            _arena: self._arena,
            _metadata: self._metadata,
        }
    }

    /// Returns the helper to safely initialize the array.
    pub fn start_initializer(self) -> ArrayInitializer<T> {
        ArrayInitializer {
            uninit_array: self,
            initialized_len: 0,
        }
    }
}

pub (crate) struct ArrayMetadata<T> {
    pub _len: usize,
    pub _data: *mut T,
}

pub (crate) fn drop_array<T>(data: *const u8) {
    let metadata: &mut ArrayMetadata<T> = unsafe { std::mem::transmute::<*const u8, &mut ArrayMetadata<T>>(data) };
    if metadata._data == null_mut() {
        return;
    }

    let len = metadata._len;
    metadata._len = 0;
    for item_ptr in unsafe { Array::<T>::iter_impl(metadata._data as *const u8, len) } {
        let item_ref: &T = unsafe { std::mem::transmute::<*const T, &T>(item_ptr) };
        let item: T = unsafe { std::mem::transmute_copy::<T, T>(item_ref) };
        std::mem::drop(item);
    }

    metadata._data = null_mut();
}