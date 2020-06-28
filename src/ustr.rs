use crate::{Arena, WeakArena, UploadError};
use std::ffi::{CStr, CString};
use std::str::Utf8Error;
use std::fmt::{Display, Debug};
use std::error::Error;

#[derive(Debug)]
pub enum UStrError {
    CStrIsNotUtf8(Utf8Error),
    StrContainsNul,
    StringIsTooLong { length: usize, max_size: usize },
    UploadError(UploadError),
}

impl Display for UStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UStrError::CStrIsNotUtf8(e) => write!(f, "Input CStr is not valid UTF-8 - {}", e),
            UStrError::UploadError(e) => write!(f, "Failed to upload to arena - {}", e),
            UStrError::StrContainsNul => Display::fmt("Input str slice contains nul", f),
            UStrError::StringIsTooLong { length, max_size } => write!(f, "Input string should be smaller than {} bytes, but was {}", max_size, length),
        }
    }
}

impl From<UploadError> for UStrError {
    fn from(other: UploadError) -> Self {
        UStrError::UploadError(other)
    }
}

impl Error for UStrError {}

const MAX_USTR: usize = u16::MAX as usize - 1;

/// UTF-8 string that does not contain nul values, and is stored with nul termination
/// for easy conversion to CStr.
///
/// This string is valid even when `Arena` is dropped, because it holds a weak arena reference
/// which does not return memory back to `Memory` as long as it is alive. That said, make sure to
/// drop all these strings to reclaim the memory.
#[derive(Clone)]
pub struct UStr {
    _arena: WeakArena,
    cstr_with_nul_len: u16,
    first: *mut u8,
}

impl Debug for UStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(AsRef::<str>::as_ref(self), f)
    }
}

impl Display for UStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(AsRef::<str>::as_ref(self), f)
    }
}

impl PartialEq for UStr {
    fn eq(&self, other: &Self) -> bool {
        if self.first == other.first {
            return true;
        }
        AsRef::<str>::as_ref(self).eq(AsRef::<str>::as_ref(other))
    }
}

impl PartialEq<str> for UStr {
    fn eq(&self, other: &str) -> bool {
        AsRef::<str>::as_ref(self).eq(other)
    }
}

impl PartialEq<UStr> for str {
    fn eq(&self, other: &UStr) -> bool {
        self.eq(AsRef::<str>::as_ref(other))
    }
}

impl PartialEq<UStr> for String {
    fn eq(&self, other: &UStr) -> bool {
        AsRef::<str>::as_ref(self).eq(AsRef::<str>::as_ref(other))
    }
}

impl PartialEq<UStr> for CStr {
    fn eq(&self, other: &UStr) -> bool {
        self.eq(AsRef::<CStr>::as_ref(other))
    }
}

impl PartialEq<UStr> for CString {
    fn eq(&self, other: &UStr) -> bool {
        AsRef::<CStr>::as_ref(self).eq(AsRef::<CStr>::as_ref(other))
    }
}

impl Eq for UStr {}

impl UStr {
    /// Initialize from &CStr.
    pub fn from_cstr(arena: &Arena, value: &CStr) -> Result<UStr, UStrError> {
        match value.to_str() {
            Ok(str) => {
                if str.len() > MAX_USTR {
                    return Err(UStrError::StringIsTooLong { length: str.len(), max_size: MAX_USTR });
                }
                Ok(unsafe { UStr::from_str_unchecked(arena, str)? })
            },
            Err(e) => Err(UStrError::CStrIsNotUtf8(e)),
        }
    }

    /// Initialize from `char*` which comes from a trusted source.
    pub fn from_strusted_cstr_ptr(arena: &Arena, bytes: *const i8) -> Result<UStr, UStrError> {
        UStr::from_cstr(arena, unsafe { CStr::from_ptr(bytes) })
    }

    /// Initialize from &str.
    pub fn from_str(arena: &Arena, value: &str) -> Result<UStr, UStrError> {
        if value.len() > MAX_USTR {
            return Err(UStrError::StringIsTooLong { length: value.len(), max_size: MAX_USTR });
        }

        for byte in value.bytes() {
            if byte == b'\0' {
                return Err(UStrError::StrContainsNul)
            }
        }

        Ok(unsafe { UStr::from_str_unchecked(arena, value)? })
    }

    unsafe fn from_str_unchecked(arena: &Arena, value: &str) -> Result<UStr, UploadError> {
        let bytes = value.as_bytes();
        let cstr_with_nul_len = bytes.len() + 1;
        let ptr = arena.upload_no_drop_bytes(cstr_with_nul_len, bytes.iter().map(|v| *v).chain(std::iter::once(0u8)))?;
        Ok(UStr {
            _arena: arena.to_weak_arena(),
            cstr_with_nul_len: cstr_with_nul_len as u16,
            first: ptr,
        })
    }

    /// Get pointer to `char*`.
    pub fn as_ptr(&self) -> *const i8 {
        self.first as *const i8
    }
}

impl AsRef<str> for UStr {
    fn as_ref(&self) -> &str {
        // potential access to weak arena
        // but memory is returned only when the weak reference is dropped, so this is ok
        let slice = unsafe { std::slice::from_raw_parts(self.first, (self.cstr_with_nul_len - 1) as usize) };
        unsafe { std::str::from_utf8_unchecked(slice) }
    }
}

impl AsRef<CStr> for UStr {
    fn as_ref(&self) -> &CStr {
        // potential access to weak arena
        // but memory is returned only when the weak reference is dropped, so this is ok
        unsafe { CStr::from_ptr(self.first as *const i8) }
    }
}

#[cfg(test)]
mod ustr_tests {
    use crate::{Memory, Arena};
    use crate::UStr;
    use std::ffi::CString;

    #[test]
    fn test_str() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        let str = UStr::from_str(&arena, "hello world!").expect("failed to create");
        assert_eq!("hello world!", &str);
        assert_eq!(&CString::new("hello world!").expect("ok"), &str);
    }

    #[test]
    fn test_str_with_nul() {
        let memory = Memory::new();
        let arena = Arena::new(&memory).unwrap();
        assert_eq!(None, UStr::from_str(&arena, "hello\0world!").ok());
    }
}