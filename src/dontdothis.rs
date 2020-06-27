#![allow(dead_code)]
/**!

Reinterpret portions of memory as a struct and vice-versa.

*/

#[inline(always)]
pub unsafe fn slice_as_value_ref<T: Sized>(bytes: &[u8]) -> &T {
    std::mem::transmute::<*const u8, &T>(bytes.as_ptr())
}

#[inline(always)]
pub unsafe fn slice_as_value_ref_mut<T: Sized>(bytes: &mut [u8]) -> &mut T {
    std::mem::transmute::<*mut u8, &mut T>(bytes.as_mut_ptr())
}

#[inline(always)]
pub unsafe fn value_as_slice<T: Sized>(val: &T) -> &[u8] {
    let ptr_to_val = std::mem::transmute::<&T, *const u8>(val);
    std::slice::from_raw_parts(ptr_to_val, std::mem::size_of::<T>())
}

#[inline(always)]
pub unsafe fn value_as_mut_slice<T: Sized>(val: &mut T) -> &mut [u8] {
    let ptr_to_val = std::mem::transmute::<&T, *mut u8>(val);
    std::slice::from_raw_parts_mut(ptr_to_val, std::mem::size_of::<T>())
}