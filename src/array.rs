use crate::{Arena, UploadError, WeakArena};
use crate::dontdothis::next_item_aligned_start;
use std::ptr::{null_mut};
use crate::iter::EmptyIfDeadIter;

/// Continuous memory block containing many elements of the same type.
pub struct Array<T> where T: Sized {
    _arena: WeakArena,
    _metadata: *mut ArrayMetadata<T>,
}

struct ArrayMetadata<T> {
    _len: usize,
    _offset: usize,
    _data: *mut T,
}

fn drop_array<T>(data: *const u8) {
    let metadata: &mut ArrayMetadata<T> = unsafe { std::mem::transmute::<*const u8, &mut ArrayMetadata<T>>(data) };
    if metadata._data == null_mut() {
        return;
    }

    for item_ptr in unsafe { Array::<T>::iter_impl(metadata._data as *const u8, metadata._len, metadata._offset) } {
        let item_ref: &T = unsafe { std::mem::transmute::<*const T, &T>(item_ptr) };
        let _oh_look_it_teleported_here: T = unsafe { std::mem::transmute_copy::<T, T>(item_ref) };
    }

    metadata._data = null_mut();
}

impl<T> Array<T> where T: Sized {
    const fn aligned_item_size() -> usize {
        next_item_aligned_start::<T>(std::mem::size_of::<T>())
    }

    /// Returns the length of this array if the `Arena` is alive.
    pub fn len(&self) -> Option<usize> {
        if self._arena.is_alive() {
            Some(unsafe { (*self._metadata)._len })
        } else {
            None
        }
    }

    /// Creates a new array and places the data to it.
    pub fn new(arena: &Arena, iter: impl ExactSizeIterator<Item=T>) -> Result<Array<T>, UploadError> {
        unsafe {
            let len = iter.len();
            let metadata = arena.upload_no_drop::<ArrayMetadata<T>>(ArrayMetadata::<T> {
                _len: len,
                _offset: Self::aligned_item_size(),
                _data: null_mut(),
            })?;

            arena.push_custom_drop_fn(drop_array::<T>, metadata as *const u8)?;

            let ptr = arena.alloc_no_drop_items_aligned_uninit::<T>(len, Self::aligned_item_size())? as *mut u8;
            (*metadata)._data = ptr as *mut T;

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
                _metadata: metadata,
            })
        }
    }

    unsafe fn iter_impl(data: *const u8, len: usize, offset: usize) -> impl ExactSizeIterator<Item=*const T> {
        (0..len)
            .map(move |i| {
                let total_offset = offset * i;
                data.offset(total_offset as isize) as *const T
            })
    }

    /// Iterates over the item references in arena if the arena is alive.
    pub fn iter(&self) -> Option<impl ExactSizeIterator<Item=&T>> {
        if self._arena.is_alive() {
            Some(unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len, (*self._metadata)._offset)
                    .map(|ptr| std::mem::transmute::<*const T, &T>(ptr))
            })
        } else {
            None
        }
    }

    /// Iterates over the item references in arena, returns no items if the arena is dead.
    pub fn empty_if_dead_iter(&self) -> impl ExactSizeIterator<Item=&T> {
        EmptyIfDeadIter {
            is_alive: self._arena.is_alive(),
            inner: unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len, (*self._metadata)._offset)
                    .map(|ptr| std::mem::transmute::<*const T, &T>(ptr))
            }
        }
    }

    /// Iterates over the mutable item references in arena if the arena is alive.
    pub fn iter_mut(&self) -> Option<impl ExactSizeIterator<Item=&mut T>> {
        if self._arena.is_alive() {
            Some(unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len, (*self._metadata)._offset)
                    .map(|ptr| std::mem::transmute::<*const T, &mut T>(ptr))
            })
        } else {
            None
        }
    }

    /// Iterates over the mutable item references in arena, returns no items if the arena is dead.
    pub fn empty_if_dead_iter_mut(&mut self) -> impl ExactSizeIterator<Item=&mut T> {
        EmptyIfDeadIter {
            is_alive: self._arena.is_alive(),
            inner: unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len, (*self._metadata)._offset)
                    .map(|ptr| std::mem::transmute::<*const T, &mut T>(ptr))
            }
        }
    }
}

#[cfg(test)]
mod array {
    use crate::{Memory, Arena, Array, MemurIterator};

    #[test]
    fn has_items_when_iterating() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i64)).unwrap();
        for (i, (item, expected)) in items.empty_if_dead_iter().zip((0..12).map(|v| v as i64)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i8() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i8)).unwrap();
        for (i, (item, expected)) in items.empty_if_dead_iter().zip((0..12).map(|v| v as i8)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i16() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = Array::new(&arena, (0..12).map(|v| v as i16)).unwrap();
        for (i, (item, expected)) in items.empty_if_dead_iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn test_collect() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();

        let items3 = Array::new(
            &arena,
            (0..12)
                .map(|v| v as i16)
        )
            .unwrap()
            .empty_if_dead_iter()
            .map(|i: &i16| *i)
            .collect_array(&arena)
            .unwrap()
            .iter().unwrap()
            .map(|i: &i16| *i)
            .collect_array(&arena)
            .unwrap();

        for (i, (item, expected)) in items3.empty_if_dead_iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i16_but_not_when_arena_is_dead() {
        let memory = Memory::new();
        let items: Array<i16> = {
            let arena = Arena::new(&memory).unwrap();
            let items = Array::new(&arena, (0..12).map(|v| v as i16)).unwrap();
            for (i, (item, expected)) in items.empty_if_dead_iter().zip((0..12).map(|v| v as i16)).enumerate() {
                assert_eq!(*item, expected, "at index {}", i);
            }
            assert_eq!(12, items.len().unwrap());
            items
        };

        let sum = items.empty_if_dead_iter().fold(0, |acc, _| acc + 1);
        assert_eq!(0, sum);
        assert_eq!(None, items.len());
    }
}