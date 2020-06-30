use crate::{Arena, WeakArena, UploadError};
use std::ptr::{null_mut};

const MAX_ITEMS: usize = 32;

struct PartialSequence<T> where T: Sized {
    items: [*mut T; MAX_ITEMS],
    next_list: *mut PartialSequence<T>,
    used_items: u16,
}

impl<T> PartialSequence<T> where T: Sized {
    pub fn empty() -> PartialSequence<T> {
        PartialSequence {
            items: [null_mut(); MAX_ITEMS],
            next_list: null_mut(),
            used_items: 0,
        }
    }

    pub unsafe fn take_empty_slot(&mut self) -> Option<&mut *mut T> {
        if self.used_items >= MAX_ITEMS as u16 {
            None
        } else {
            let index = self.used_items;
            self.used_items += 1;
            Some(self.items.get_unchecked_mut(index as usize))
        }
    }
}

/// Append-only list
// don't clone
pub struct List<T> where T: Sized {
    arena: WeakArena,
    _len: u32,
    _first: *mut PartialSequence<T>,
    _last: *mut PartialSequence<T>,
}

impl<T> List<T> where T: Sized {
    /// Initializes a new list in arena and returns a handle to it.
    pub fn new(arena: &Arena) -> Result<List<T>, UploadError> {
        unsafe {
            let starting_sequence = arena.upload_auto_drop(PartialSequence::empty())?;

            Ok(List {
                arena: arena.to_weak_arena(),
                _len: 0,
                _first: starting_sequence,
                _last: starting_sequence,
            })
        }
    }

    #[inline(always)]
    pub fn len(&self) -> u32 {
        self._len
    }

    /// Iterates over the item references in arena, returns no items if the arena is dead.
    #[inline(always)]
    pub fn empty_if_dead_iter(&self) -> impl ExactSizeIterator<Item=&T> {
        let map = |item| unsafe { std::mem::transmute::<*mut T, &T>(item) };
        if self.arena.is_alive() {
            ListIter {
                len: self._len as usize,
                index: 0,
                current: self._first,
            }.map(map)
        } else {
            ListIter {
                len: 0,
                index: 0,
                current: null_mut(),
            }.map(map)
        }
    }

    /// Iterates over the item references in arena if the arena is alive.
    pub fn iter(&self) -> Option<impl ExactSizeIterator<Item=&T>> {
        if self.arena.is_alive() {
            Some(ListIter {
                len: self._len as usize,
                index: 0,
                current: self._first,
            }.map(|item| unsafe { std::mem::transmute::<*mut T, &T>(item) }))
        } else {
            None
        }
    }

    /// Iterates over the mutable item references in arena, returns no items if the arena is dead.
    #[inline(always)]
    pub fn empty_if_dead_iter_mut(&mut self) -> impl ExactSizeIterator<Item=&mut T> {
        let map = |item| unsafe { std::mem::transmute::<*mut T, &mut T>(item) };
        if self.arena.is_alive() {
            ListIter {
                len: self._len as usize,
                index: 0,
                current: self._first,
            }.map(map)
        } else {
            ListIter {
                len: 0,
                index: 0,
                current: null_mut(),
            }.map(map)
        }
    }

    /// Iterates over the mutable item references in arena if the arena is alive.
    pub fn iter_mut(&mut self) -> Option<impl ExactSizeIterator<Item=&mut T>> {
        if self.arena.is_alive() {
            Some(ListIter {
                len: self._len as usize,
                index: 0,
                current: self._first,
            }.map(|item| unsafe { std::mem::transmute::<*mut T, &mut T>(item) }))
        } else {
            None
        }
    }

    /// Appends a new item to list if the arena is alive.
    pub fn push(&mut self, item: T) -> Result<(), UploadError> {
        let arena = self.arena.arena().ok_or(UploadError::ArenaIsNotAlive)?;

        unsafe {
            if let Some(empty_slot) = (*self._last).take_empty_slot() {
                let item_ptr = arena.upload_auto_drop(item)?;
                *empty_slot = item_ptr;
            } else {
                let next_sequence = arena.upload_auto_drop(PartialSequence::empty())?;
                (*self._last).next_list = next_sequence;
                self._last = next_sequence;
                let item_ptr = arena.upload_auto_drop(item)?;
                let empty_slot = (*self._last).take_empty_slot().unwrap();
                *empty_slot = item_ptr;
            }
        }
        self._len += 1;

        Ok(())
    }
}

impl<T> std::fmt::Debug for List<T> where T: std::fmt::Debug, T: Sized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        for i in self.empty_if_dead_iter() {
            list.entry(i);
        }
        list.finish()
    }
}

struct ListIter<K> {
    current: *mut PartialSequence<K>,
    index: usize,
    len: usize,
}

impl<K> ExactSizeIterator for ListIter<K> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<K> Iterator for ListIter<K> {
    type Item = *mut K;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.current == null_mut() {
                return None;
            }
            if self.index >= MAX_ITEMS {
                if (*self.current).next_list == null_mut() {
                    self.current = null_mut();
                    return None;
                }
                self.current = (*self.current).next_list;
                self.index = 0;
            }
            let item = (*self.current).items.get_unchecked(self.index);
            if *item == null_mut() {
                self.current = null_mut();
                None
            } else {
                self.index += 1;
                Some(*item)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

#[cfg(test)]
mod list_tests {
    use crate::{Memory, Arena, MemurIterator};
    use crate::List;
    use std::fmt::Debug;

    struct Compact<T> where T: Debug {
        value: T,
    }

    impl<T> Drop for Compact<T> where T: Debug {
        fn drop(&mut self) {
            //println!("drop {:?}", self.value);
        }
    }

    #[test]
    fn simple_test() {
        let _obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem).unwrap();
            let mut list = List::new(&arena).unwrap();
            assert_eq!(0, list.len());
            list.push(Compact { value: 1 }).unwrap();
            assert_eq!(1, list.len());
            list.push(Compact { value: 2 }).unwrap();
            assert_eq!(2, list.len());
            list.push(Compact { value: 3 }).unwrap();
            assert_eq!(3, list.len());
            list.push(Compact { value: 4 }).unwrap();
            assert_eq!(4, list.len());
            list.push(Compact { value: 5 }).unwrap();
            assert_eq!(5, list.len());
            for (i, item) in (1..=5).zip(list.empty_if_dead_iter()) {
                assert_eq!(i, item.value);
            }
        };
    }

    #[test]
    fn many_items_test() {
        let _obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem).unwrap();
            let mut list = List::new(&arena).unwrap();
            for i in 0..super::MAX_ITEMS * 3 {
                list.push(Compact { value: i }).unwrap();
            }
            for (i, item) in (0..super::MAX_ITEMS * 3).zip(list.empty_if_dead_iter_mut()) {
                assert_eq!(i, item.value);
            }
        };
    }

    #[test]
    fn test_collect() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();

        let items3 = (0..12)
            .map(|v| v as i16)
            .collect_list(&arena).unwrap()
            .empty_if_dead_iter()
            .map(|i: &i16| *i)
            .collect_list(&arena)
            .unwrap()
            .iter().unwrap()
            .map(|i: &i16| *i)
            .collect_list(&arena)
            .unwrap();

        for (i, (item, expected)) in items3.empty_if_dead_iter().zip((0..12).map(|v| v as i16)).enumerate() {
            assert_eq!(*item, expected, "at index {}", i);
        }
    }
}