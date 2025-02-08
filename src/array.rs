use crate::{Arena, UploadError, WeakArena};
use std::ptr::null_mut;
use std::ops::{Index, IndexMut};

/// Arena‐uploaded metadata for the growable array. It stores:
/// - `_len`: the number of pushed items,
/// - `_capacity`: the size of the pointer table,
/// - `_ptrs`: a pointer to the table of pointers (each pointer refers to an item of type `T`).
#[repr(C)]
pub struct GrowableArrayMetadata<T> {
    pub _len: usize,
    pub _capacity: usize,
    pub _ptrs: *mut *mut T,
}

/// Custom drop function for a growable array. When the arena dies this function is invoked
/// (via the arena’s drop registration) to drop each item in the pointer table.
pub(crate) fn drop_growable_array<T>(data: *const u8) {
    let meta = unsafe { &mut *(data as *mut GrowableArrayMetadata<T>) };
    if meta._ptrs.is_null() {
        return;
    }
    for i in 0..meta._len {
        let item_ptr = unsafe { *meta._ptrs.add(i) };
        if !item_ptr.is_null() {
            unsafe { std::ptr::drop_in_place(item_ptr) };
        }
    }
    // The pointer table itself is not freed individually;
    // the arena will reclaim all memory when it is dropped.
}

/// A growable, arena–backed array type. Although its API is Vec–like,
/// the items are not stored contiguously but rather allocated individually
/// with their pointers stored in an arena–allocated pointer table.
pub struct Array<T>
where
    T: Sized,
{
    pub(crate) _arena: WeakArena,
    pub(crate) _metadata: *mut GrowableArrayMetadata<T>,
}

impl<T> Array<T> {
    /// Creates a new array with a default initial capacity.
    pub fn new(arena: &Arena) -> Result<Self, UploadError> {
        Self::with_capacity(arena, 4)
    }

    /// Creates a new array with the specified capacity for the pointer table.
    pub fn with_capacity(arena: &Arena, capacity: usize) -> Result<Self, UploadError> {
        unsafe {
            // Upload the metadata (with zero items initially).
            let metadata = arena.upload_no_drop::<GrowableArrayMetadata<T>>(GrowableArrayMetadata {
                _len: 0,
                _capacity: capacity,
                _ptrs: null_mut(),
            })?;
            // Register our custom drop function so that items get dropped when the arena dies.
            arena.push_custom_drop_fn(drop_growable_array::<T>, metadata as *const u8)?;

            // Allocate the pointer table (each entry is a *mut T).
            let ptrs = arena.alloc_no_drop_items_aligned_uninit::<*mut T>(
                capacity,
                std::mem::size_of::<*mut T>(),
            )? as *mut *mut T;
            (*metadata)._ptrs = ptrs;

            Ok(Array {
                _arena: arena.to_weak_arena(),
                _metadata: metadata,
            })
        }
    }

    /// Copies data to Vec
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.iter().cloned().collect()
    }

    /// Creates a new array by consuming an iterator.
    ///
    /// The capacity is chosen based on the iterator’s size hint (at least 4).
    pub fn from_iter<I: IntoIterator<Item = T>>(arena: &Arena, iter: I) -> Result<Self, UploadError> {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        // Ensure we have at least a small capacity.
        let mut array = Array::with_capacity(arena, lower.max(4))?;
        for item in iter {
            array.push(item)?;
        }
        Ok(array)
    }

    /// Returns the number of items in the array if the arena is alive.
    pub fn len(&self) -> Option<usize> {
        if self._arena.is_alive() {
            unsafe { Some((*self._metadata)._len) }
        } else {
            None
        }
    }

    /// Returns the capacity of the pointer table if the arena is alive.
    pub fn capacity(&self) -> Option<usize> {
        if self._arena.is_alive() {
            unsafe { Some((*self._metadata)._capacity) }
        } else {
            None
        }
    }

    /// Returns true if the array is empty (or if the arena is dead).
    pub fn is_empty(&self) -> bool {
        self.len().unwrap_or(0) == 0
    }

    /// Pushes a new item onto the array.
    ///
    /// The item is allocated in the arena and its pointer is stored. If there is no room in the pointer
    /// table, a new (larger) table is allocated and the existing pointers are copied over.
    pub fn push(&mut self, item: T) -> Result<(), UploadError> {
        if !self._arena.is_alive() {
            panic!("Arena is dead");
        }
        let arena = self._arena.arena().expect("Arena is dead");
        unsafe {
            let meta = &mut *self._metadata;
            if meta._len == meta._capacity {
                // Grow: double the capacity (or use 4 if capacity is 0).
                let new_capacity = if meta._capacity == 0 { 4 } else { meta._capacity * 2 };
                let new_ptrs = arena.alloc_no_drop_items_aligned_uninit::<*mut T>(
                    new_capacity,
                    std::mem::size_of::<*mut T>(),
                )? as *mut *mut T;
                std::ptr::copy_nonoverlapping(meta._ptrs, new_ptrs, meta._len);
                meta._ptrs = new_ptrs;
                meta._capacity = new_capacity;
            }
            // Allocate space for the new item.
            let item_ptr = arena.alloc_no_drop_items_aligned_uninit::<T>(
                1,
                std::mem::size_of::<T>(),
            )? as *mut T;
            std::ptr::write(item_ptr, item);
            // Store the pointer in the pointer table.
            *meta._ptrs.add(meta._len) = item_ptr;
            meta._len += 1;
        }
        Ok(())
    }

    /// Removes and returns the last item from the array.
    pub fn pop(&mut self) -> Option<T> {
        if !self._arena.is_alive() {
            return None;
        }
        unsafe {
            let meta = &mut *self._metadata;
            if meta._len == 0 {
                return None;
            }
            meta._len -= 1;
            let item_ptr = *meta._ptrs.add(meta._len);
            Some(std::ptr::read(item_ptr))
        }
    }

    /// Returns an iterator over shared references to the items.
    pub fn iter(&self) -> ArrayIter<T> {
        let len = self.len().unwrap_or(0);
        ArrayIter {
            array: self,
            index: 0,
            len,
        }
    }

    /// Returns an iterator over mutable references to the items.
    pub fn iter_mut(&mut self) -> ArrayIterMut<T> {
        let len = self.len().unwrap_or(0);
        ArrayIterMut {
            array: self,
            index: 0,
            len,
        }
    }
}

