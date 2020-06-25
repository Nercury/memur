/// Location of data in memory space.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct Elien {
    pub offset: usize,
    pub size: usize,
}

struct RawSection {
    data: [u8],
}

impl RawSection {

    #[inline(always)]
    fn allocated_len_ref(&self) -> &usize {
        slice_as_value_ref(self.at(Elien {
            offset: 0,
            size: std::mem::size_of::<usize>(),
        }))
    }

    #[inline(always)]
    fn allocated_len_mut(&mut self) -> &mut usize {
        slice_as_value_ref_mut(self.at_mut(Elien {
            offset: 0,
            size: std::mem::size_of::<usize>(),
        }))
    }

    /// Mark `size` amount of memory as used and return actual `Elien` location of it.
    ///
    /// Returns `None` if out of space.
    pub fn alloc(&mut self, size: usize) -> Option<Elien> {
        let prev_len = *self.allocated_len_ref();
        let next_len = prev_len + size;
        if self.data.len() < next_len as usize {
            return None;
        }

        if size == 0 {
            return Some(Elien {
                offset: 0,
                size: 0,
            });
        }

        let offset = prev_len;
        *self.allocated_len_mut() = next_len;

        Some(Elien {
            offset,
            size,
        })
    }

    /// Write slice data to `Elien` location.
    ///
    /// Panics if `Elien` is out of range or data size does not match `Elien` size.
    #[inline(always)]
    pub fn write(&mut self, el: Elien, data: &[u8]) {
        self.data[el.offset as usize..(el.offset + el.size) as usize].clone_from_slice(data);
    }

    /// Allocate and write data to new `Elien`, and return the `Elien` data location.
    ///
    /// Returns `None` if out of space.
    #[inline(always)]
    pub fn alloc_write(&mut self, data: &[u8]) -> Option<Elien> {
        let el = self.alloc(data.len())?;
        self.write(el, data);
        Some(el)
    }

    /// Return pointer to memory from an `Elien` offset.
    #[inline(always)]
    pub unsafe fn ptr_at_unchecked(&self, offset: usize) -> *const u8 {
        self.data.as_ptr().offset(offset as isize)
    }

    /// Return data slice used by `Elien`.
    ///
    /// Panics if `Elien` is out of range.
    #[inline(always)]
    pub fn at(&self, elien: Elien) -> &[u8] {
        &self.data[elien.offset as usize..(elien.offset + elien.size) as usize]
    }

    /// Return mutable data slice used by `Elien`.
    ///
    /// Panics if `Elien` is out of range.
    #[inline(always)]
    pub fn at_mut(&mut self, elien: Elien) -> &mut [u8] {
        &mut self.data[elien.offset as usize..(elien.offset + elien.size) as usize]
    }
}

/// Tracks used memory in a continuous memory block.
pub struct Section {
    data: Box<RawSection>,
}

impl Section {
    /// Create a new section from an existing continuous memory block.
    pub fn with_storage(data: Box<[u8]>) -> Section {
        if data.len() < std::mem::size_of::<usize>() {
            panic!("Storage block of size {} < {} too small", data.len(), std::mem::size_of::<usize>());
        }

        let mut s = Section {
            data: unsafe { std::mem::transmute::<Box<[u8]>, Box<RawSection>>(data) },
        };

        *s.allocated_len_mut() = std::mem::size_of::<usize>();

        s
    }

    #[inline(always)]
    fn allocated_len_ref(&self) -> &usize {
        self.data.allocated_len_ref()
    }

    #[inline(always)]
    fn allocated_len_mut(&mut self) -> &mut usize {
        self.data.allocated_len_mut()
    }

    /// Allocate and write data to new `Elien`, and return the `Elien` data location.
    ///
    /// Returns `None` if out of space.
    #[inline(always)]
    pub fn alloc_write(&mut self, data: &[u8]) -> Option<Elien> {
        let el = self.data.alloc(data.len())?;
        self.data.write(el, data);
        Some(el)
    }

