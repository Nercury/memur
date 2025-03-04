use crate::{Arena, UploadError, WeakArena};
use crate::dontdothis::{next_item_aligned_start, value_as_slice};
use std::ptr::{null_mut};
use crate::iter::EmptyIfDeadIter;
use std::borrow::Borrow;
use std::ops::{Index, IndexMut, Range, RangeFrom, RangeTo, RangeToInclusive, RangeFull};
use crate::array_uninit::{ArrayMetadata, drop_array, UninitArray};

/// A helper to safely initialize items of `UninitArray`.
pub struct ArrayInitializer<T> where T: Sized {
    pub (crate) uninit_array: UninitArray<T>,
    pub (crate) initialized_len: usize,
}

impl<T> ArrayInitializer<T> where T: Sized {
    /// Push new item to `UninitArray`.
    pub fn push(&mut self, item: T) {
        if self.initialized_len < self.uninit_array.capacity() {
            let target_byte_ptr = unsafe { self.uninit_array.data_mut().offset(self.initialized_len as isize) as *mut u8 };
            let ref_to_target = unsafe { std::slice::from_raw_parts_mut(target_byte_ptr, std::mem::size_of::<T>()) };
            let ref_to_source = unsafe { value_as_slice(&item) };
            for (in_byte, out_byte) in ref_to_source.iter().zip(ref_to_target.iter_mut()) {
                *out_byte = *in_byte;
            }
            self.initialized_len += 1;
        }
    }

    /// Calling this function finalizes the array initialization. The number of items added over
    /// this initializer should be lower or equal `UninitArray` length.
    pub fn initialized(self) -> Option<FixedArray<T>> {
        if self.initialized_len > self.uninit_array.capacity() {
            None
        } else {
            Some(unsafe { self.uninit_array.initialized_to_len(self.initialized_len) })
        }
    }
}

/// Continuous memory block containing many elements of the same type.
pub struct FixedArray<T> where T: Sized {
    pub (crate) _arena: WeakArena,
    pub (crate) _metadata: *mut ArrayMetadata<T>,
}

impl<T> FixedArray<T> where T: Sized {
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

    /// Returns true if arena is dead or array is empty.
    pub fn is_empty(&self) -> bool {
        self.len().unwrap_or(0) == 0
    }

