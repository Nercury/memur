use crate::{Arena, List, UploadError};

/// Implements collect to `Arena` allocated lists.
pub trait MemurIterator: Iterator {
    fn collect_list(self, arena: &Arena) -> Result<List<Self::Item>, UploadError>;

    fn collect_result_list<I, E>(self, arena: &Arena) -> Result<List<I>, E>
        where
            Self: Iterator<Item=Result<I, E>>,
            E: From<UploadError>;
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
}