use std::collections::VecDeque;
use crate::droplist::{DropList, DropListWriteResult};

mod droplist;
mod dontdothis;
mod list;
mod ustr;

pub use list::List;
pub use ustr::UStr;
use std::ptr::null_mut;

struct ArenaMemoryInstance {
    free_blocks: VecDeque<Box<[u8]>>,
    max_free_blocks: i32,
    min_free_blocks: i32,
    new_block_size: usize,
}

impl ArenaMemoryInstance {
    pub fn new() -> ArenaMemoryInstance {
        let mut i = ArenaMemoryInstance {
            free_blocks: VecDeque::with_capacity(1024),
            // max_free_blocks: 1024 * 16,
            // min_free_blocks: 1024 * 4,
            // new_block_size: 1024 * 1024,
            max_free_blocks: 4,
            min_free_blocks: 2,
            new_block_size: 1024*32,
        };
        i.check_if_not_enough_blocks_and_initialize();
        i
    }

    fn check_if_not_enough_blocks_and_initialize(&mut self) {
        if self.free_blocks.len() < self.min_free_blocks as usize {
            while self.free_blocks.len() < self.max_free_blocks as usize {
                println!("-- init   block of size {}", self.new_block_size);
                self.free_blocks.push_front(vec![0u8; self.new_block_size as usize].into_boxed_slice());
            }
        }
    }

    /// Returns cleaned-up memory size
    pub fn cleanup(&mut self) -> usize {
        let mut cleaned_up_size = 0;
        while self.free_blocks.len() > self.max_free_blocks as usize {
            if let Some(block) = self.free_blocks.pop_front() {
                cleaned_up_size += block.len();
                println!("-- clean  block of size {}", block.len());
            }
        }
        cleaned_up_size
    }

    pub fn take_block(&mut self) -> Box<[u8]> {
        self.check_if_not_enough_blocks_and_initialize();
        let block = self.free_blocks.pop_back().expect("no allocated memory");

        println!("-- take   block of size {}", block.len());

        block
    }

    pub fn return_block(&mut self, block: Box<[u8]>) {

        println!("-- return block of size {}", block.len());

        self.free_blocks.push_back(block);
    }
}

#[derive(Clone)]
pub struct Memory {
    shared: std::sync::Arc<std::sync::Mutex<ArenaMemoryInstance>>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            shared: std::sync::Arc::new(std::sync::Mutex::new(ArenaMemoryInstance::new()))
        }
    }

    /// Returns cleaned-up memory size
    #[inline(always)]
    pub fn cleanup(&mut self) -> usize {
        self.shared.lock().expect("lock").cleanup()
    }

    #[inline(always)]
    pub fn take_block(&mut self) -> Box<[u8]> {
        self.shared.lock().expect("lock").take_block()
    }

    #[inline(always)]
    pub fn return_block(&mut self, block: Box<[u8]>) {
        self.shared.lock().expect("lock").return_block(block)
    }
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

struct Block {
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

    pub unsafe fn push<T>(&mut self, value: T) -> Option<*mut T> {
        match self.push_copy(&value) {
            None => None,
            Some(ptr) => {
                std::mem::forget(value);
                Some(ptr)
            },
        }
    }

    pub unsafe fn push_copy<T>(&mut self, value: &T) -> Option<*mut T> {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        let align = std::mem::align_of::<T>();
        let padding = (align - (metadata.next_item_offset % align)) % align;
        let aligned = metadata.next_item_offset + padding;
        let end = aligned + std::mem::size_of::<T>();
        if end > self.data.len() {
            None
        } else {
            let target_slice = &mut self.data[aligned..];
            let source_slice = dontdothis::value_as_slice(value);
            for (inbyte, outbyte) in source_slice.iter().zip(target_slice.iter_mut()) {
                *outbyte = *inbyte;
            }
            metadata.next_item_offset = end;
            Some(dontdothis::slice_as_value_ref_mut::<T>(target_slice))
        }
    }

    unsafe fn into_previous_block_and_data(mut self) -> (Option<Block>, Box<[u8]>) {
        let metadata = BlockMetadata::reinterpret_from_slice_mut(&mut *self.data);
        let mut block = None;
        std::mem::swap(&mut block, &mut metadata.previous_block);
        (block, self.data)
    }

