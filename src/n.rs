use crate::{WeakArena, Arena, UploadError, DropFn};
use std::ptr::null_mut;

pub struct DropItem {
    pub fun: DropFn,
    pub data: *const u8,
    pub next: *mut DropItem,
}

impl DropItem {
    #[inline(always)]
    pub unsafe fn execute(&self) {
        (self.fun)(self.data);
    }
}

struct NMetadata<T> {
    value: T,
    outlives: *mut DropItem,
}

impl<T> Drop for NMetadata<T> {
    fn drop(&mut self) {
        let mut outlives = self.outlives;
        self.outlives = null_mut();
        while outlives != null_mut() {
            trace!("drop outlives");
            unsafe {
                (*outlives).execute();
                let next = (*outlives).next;
                (*outlives).next = null_mut();
                outlives = next;
            }
        }
        trace!("drop NMetadata");
    }
}

/// A wrapper of struct that is stored in arena memory.
// can't clone because can be accessed as mutable
pub struct N<T> {
    _arena: WeakArena,
    _ptr: *mut NMetadata<T>,
}

impl<T> N<T> {
    /// Stores the value in arena and returns a handle to it.
    pub fn new(arena: &Arena, value: T) -> Result<N<T>, UploadError> {
        let wrapped = NMetadata { value, outlives: null_mut() };
        let (item_ptr, _) = unsafe { arena.upload_auto_drop(wrapped)? };
        Ok(N {
            _arena: arena.to_weak_arena(),
            _ptr: item_ptr,
        })
    }

    /// Returns a reference to value or panics if arena is dead.
    pub fn expect(&self, message: &str) -> (Arena, &T) {
        if let Some(arena) = self._arena.arena() {
            (arena, &unsafe { std::mem::transmute::<*mut NMetadata<T>, &NMetadata<T>>(self._ptr) }.value)
        } else {
            panic!("{}", message);
        }
    }

    /// Returns a reference to value if the arena is alive.
    pub fn val(&self) -> Option<&T> {
        if self._arena.is_alive() {
            Some(&unsafe { std::mem::transmute::<*mut NMetadata<T>, &NMetadata<T>>(self._ptr) }.value)
        } else {
            None
        }
    }

    /// Returns a mutable reference to value or panics if arena is dead.
    pub fn expect_mut(&self, message: &str) -> (Arena, &mut T) {
        if let Some(arena) = self._arena.arena() {
            (arena, &mut unsafe { std::mem::transmute::<*mut NMetadata<T>, &mut NMetadata<T>>(self._ptr) }.value)
        } else {
            panic!("{}", message);
        }
    }

    /// Returns a mutable reference to value if the arena is alive.
    pub fn var(&mut self) -> Option<&mut T> {
        if self._arena.is_alive() {
            Some(&mut unsafe { std::mem::transmute::<*mut NMetadata<T>, &mut NMetadata<T>>(self._ptr) }.value)
        } else {
            None
        }
    }

    /// Puts another `value` to the same `Arena` and ensures that it is dropped only after this
    /// value is dropped, in other words, this struct should outlive the specified struct.
    /// Super useful for managing deterministic drop order.
    pub fn outlives<O>(&self, value: O) -> Result<N<O>, UploadError> {
        match self._arena.arena() {
            None => Err(UploadError::ArenaIsNotAlive),
            Some(arena) => {
                let wrapped = NMetadata { value, outlives: null_mut() };
                let o_wrapper_ptr = unsafe { arena.upload_no_drop(wrapped)? };
                let md = unsafe { std::mem::transmute::<*mut NMetadata<T>, &mut NMetadata<T>>(self._ptr) };
                let drop_item = unsafe { arena.upload_no_drop(DropItem {
                    fun: |data| {
                        trace!("drop closure {:?}", data);
                        let o_ref = std::mem::transmute::<*const u8, &NMetadata<O>>(data);
                        let o = std::mem::transmute_copy::<NMetadata<O>, NMetadata<O>>(o_ref);
                        std::mem::drop(o);
                    },
                    data: o_wrapper_ptr as *const u8,
                    next: null_mut(),
                }) }?;

                if md.outlives == null_mut() {
                    md.outlives = drop_item;
                } else { unsafe {
                    (*drop_item).next = md.outlives;
                    md.outlives = drop_item;
                } }
                Ok(N {
                    _arena: self._arena.clone(),
                    _ptr: o_wrapper_ptr,
                })
            },
        }
    }
}