use crate::{Arena, UploadError, WeakArena};

pub struct Array<T> where T: Sized {
    _arena: WeakArena,
    _len: usize,
    _ptr: *mut T,
}

impl<T> Array<T> where T: Sized {
    pub fn new(arena: &Arena, iter: impl ExactSizeIterator<Item=T>) -> Result<Array<T>, UploadError> {
        unsafe {
            let len = iter.len();
            let total_bytes_len = std::mem::size_of::<T>() * len;
            let ptr = arena.upload_no_drop_bytes(
                total_bytes_len,
                iter.flat_map(|i| {
                    let ptr_to_forgotten_value = std::mem::transmute::<&T, *const u8>(&i);
                    std::mem::forget(i);
                    let magic_slice: &'static [u8] = std::slice::from_raw_parts(
                        ptr_to_forgotten_value, std::mem::size_of::<T>());
                    magic_slice.iter().map(|b| *b)
                })
            )?;
            Ok(Array {
                _arena: arena.to_weak_arena(),
                _len: len,
                _ptr: ptr as *mut T,
            })
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        unsafe { std::slice::from_raw_parts(self._ptr as *const T, self._len) }.iter()
    }

    pub fn iter_mut(&self) -> impl Iterator<Item=&mut T> {
        unsafe { std::slice::from_raw_parts_mut(self._ptr, self._len) }.iter_mut()
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