    fn remaining_bytes_for_alignment<T>(&self) -> (isize, usize) {
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

struct ArenaMetadata {
    memory: Memory,
    last_block: Option<Block>,
    first_drop_list: *mut DropList,
    last_drop_list: *mut DropList,
    strong_rc: i64,
    rc: i64,
}

impl ArenaMetadata {
    #[inline(always)]
    pub fn inc_rc(&mut self) {
        self.strong_rc += 1;
        self.rc += 1;
        println!("inc_rc s {} t {}", self.strong_rc, self.rc);
    }

    #[inline(always)]
    pub fn dec_rc(&mut self) {
        self.strong_rc -= 1;
        self.rc -= 1;
        println!("dec_rc s {} t {}", self.strong_rc, self.rc);
    }

    #[inline(always)]
    pub fn inc_weak(&mut self) {
        self.rc += 1;
        println!("inc_wk s {} t {}", self.strong_rc, self.rc);
    }

    #[inline(always)]
    pub fn dec_weak(&mut self) {
        self.rc -= 1;
        println!("dec_wk s {} t {}", self.strong_rc, self.rc);
    }

    unsafe fn push_drop_fn<T>(&mut self, data: *const u8) {
        debug_assert_ne!(null_mut(), self.first_drop_list, "push: drop list not null (1)");
        debug_assert_ne!(null_mut(), self.last_drop_list, "push: drop list not null (2)");

        match (*self.last_drop_list).push_drop_fn::<T>(data) {
            DropListWriteResult::ListFull => {
                let next_drop_list = self.upload_no_drop(DropList::empty());
                (*self.last_drop_list).set_next_list(next_drop_list);
                self.last_drop_list = next_drop_list;
                if let DropListWriteResult::ListFull = (*self.last_drop_list).push_drop_fn::<T>(data) {
                    unreachable!("new drop list should be empty");
                }
            },
            DropListWriteResult::ListNotFull => (),
        }
    }

    pub unsafe fn upload_auto_drop<T>(&mut self, value: T) -> *mut T {
        let last_block = self.last_block.as_mut().unwrap();
        if let Some(value_ptr) = last_block.push_copy::<T>(&value) {
            std::mem::forget(value);
            self.push_drop_fn::<T>(value_ptr as *const u8);
            return value_ptr;
        }

        let mut block = Some(Block::new(self.memory.take_block()));
        std::mem::swap(&mut block, &mut self.last_block);
        let last_block = self.last_block.as_mut().unwrap();
        last_block.set_previous_block(block.unwrap());

        let value_ptr = last_block.push_copy::<T>(&value).expect("fits into subsequent block (1)");
        std::mem::forget(value);
        self.push_drop_fn::<T>(value_ptr as *const u8);
        value_ptr
    }

    pub unsafe fn upload_no_drop<T>(&mut self, value: T) -> *mut T {
        let last_block = self.last_block.as_mut().unwrap();
        if let Some(value_ptr) = last_block.push_copy::<T>(&value) {
            std::mem::forget(value);
            return value_ptr;
        }

        let mut block = Some(Block::new(self.memory.take_block()));
        std::mem::swap(&mut block, &mut self.last_block);
        let last_block = self.last_block.as_mut().unwrap();
        last_block.set_previous_block(block.unwrap());

        let value_ptr = last_block.push_copy::<T>(&value).expect("fits into subsequent block (2)");
        std::mem::forget(value);
        value_ptr
    }

    pub unsafe fn upload_no_drop_bytes(&mut self, len: usize, value: impl Iterator<Item=u8>) -> *mut u8 {
        let last_block = self.last_block.as_mut().unwrap();
        let (remaining_bytes_for_alignment, aligned_start) = last_block.remaining_bytes_for_alignment::<[u8; 1]>();
        if remaining_bytes_for_alignment >= len as isize {
            return last_block.upload_bytes_unchecked(aligned_start, len, value);
        }

        let mut block = Some(Block::new(self.memory.take_block()));
        std::mem::swap(&mut block, &mut self.last_block);
        let last_block = self.last_block.as_mut().unwrap();
        last_block.set_previous_block(block.unwrap());

        let (remaining_bytes_for_alignment, aligned_start) = last_block.remaining_bytes_for_alignment::<[u8; 1]>();
        if remaining_bytes_for_alignment >= len as isize {
            return last_block.upload_bytes_unchecked(aligned_start, len, value);
        }

        unreachable!("upload_no_drop_bytes failed after acquiring next block")
    }

    pub unsafe fn drop_objects(&mut self) {
        debug_assert_ne!(null_mut(), self.first_drop_list, "drop_objects: drop list not null");
        (*self.first_drop_list).execute_drop_chain();
        self.first_drop_list = null_mut();
        self.last_drop_list = null_mut();
    }

    /// After the call to this function metadata must not be used
    pub unsafe fn reclaim_memory(&mut self) -> ArenaMetadata {
        let mut block = None;
        std::mem::swap(&mut block, &mut self.last_block);
        while block.is_some() {
            let (previous_block, data) = block.unwrap().into_previous_block_and_data();
            self.memory.return_block(data);
            block = previous_block;
        }

        std::mem::transmute_copy::<ArenaMetadata, ArenaMetadata>(&*self)
    }
}

pub struct WeakArena {
    metadata: *mut ArenaMetadata,
}

pub struct Arena {
    metadata: *mut ArenaMetadata,
}

impl Arena {
    pub fn new(memory: &Memory) -> Arena {
        let mut memory = memory.clone();
        let mut block = Block::new(memory.take_block());
        let drop_list = unsafe { block.push(DropList::empty()) }.expect("first droplist fits");
        let metadata = unsafe { block.push(ArenaMetadata {
            memory,
            last_block: None,
            first_drop_list: drop_list,
            last_drop_list: drop_list,
            strong_rc: 1,
            rc: 1
        }) }.expect("arena metadata fits");
        unsafe { (*metadata).last_block = Some(block) };

        Arena {
            metadata,
        }
    }

    pub fn weak(&self) -> WeakArena {
        println!("split weak arena");
        unsafe { self.md().inc_weak() };
        WeakArena {
            metadata: self.metadata,
        }
    }

    #[inline(always)]
    unsafe fn md(&self) -> &mut ArenaMetadata {
        std::mem::transmute::<*mut ArenaMetadata, &mut ArenaMetadata>(self.metadata)
    }

    #[inline(always)]
    pub unsafe fn upload_auto_drop<T>(&self, value: T) -> *mut T {
        self.md().upload_auto_drop::<T>(value)
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop<T>(&self, value: T) -> *mut T {
        self.md().upload_no_drop::<T>(value)
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop_bytes(&self, len: usize, value: impl Iterator<Item=u8>) -> *mut u8 {
        self.md().upload_no_drop_bytes(len, value)
    }

    pub fn n<T>(&self, value: T) -> N<T> {
        N {
            _arena: self.weak(),
            _ptr: unsafe { self.upload_auto_drop(value) },
        }
    }
}

impl WeakArena {
    #[inline(always)]
    unsafe fn md(&self) -> &mut ArenaMetadata {
        std::mem::transmute::<*mut ArenaMetadata, &mut ArenaMetadata>(self.metadata)
    }

    #[inline(always)]
    pub unsafe fn upload_auto_drop<T>(&self, value: T) -> *mut T {
        self.md().upload_auto_drop::<T>(value)
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop<T>(&self, value: T) -> *mut T {
        self.md().upload_no_drop::<T>(value)
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop_bytes(&self, len: usize, value: impl Iterator<Item=u8>) -> *mut u8 {
        self.md().upload_no_drop_bytes(len, value)
    }
}

impl Clone for Arena {
    fn clone(&self) -> Self {
        println!("clone arena");
        let metadata = self.metadata;
        unsafe { (*metadata).inc_rc(); }
        Arena {
            metadata,
        }
    }
}

impl Clone for WeakArena {
    fn clone(&self) -> Self {
        println!("clone weak");
        let metadata = self.metadata;
        unsafe { (*metadata).inc_weak(); }
        WeakArena {
            metadata,
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        println!("drop arena");

        let metadata = unsafe { self.md() };
        (*metadata).dec_rc();

        if (*metadata).strong_rc == 0 {
            println!("drop arena objects");
            unsafe { (*metadata).drop_objects() };
        }

        if (*metadata).rc == 0 {
            println!("reclaim memory");
            unsafe { metadata.reclaim_memory() };
            // this should be the last use of this metadata
        }
    }
}

impl Drop for WeakArena {
    fn drop(&mut self) {
        println!("drop weak");

        let metadata = unsafe { self.md() };
        (*metadata).dec_weak();

        if (*metadata).rc == 0 {
            println!("reclaim memory");
            unsafe { metadata.reclaim_memory() };
            // this should be the last use of this metadata
        }
    }
}

pub struct N<T> {
    _arena: WeakArena,
    _ptr: *mut T,
}

#[cfg(test)]
mod arena_tests {
    use crate::{Memory, Arena};

    struct Compact {
        value: u8,
    }

    impl Drop for Compact {
        fn drop(&mut self) {
            println!("drop {}", self.value);
        }
    }

    #[test]
    fn test() {
        let _obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem);
            arena.n(Compact { value: 5 })
        };
    }
}