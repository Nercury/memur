use crate::dontdothis;

pub enum PlacementError {
    NotEnoughSpaceInBlock,
    ItemTooBig,
}

struct BlockMetadata {
    next_item_offset: usize,
    previous_block: Option<Block>,
}

impl BlockMetadata {
    pub unsafe fn init_in_slice(slice: &mut [u8]) -> Option<()> {
        if std::mem::size_of::<BlockMetadata>() > slice.len() {
            None
        } else {
            let metadata = BlockMetadata {
                next_item_offset: std::mem::size_of::<BlockMetadata>(),
                previous_block: None,
            };
            let metadata_as_slice = dontdothis::value_as_slice(&metadata);
            for (inbyte, outbyte) in metadata_as_slice.iter().zip(slice.iter_mut()) {
                *outbyte = *inbyte;
            }
            std::mem::forget(metadata);
            Some(())
        }
    }

    #[inline(always)]
    pub unsafe fn reinterpret_from_slice_ptr_mut(slice: &mut [u8]) -> *mut BlockMetadata {
        dontdothis::slice_as_value_ref_mut::<BlockMetadata>(slice)
    }

    #[inline(always)]
    pub unsafe fn reinterpret_from_slice_ptr(slice: &[u8]) -> *const BlockMetadata {
        dontdothis::slice_as_value_ref::<BlockMetadata>(slice)
    }

    #[inline(always)]
    pub unsafe fn reinterpret_from_slice_mut<'a, 'b>(slice: &'a mut [u8]) -> &'b mut BlockMetadata {
        std::mem::transmute::<*mut BlockMetadata, &mut BlockMetadata>(
            BlockMetadata::reinterpret_from_slice_ptr_mut(slice)
        )
    }

    #[inline(always)]
    pub unsafe fn reinterpret_from_slice<'a, 'b>(slice: &'a [u8]) -> &'b BlockMetadata {
        std::mem::transmute::<*const BlockMetadata, &BlockMetadata>(
            BlockMetadata::reinterpret_from_slice_ptr(slice)
        )
    }
}

pub struct Block {
    data: Box<[u8]>,
}

impl Block {
    pub fn new(mut data: Box<[u8]>) -> Block {
        unsafe { BlockMetadata::init_in_slice(&mut *data).expect("init metadata in block") };
        Block {
            data
        }
    }

    pub unsafe fn set_previous_block(&mut self, block: Block) {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        metadata.previous_block = Some(block);
    }

    pub unsafe fn push<T>(&mut self, value: T) -> Result<*mut T, PlacementError> {
        match self.push_copy(&value) {
            Err(e) => Err(e),
            Ok(ptr) => {
                std::mem::forget(value);
                Ok(ptr)
            },
        }
    }

    pub fn largest_item_size(&self) -> usize {
        self.data.len() - std::mem::size_of::<BlockMetadata>()
    }

    pub unsafe fn push_copy<T>(&mut self, value: &T) -> Result<*mut T, PlacementError> {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        let align = std::mem::align_of::<T>();
        let padding = (align - (metadata.next_item_offset % align)) % align;
        let aligned = metadata.next_item_offset + padding;
        let end = aligned + std::mem::size_of::<T>();
        if end > self.data.len() {
            if std::mem::size_of::<T>() > self.largest_item_size() {
                Err(PlacementError::ItemTooBig)
            } else {
                Err(PlacementError::NotEnoughSpaceInBlock)
            }
        } else {
            let target_slice = &mut self.data[aligned..];
            let source_slice = dontdothis::value_as_slice(value);
            for (inbyte, outbyte) in source_slice.iter().zip(target_slice.iter_mut()) {
                *outbyte = *inbyte;
            }
            metadata.next_item_offset = end;
            Ok(dontdothis::slice_as_value_ref_mut::<T>(target_slice))
        }
    }

    pub unsafe fn into_previous_block_and_data(mut self) -> (Option<Block>, Box<[u8]>) {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        let mut block = None;
        std::mem::swap(&mut block, &mut metadata.previous_block);
        (block, self.data)
    }

    pub fn remaining_bytes_for_alignment<T>(&self) -> (isize, usize) {
        let metadata = unsafe { BlockMetadata::reinterpret_from_slice(&*self.data) };
        let align = std::mem::align_of::<T>();
        let padding = (align - (metadata.next_item_offset % align)) % align;
        let aligned = metadata.next_item_offset + padding;
        (self.data.len() as isize - aligned as isize, aligned)
    }

    pub unsafe fn upload_bytes_unchecked(&mut self, aligned_start: usize, len: usize, value: impl Iterator<Item=u8>) -> *mut u8 {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        let end = aligned_start + len;
        debug_assert!(end <= self.data.len(), "upload_bytes_unchecked end <= data.len");
        let target_slice = &mut self.data[aligned_start..];
        for (inbyte, outbyte) in value.zip(target_slice.iter_mut()) {
            *outbyte = inbyte;
        }
        metadata.next_item_offset = end;
        target_slice.as_mut_ptr()
    }
}