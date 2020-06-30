use crate::{Arena, UploadError, WeakArena};
use crate::dontdothis::next_item_aligned_start;

pub struct Array<T> where T: Sized {
    _arena: WeakArena,
    _len: usize,
    _ptr: *mut T,
}

impl<T> Array<T> where T: Sized {
    const fn aligned_item_size() -> usize {
        next_item_aligned_start::<T>(std::mem::size_of::<T>())
    }

    pub fn new(arena: &Arena, iter: impl ExactSizeIterator<Item=T>) -> Result<Array<T>, UploadError> {
        unsafe {
            let len = iter.len();
            let ptr = arena.alloc_no_drop_items_aligned_uninit::<T>(len, Self::aligned_item_size())? as *mut u8;
            for (index, item) in iter.enumerate() {
                let item_ptr = std::mem::transmute::<&T, *const u8>(&item);
                let arena_item_start_ptr = ptr.offset((index * Self::aligned_item_size()) as isize);
                let item_as_bytes = std::slice::from_raw_parts(item_ptr, std::mem::size_of::<T>());
                let arena_location_bytes = std::slice::from_raw_parts_mut(arena_item_start_ptr, std::mem::size_of::<T>());
                for (inb, outb) in item_as_bytes.iter().zip(arena_location_bytes.iter_mut()) {
                    *outb = *inb;
                }
                std::mem::forget(item);
            }

            Ok(Array {
                _arena: arena.to_weak_arena(),
                _len: len,
                _ptr: ptr as *mut T,
            })
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        let byte_ptr = self._ptr as *const u8;
        unsafe {
            (0..self._len)
                .map(move |i| {
                    let offset = Self::aligned_item_size() * i;
                    std::mem::transmute::<*const T, &T>(byte_ptr.offset(offset as isize) as *const T)
                })
        }
    }

    pub fn iter_mut(&self) -> impl Iterator<Item=&mut T> {
        let byte_ptr = self._ptr as *mut u8;
        unsafe {
            (0..self._len)
                .map(move |i| {
                    let offset = Self::aligned_item_size() * i;
                    std::mem::transmute::<*mut T, &mut T>(byte_ptr.offset(offset as isize) as *mut T)
                })
        }
    }
}

impl<T> Drop for Array<T> where T: Sized {
    fn drop(&mut self) {
        for item in self.iter() {
            let _oh_look_it_teleported_here: T = unsafe { std::mem::transmute_copy::<T, T>(item) };
            // and it's dropped
        }
    }
}

#[cfg(test)]
mod array {
    use crate::{Memory, Arena, Array};

    #[test]
    fn has_items_when_iterating() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i64)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i64)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i8() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i8)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i8)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i16() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i16)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }
}