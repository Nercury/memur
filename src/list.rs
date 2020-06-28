use crate::{Arena, WeakArena};
use std::ptr::null_mut;

const MAX_ITEMS: usize = 32;

struct PartialSequence<T> {
    items: [*mut T; MAX_ITEMS],
    next_list: *mut PartialSequence<T>,
    used_items: u16,
}

impl<T> PartialSequence<T> {
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
pub struct List<T> {
    arena: WeakArena,
    _len: u32,
    _first: *mut PartialSequence<T>,
    _last: *mut PartialSequence<T>,
}

impl<T> List<T> {
    /// Initializes a new list in arena and returns a handle to it.
    pub fn new(arena: &Arena) -> List<T> {
        unsafe {
            let starting_sequence = arena.upload_auto_drop(PartialSequence::empty());

            List {
                arena: arena.to_weak_arena(),
                _len: 0,
                _first: starting_sequence,
                _last: starting_sequence,
            }
        }
    }

    /// Iterates over the item references in arena if the arena is alive.
    pub fn iter(&mut self) -> Option<impl Iterator<Item=&T>> {
        struct State<T> {
            current: *mut PartialSequence<T>,
            index: usize,
        }

        Some((0..self._len)
            .scan(State { current: self._first, index: 0 }, |state, _| {
                if state.index >= MAX_ITEMS {
                    (*state).current = unsafe { (*(*state).current).next_list };
                    debug_assert_ne!(state.current, null_mut(), "seq != null");
                    state.index = 0;
                }
                let item = *unsafe { (*(*state).current).items.get_unchecked_mut(state.index) };
                debug_assert_ne!(item, null_mut(), "item != null");
                state.index += 1;
                Some(unsafe { std::mem::transmute::<*mut T, &T>(item) })
            }))
    }

    /// Iterates over the mutable item references in arena if the arena is alive.
    pub fn iter_mut(&mut self) -> Option<impl Iterator<Item=&mut T>> {
        struct State<T> {
            current: *mut PartialSequence<T>,
            index: usize,
        }

        Some((0..self._len)
            .scan(State { current: self._first, index: 0 }, |state, _| {
                if state.index >= MAX_ITEMS {
                    (*state).current = unsafe { (*(*state).current).next_list };
                    debug_assert_ne!(state.current, null_mut(), "seq != null");
                    state.index = 0;
                }
                let item = *unsafe { (*(*state).current).items.get_unchecked_mut(state.index) };
                debug_assert_ne!(item, null_mut(), "item != null");
                state.index += 1;
                Some(unsafe { std::mem::transmute::<*mut T, &mut T>(item) })
            }))
    }

    /// Appends a new item to list if the arena is alive.
    pub fn push(&mut self, item: T) -> Option<()> {
        match self.arena.arena() {
            None => None,
            Some(arena) => {
                unsafe {
                    if let Some(empty_slot) = (*self._last).take_empty_slot() {
                        let item_ptr = arena.upload_auto_drop(item);
                        *empty_slot = item_ptr;
                    } else {
                        let next_sequence = arena.upload_auto_drop(PartialSequence::empty());
                        (*self._last).next_list = next_sequence;
                        self._last = next_sequence;
                        let item_ptr = arena.upload_auto_drop(item);
                        let empty_slot = (*self._last).take_empty_slot().unwrap();
                        *empty_slot = item_ptr;
                    }
                }
                self._len += 1;
                Some(())
            },
        }
    }
}

#[cfg(test)]
mod list_tests {
    use crate::{Memory, Arena};
    use crate::List;
    use std::fmt::Debug;

    struct Compact<T> where T: Debug {
        value: T,
    }

    impl<T> Drop for Compact<T> where T: Debug {
        fn drop(&mut self) {
            println!("drop {:?}", self.value);
        }
    }

    #[test]
    fn simple_test() {
        let _obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem);
            let mut list = List::new(&arena);
            list.push(Compact { value: 1 });
            list.push(Compact { value: 2 });
            list.push(Compact { value: 3 });
            list.push(Compact { value: 4 });
            list.push(Compact { value: 5 });
            for (i, item) in (1..=5).zip(list.iter_mut().unwrap()) {
                assert_eq!(i, item.value);
            }
        };
    }

    #[test]
    fn many_items_test() {
        let _obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem);
            let mut list = List::new(&arena);
            for i in 0..super::MAX_ITEMS * 3 {
                list.push(Compact { value: i });
            }
            for (i, item) in (0..super::MAX_ITEMS * 3).zip(list.iter_mut().unwrap()) {
                assert_eq!(i, item.value);
            }
        };
    }
}