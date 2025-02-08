use crate::{Arena, List, UploadError, FixedArray, Array};

/// Implements collect to `Arena` allocated lists.
pub trait MemurIterator: Iterator {
    fn collect_list(self, arena: &Arena) -> Result<List<Self::Item>, UploadError>;

    fn collect_result_list<I, E>(self, arena: &Arena) -> Result<List<I>, E>
        where
            Self: Iterator<Item=Result<I, E>>,
            E: From<UploadError>;

    fn collect_fixed_array(self, arena: &Arena) -> Result<FixedArray<Self::Item>, UploadError> where Self: ExactSizeIterator;
    fn collect_array(self, arena: &Arena) -> Result<Array<Self::Item>, UploadError>;
}

impl<Q: Iterator> MemurIterator for Q {
    fn collect_list(self, arena: &Arena) -> Result<List<Self::Item>, UploadError>
    {
        let mut list = List::new(arena)?;
        for i in self {
            list.push(i)?;
        }
        Ok(list)
    }

    fn collect_result_list<I, E>(self, arena: &Arena) -> Result<List<I>, E>
        where
            Self: Iterator<Item=Result<I, E>>,
            E: From<UploadError>
    {
        let mut list = List::new(arena)?;
        for mi in self {
            let i = mi?;
            list.push(i)?;
        }
        Ok(list)
    }

    fn collect_fixed_array(self, arena: &Arena) -> Result<FixedArray<Self::Item>, UploadError> where Q: ExactSizeIterator {
        FixedArray::new(arena, self)
    }

    fn collect_array(self, arena: &Arena) -> Result<Array<Self::Item>, UploadError> {
        let (lower, _) = self.size_hint();
        let mut array = Array::with_capacity(arena, lower.max(4))?;
        for item in self {
            array.push(item)?;
        }
        Ok(array)
    }
}

/// Converts standard collections (Vec or slice) into an arena–allocated FixedArray.
///
/// For owned collections (Vec<T>), the items are moved into the FixedArray;
/// for borrowed ones (e.g. &[T]), the elements are cloned so T must implement Clone.
pub trait ToArenaFixedArray<T> {
    fn to_arena_fixed_array(self, arena: &Arena) -> Result<FixedArray<T>, UploadError>;
}

/// Converts standard collections (Vec or slice) into an arena–allocated Array.
///
/// (Note: For a borrowed slice, T must implement Clone so that the items can be copied.)
pub trait ToArenaArray<T> {
    fn to_arena_array(self, arena: &Arena) -> Result<Array<T>, UploadError>;
}

/// Converts standard collections (Vec or slice) into an arena–allocated List.
///
/// (Note: For a borrowed slice, T must implement Clone so that the items can be copied.)
pub trait ToArenaList<T> {
    fn to_arena_list(self, arena: &Arena) -> Result<List<T>, UploadError>;
}

impl<T> ToArenaFixedArray<T> for Vec<T> {
    fn to_arena_fixed_array(self, arena: &Arena) -> Result<FixedArray<T>, UploadError> {
        // Consume the Vec, converting its iterator (which is ExactSizeIterator)
        // into a FixedArray allocated in the arena.
        FixedArray::new(arena, self.into_iter())
    }
}

impl<T: Clone> ToArenaFixedArray<T> for &[T] {
    fn to_arena_fixed_array(self, arena: &Arena) -> Result<FixedArray<T>, UploadError> {
        // Iterate over a borrowed slice and clone each element into the new FixedArray.
        FixedArray::new(arena, self.iter().cloned())
    }
}

impl<T> ToArenaArray<T> for Vec<T> {
    fn to_arena_array(self, arena: &Arena) -> Result<Array<T>, UploadError> {
        // Convert the Vec into an iterator and then build an Array (growable)
        Array::from_iter(arena, self.into_iter())
    }
}

impl<T: Clone> ToArenaArray<T> for &[T] {
    fn to_arena_array(self, arena: &Arena) -> Result<Array<T>, UploadError> {
        // Convert a borrowed slice into an iterator, clone its items and build an Array.
        Array::from_iter(arena, self.iter().cloned())
    }
}

impl<T> ToArenaList<T> for Vec<T> {
    fn to_arena_list(self, arena: &Arena) -> Result<List<T>, UploadError> {
        List::from_iter(arena, self.into_iter())
    }
}

impl<T: Clone> ToArenaList<T> for &[T] {
    fn to_arena_list(self, arena: &Arena) -> Result<List<T>, UploadError> {
        List::from_iter(arena, self.iter().cloned())
    }
}