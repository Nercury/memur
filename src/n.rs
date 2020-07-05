use crate::{WeakArena, Arena, UploadError};

/// A wrapper of struct that is stored in arena memory.
// can't clone because can be accessed as mutable
pub struct N<T> {
    _arena: WeakArena,
    _ptr: *mut T,
}

impl<T> N<T> {
    /// Stores the value in arena and returns a handle to it.
    pub fn new(arena: &Arena, value: T) -> Result<N<T>, UploadError> {
        Ok(N {
            _arena: arena.to_weak_arena(),
            _ptr: unsafe { arena.upload_auto_drop(value)? },
        })
    }

    /// Returns a reference to value if the arena is alive.
    pub fn val(&self) -> Option<&T> {
        if self._arena.is_alive() {
            Some(unsafe { std::mem::transmute::<*mut T, &T>(self._ptr) })
        } else {
            None
        }
    }

    /// Returns a mutable reference to value if the arena is alive.
    pub fn var(&mut self) -> Option<&mut T> {
        if self._arena.is_alive() {
            Some(unsafe { std::mem::transmute::<*mut T, &mut T>(self._ptr) })
        } else {
            None
        }
    }

    pub fn outlives<O>(&self, value: O) -> Result<N<O>, UploadError> {
        match self._arena.arena() {
            None => Err(UploadError::ArenaIsNotAlive),
            Some(arena) => N::new(&arena, value),
        }
    }
}