/// Iterator over shared references in an `Array<T>`.
pub struct ArrayIter<'a, T> {
    array: &'a Array<T>,
    index: usize,
    len: usize,
}

impl<'a, T> Iterator for ArrayIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            unsafe {
                let meta = &*self.array._metadata;
                let item_ptr = *meta._ptrs.add(self.index);
                self.index += 1;
                Some(&*item_ptr)
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for ArrayIter<'a, T> {}

/// Iterator over mutable references in an `Array<T>`.
pub struct ArrayIterMut<'a, T> {
    array: &'a mut Array<T>,
    index: usize,
    len: usize,
}

impl<'a, T> Iterator for ArrayIterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            unsafe {
                let meta = &mut *self.array._metadata;
                let item_ptr = *meta._ptrs.add(self.index);
                self.index += 1;
                Some(&mut *item_ptr)
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for ArrayIterMut<'a, T> {}

impl<T> Index<usize> for Array<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let meta = &*self._metadata;
            if index >= meta._len {
                panic!("index out of bounds");
            }
            &*(*meta._ptrs.add(index))
        }
    }
}

impl<T> IndexMut<usize> for Array<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe {
            let meta = &mut *self._metadata;
            if index >= meta._len {
                panic!("index out of bounds");
            }
            &mut *(*meta._ptrs.add(index))
        }
    }
}

impl<T> std::fmt::Debug for Array<T> where T: std::fmt::Debug, T: Sized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        for i in self.iter() {
            list.entry(i);
        }
        list.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use super::*;
    use crate::{Arena, Memory, MemurIterator};
    use crate::dropflag::{Droppable, DropFlag};

    #[test]
    fn test_from_iter_numbers() {
        // Create a new arena.
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        // Collect the range 0..10 into our arena Array.
        let array = Array::from_iter(&arena, 0..10).unwrap();
        assert_eq!(array.len().unwrap(), 10);
        // Verify that each item is correct.
        for (i, &item) in array.iter().enumerate() {
            assert_eq!(item, i);
        }
    }

    #[test]
    fn test_collect_array_ext() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        // Use the extension trait to collect an iterator.
        let array = (10..20).collect_array(&arena).unwrap();
        assert_eq!(array.len().unwrap(), 10);
        for (i, &item) in array.iter().enumerate() {
            assert_eq!(item, 10 + i);
        }
    }

    #[test]
    fn test_push_pop() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let mut array = Array::new(&arena).unwrap();
        for i in 0..5 {
            array.push(i).unwrap();
        }
        assert_eq!(array.len().unwrap(), 5);
        // Pop all items and verify the order.
        for i in (0..5).rev() {
            let popped = array.pop().unwrap();
            assert_eq!(popped, i);
        }
        assert_eq!(array.len().unwrap(), 0);
    }

    #[test]
    fn test_droppable_collect() {
        // This test uses your dropflag module to check that items are dropped.
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let flag1 = DropFlag::new(RefCell::new(false));
        let flag2 = DropFlag::new(RefCell::new(false));

        let d1 = Droppable { dropflag: flag1.clone() };
        let d2 = Droppable { dropflag: flag2.clone() };

        {
            // Collect two droppable items into the array.
            let _array = Array::from_iter(&arena, vec![d1, d2]).unwrap();
            // When the arena is eventually dropped, our custom drop function will
            // call drop_in_place on each item.
        }
        // Drop the arena explicitly (which will run the registered drop functions).
        drop(arena);

        // Now check that each drop flag was set.
        assert_eq!(*flag1.borrow(), true);
        assert_eq!(*flag2.borrow(), true);
    }
}