    /// Return pointer to memory from an `Elien` offset.
    #[inline(always)]
    pub unsafe fn ptr_at_unchecked(&self, offset: usize) -> *const u8 {
        self.data.ptr_at_unchecked(offset)
    }

    /// Return data slice used by `Elien`.
    ///
    /// Panics if `Elien` is out of range.
    #[inline(always)]
    pub fn at(&self, elien: Elien) -> &[u8] {
        self.data.at(elien)
    }

    /// Return mutable data slice used by `Elien`.
    ///
    /// Panics if `Elien` is out of range.
    #[inline(always)]
    pub fn at_mut(&mut self, elien: Elien) -> &mut [u8] {
        self.data.at_mut(elien)
    }
}

// #[cfg(test)]
// mod section_tests {
//     use super::{Section, Elien};
//
//     #[test]
//     fn section_allocates_none() {
//         let mut bytes = Section::with_storage(vec![0; 0].into_boxed_slice());
//         assert_eq!(None, bytes.alloc_write(&[1]));
//     }
//
//     #[test]
//     #[should_panic]
//     fn section_out_of_bounds_access_panics() {
//         let bytes = Section::with_storage(vec![0; 0].into_boxed_slice());
//         bytes.at(Elien { offset: 0, size: 1 });
//     }
//
//     #[test]
//     fn section_allocates_exact() {
//         let mut bytes = Section::with_storage(vec![0; 8].into_boxed_slice());
//         assert_eq!(Some(Elien { offset: 0, size: 1 }), bytes.alloc_write(&[1]));
//         assert_eq!(&[1], bytes.at(Elien { offset: 0, size: 1 }));
//         assert_eq!(Some(Elien { offset: 1, size: 1 }), bytes.alloc_write(&[3]));
//         assert_eq!(&[3], bytes.at(Elien { offset: 1, size: 1 }));
//         assert_eq!(Some(Elien { offset: 2, size: 1 }), bytes.alloc_write(&[5]));
//         assert_eq!(&[5], bytes.at(Elien { offset: 2, size: 1 }));
//         assert_eq!(None, bytes.alloc_write(&[7]));
//     }
//
//     #[test]
//     fn section_does_not_overflow() {
//         let mut bytes = Section::with_storage(vec![0; 2].into_boxed_slice());
//         assert_eq!(Some(Elien { offset: 0, size: 1 }), bytes.alloc_write(&[1]));
//         assert_eq!(&[1], bytes.at(Elien { offset: 0, size: 1 }));
//         assert_eq!(None, bytes.alloc_write(&[2, 3]));
//         assert_eq!(Some(Elien { offset: 1, size: 1 }), bytes.alloc_write(&[5]));
//         assert_eq!(&[5], bytes.at(Elien { offset: 1, size: 1 }));
//         assert_eq!(None, bytes.alloc_write(&[7]));
//     }
// }

#[inline(always)]
fn slice_as_value_ref<T: Sized>(bytes: &[u8]) -> &T {
    unsafe {std::mem::transmute::<*const u8, &T>(bytes.as_ptr())}
}

#[inline(always)]
fn slice_as_value_ref_mut<T: Sized>(bytes: &mut [u8]) -> &mut T {
    unsafe {std::mem::transmute::<*mut u8, &mut T>(bytes.as_mut_ptr())}
}

#[inline(always)]
fn value_as_slice<T: Sized>(val: &T) -> &[u8] {
    let ptr_to_val = unsafe {std::mem::transmute::<&T, *const u8>(val) };
    unsafe { std::slice::from_raw_parts(ptr_to_val, std::mem::size_of::<T>()) }
}

#[inline(always)]
fn value_as_mut_slice<T: Sized>(val: &mut T) -> &mut [u8] {
    let ptr_to_val = unsafe {std::mem::transmute::<&T, *mut u8>(val) };
    unsafe { std::slice::from_raw_parts_mut(ptr_to_val, std::mem::size_of::<T>()) }
}