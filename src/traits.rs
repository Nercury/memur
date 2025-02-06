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