    /// Copies data into Vec
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.as_ref().to_vec()
    }

    /// Creates a new array with specified capacity and does not place data to it, the array items are not initialized.
    ///
    /// If array is dropped in this state, nothing happens, because the len is zero.
    ///
    /// Once the items are initialized, this array can be converted to `Array` type.
    /// The total number of items in array can not exceed the initial capacity.
    pub fn with_capacity(arena: &Arena, capacity: usize) -> Result<UninitArray<T>, UploadError> {
        unsafe {
            let metadata = arena.upload_no_drop::<ArrayMetadata<T>>(ArrayMetadata::<T> {
                _len: 0,
                _data: null_mut(),
            })?;

            arena.push_custom_drop_fn(drop_array::<T>, metadata as *const u8)?;

            let ptr = arena.alloc_no_drop_items_aligned_uninit::<T>(capacity, std::mem::size_of::<T>())? as *mut u8;
            (*metadata)._data = ptr as *mut T;

            Ok(UninitArray {
                _arena: arena.to_weak_arena(),
                _capacity: capacity,
                _metadata: metadata,
            })
        }
    }

    /// Creates a new array and places the data to it.
    pub fn new(arena: &Arena, iter: impl ExactSizeIterator<Item=T>) -> Result<FixedArray<T>, UploadError> {
        unsafe {
            let len = iter.len();
            let metadata = arena.upload_no_drop::<ArrayMetadata<T>>(ArrayMetadata::<T> {
                _len: len,
                _data: null_mut(),
            })?;

            arena.push_custom_drop_fn(drop_array::<T>, metadata as *const u8)?;

            // Prepare a memory block in arena, that is correctly aligned for the type T,
            // the item size also needs to be such that pointers to items are valid.
            let ptr = arena.alloc_no_drop_items_aligned_uninit::<T>(len, std::mem::size_of::<T>())? as *mut u8;
            (*metadata)._data = ptr as *mut T;

            // Consume items in iterator
            for (index, item) in iter.enumerate() {
                // Convert pointer to item into pointer to first item byte
                let item_ptr = std::mem::transmute::<&T, *const u8>(&item);
                // Get a pointer to nth element inside arena
                let arena_item_start_ptr = ptr.offset((index * Self::aligned_item_size()) as isize);
                // Convert source and target byte pointers to slices, they are easier to work with
                // than raw pointers
                let item_as_bytes = std::slice::from_raw_parts(item_ptr, std::mem::size_of::<T>());
                let arena_location_bytes = std::slice::from_raw_parts_mut(arena_item_start_ptr, std::mem::size_of::<T>());
                // Copy bytes
                for (inb, outb) in item_as_bytes.iter().zip(arena_location_bytes.iter_mut()) {
                    *outb = *inb;
                }
                // Forget the item, no more pointers are pointing to it.
                // The item will be restored back from bytes and dropped together with arena later.
                std::mem::forget(item);
            }

            Ok(FixedArray {
                _arena: arena.to_weak_arena(),
                _metadata: metadata,
            })
        }
    }

    pub (crate) unsafe fn iter_impl(data: *const u8, len: usize) -> impl ExactSizeIterator<Item=*const T> {
        (0..len)
            .map(move |i| {
                let total_offset = std::mem::size_of::<T>() * i;
                data.offset(total_offset as isize) as *const T
            })
    }

    /// Iterates over the item references in arena if the arena is alive.
    pub fn safer_iter(&self) -> Option<impl ExactSizeIterator<Item=&T>> {
        if self._arena.is_alive() {
            Some(unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len)
                    .map(|ptr| std::mem::transmute::<*const T, &T>(ptr))
            })
        } else {
            None
        }
    }

    /// Iterates over the item references in arena, returns no items if the arena is dead.
    pub fn iter(&self) -> impl ExactSizeIterator<Item=&T> {
        EmptyIfDeadIter {
            is_alive: self._arena.is_alive(),
            inner: unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len)
                    .map(|ptr| std::mem::transmute::<*const T, &T>(ptr))
            }
        }
    }

    /// Iterates over the mutable item references in arena if the arena is alive.
    pub fn safer_iter_mut(&self) -> Option<impl ExactSizeIterator<Item=&mut T>> {
        if self._arena.is_alive() {
            Some(unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len)
                    .map(|ptr| std::mem::transmute::<*const T, &mut T>(ptr))
            })
        } else {
            None
        }
    }

    /// Iterates over the mutable item references in arena, returns no items if the arena is dead.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item=&mut T> {
        EmptyIfDeadIter {
            is_alive: self._arena.is_alive(),
            inner: unsafe {
                Self::iter_impl((*self._metadata)._data as *const u8, (*self._metadata)._len)
                    .map(|ptr| std::mem::transmute::<*const T, &mut T>(ptr))
            }
        }
    }
}

impl<T> Index<Range<usize>> for FixedArray<T> {
    type Output = [T];

    #[inline(always)]
    fn index(&self, range: Range<usize>) -> &[T] {
        &self.as_ref()[range.start..range.end]
    }
}

impl<T> IndexMut<Range<usize>> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, range: Range<usize>) -> &mut [T] {
        &mut self.as_mut()[range.start..range.end]
    }
}

impl<T> Index<RangeFrom<usize>> for FixedArray<T> {
    type Output = [T];

    #[inline(always)]
    fn index(&self, range: RangeFrom<usize>) -> &[T] {
        &self.as_ref()[range.start..]
    }
}

impl<T> IndexMut<RangeFrom<usize>> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, range: RangeFrom<usize>) -> &mut [T] {
        &mut self.as_mut()[range.start..]
    }
}

impl<T> Index<RangeTo<usize>> for FixedArray<T> {
    type Output = [T];

    #[inline(always)]
    fn index(&self, range: RangeTo<usize>) -> &[T] {
        &self.as_ref()[..range.end]
    }
}

impl<T> IndexMut<RangeTo<usize>> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, range: RangeTo<usize>) -> &mut [T] {
        &mut self.as_mut()[..range.end]
    }
}

impl<T> Index<RangeToInclusive<usize>> for FixedArray<T> {
    type Output = [T];

    #[inline(always)]
    fn index(&self, range: RangeToInclusive<usize>) -> &[T] {
        &self.as_ref()[..=range.end]
    }
}

impl<T> IndexMut<RangeToInclusive<usize>> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, range: RangeToInclusive<usize>) -> &mut [T] {
        &mut self.as_mut()[..=range.end]
    }
}

impl<T> Index<RangeFull> for FixedArray<T> {
    type Output = [T];

    #[inline(always)]
    fn index(&self, _: RangeFull) -> &[T] {
        self.as_ref()
    }
}

impl<T> IndexMut<RangeFull> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, _: RangeFull) -> &mut [T] {
        self.as_mut()
    }
}

impl<T> Index<usize> for FixedArray<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.as_ref()[index]
    }
}

impl<T> IndexMut<usize> for FixedArray<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.as_mut()[index]
    }
}

impl<T> AsRef<[T]> for FixedArray<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts((*self._metadata)._data as *const T, (*self._metadata)._len) }
    }
}

impl<T> AsMut<[T]> for FixedArray<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut((*self._metadata)._data as *mut T, (*self._metadata)._len) }
    }
}

impl<T> Borrow<[T]> for FixedArray<T> {
    #[inline(always)]
    fn borrow(&self) -> &[T] {
        self.as_ref()
    }
}

impl<T> std::fmt::Debug for FixedArray<T> where T: std::fmt::Debug, T: Sized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        for i in self.iter() {
            list.entry(i);
        }
        list.finish()
    }
}

impl<T> PartialEq for FixedArray<T> where T: PartialEq, T: Sized {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

#[cfg(test)]
mod array {
    use crate::{Memory, Arena, FixedArray, MemurIterator};

    #[test]
    fn has_items_when_iterating() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = FixedArray::new(&arena, (0..12).map(|v| v as i64)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i64)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i8() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = FixedArray::new(&arena, (0..12).map(|v| v as i8)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i8)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i16() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let items = FixedArray::new(&arena, (0..12).map(|v| v as i16)).unwrap();
        for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn test_collect() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();

        let items3 = FixedArray::new(
            &arena,
            (0..12)
                .map(|v| v as i16)
        )
            .unwrap()
            .iter()
            .map(|i: &i16| *i)
            .collect_fixed_array(&arena)
            .unwrap()
            .safer_iter().unwrap()
            .map(|i: &i16| *i)
            .collect_fixed_array(&arena)
            .unwrap();

        for (i, (item, expected)) in items3.iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }

    #[test]
    fn has_items_when_iterating_items_i16_but_not_when_arena_is_dead() {
        let memory = Memory::new();
        let items: FixedArray<i16> = {
            let arena = Arena::new(&memory).unwrap();
            let items = FixedArray::new(&arena, (0..12).map(|v| v as i16)).unwrap();
            for (i, (item, expected)) in items.iter().zip((0..12).map(|v| v as i16)).enumerate() {
                assert_eq!(*item, expected, "at index {}", i);
            }
            assert_eq!(12, items.len().unwrap());
            items
        };

        let sum = items.iter().fold(0, |acc, _| acc + 1);
        assert_eq!(0, sum);
        assert_eq!(None, items.len());
    }
}