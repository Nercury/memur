use crate::Memory;
use crate::droplist::{DropList, DropListWriteResult};
use std::ptr::null_mut;
use crate::block::Block;

/// Information about arena injected in first allocated arena memory block.
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

/// A weak `Arena` reference that holds a pointer to valid memory until dropped.
///
/// As long as the original strong `Arena` is alive, this reference can be upgraded to `Arena`.
/// If the `Arena` is no longer alive, the memory can no longer be mutated, and the drop
/// functions for all root objects in the `Arena` are already executed.
///
/// However, if your structure does not need to be dropped (i.e. bunch of bytes), the bytes
/// can be safely accessed as long as you hold the `WeakArena` reference, even if `is_alive` returns `false`.
///
/// The `WeakArena`, like `Arena`, can not be shared between threads.
pub struct WeakArena {
    metadata: *mut ArenaMetadata,
}

/// `Arena` is a memory block container that executes `drop` for your objects when it goes out of scope.
///
/// It can not be used between threads. Create a separate `WeakArena` for each thread.
///
/// You can use this `Arena` to upload data to memory and receive a pointer to this data.
/// Behind the scenes, the `Arena` will bump the pointer inside the current block, copy
/// your structure into that location, and return this pointer back. If the `Arena` runs out of blocks,
/// it will request new blocks from the main `Memory`.
///
/// You should keep a `WeakArena` reference together with your pointer, and check if it `is_alive`
/// before every pointer access. As long as you have a copy of `WeakArena`, the memory won't be
/// deallocated, however if the `Arena` is dropped and you have uploaded the structure with `_auto_drop`
/// function, the structure is dropped as well and should not be used.
///
/// `Arena` and `WeakArena` are not intended to be used directly, instead, you should create
/// wrappers around them for your structures. Inspect `List`, `UStr` or `N` structures for the example code.
/// You will find that these structures do not keep `Arena` inside, because doing that would produce
/// scenario where the `Arena` is never dropped when the structures are nested.
///
/// When all `WeakArena` and `Arena` instances are gone, the memory blocks are returned back to `Memory`.
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

    pub fn to_weak_arena(&self) -> WeakArena {
        println!("split weak arena");
        unsafe { self.md().inc_weak() };
        WeakArena {
            metadata: self.metadata,
        }
    }
}

impl WeakArena {
    /// Returns true if drop functions for the arena structures were not yet executed (the `Arena` is not dropped).
    #[inline(always)]
    pub fn is_alive(&self) -> bool {
        unsafe { self.md().strong_rc > 0 }
    }

    #[inline(always)]
    unsafe fn md(&self) -> &mut ArenaMetadata {
        std::mem::transmute::<*mut ArenaMetadata, &mut ArenaMetadata>(self.metadata)
    }

    #[inline(always)]
    pub unsafe fn upload_auto_drop<T>(&self, value: T) -> Option<*mut T> {
        if self.is_alive() {
            Some(self.md().upload_auto_drop::<T>(value))
        } else {
            None
        }
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop<T>(&self, value: T) -> Option<*mut T> {
        if self.is_alive() {
            Some(self.md().upload_no_drop::<T>(value))
        } else {
            None
        }
    }

    #[inline(always)]
    pub unsafe fn upload_no_drop_bytes(&self, len: usize, value: impl Iterator<Item=u8>) -> Option<*mut u8> {
        if self.is_alive() {
            Some(self.md().upload_no_drop_bytes(len, value))
        } else {
            None
        }
    }

    /// Try to upgrade `WeakArena` to `Arena`.
    pub fn arena(&self) -> Option<Arena> {
        if self.is_alive() {
            println!("upgrade weak to strong arena");
            unsafe { self.md().inc_rc() };
            Some(Arena {
                metadata: self.metadata,
            })
        } else {
            None
        }
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

#[cfg(test)]
mod arena_tests {
    use crate::{Memory, Arena, N};
    use crate::dropflag::DropFlag;
    use std::cell::RefCell;

    #[derive(Debug)]
    struct Compact {
        value: DropFlag<i32>,
    }

    impl PartialEq for Compact {
        fn eq(&self, other: &Self) -> bool {
            ((*self.value).borrow()).eq(&(*other.value).borrow())
        }
    }

    impl Eq for Compact {}

    impl Drop for Compact {
        fn drop(&mut self) {
            *self.value.borrow_mut() -= 1;
        }
    }

    #[allow(dead_code)]
    struct Nested<T> {
        dropflag: DropFlag<i32>,
        inner: T,
    }

    impl<T> Drop for Nested<T> {
        fn drop(&mut self) {
            *self.dropflag.borrow_mut() -= 1;
        }
    }

    #[test]
    fn value_can_not_be_used_when_arena_goes_out_of_scope() {
        let flag = DropFlag::new(RefCell::new(1));
        let mut obj = {
            let mem = Memory::new();
            let arena = Arena::new(&mem);
            let mut obj = N::new(&arena, Compact { value: flag.clone() });

            assert_eq!(1, *(*flag).borrow(), "drop was not called");
            assert_ne!(None, obj.val(), "value can be accessed");
            assert_ne!(None, obj.var(), "value can be accessed");

            obj
        };

        assert_eq!(0, *(*flag).borrow(), "drop was called");
        assert_eq!(None, obj.val(), "value can not be accessed");
        assert_eq!(None, obj.var(), "value can not be accessed");
    }

    #[test]
    fn nested_objects_are_dropped_properly() {
        let f1 = DropFlag::new(RefCell::new(1));
        let f2 = DropFlag::new(RefCell::new(1));

        let mem = Memory::new();
        let _obj = {
            let arena = Arena::new(&mem);
            let obj = N::new(&arena,
                             Nested {
                                 dropflag: f1.clone(),
                                 inner: N::new(&arena,
                                               Nested {
                                                   dropflag: f2.clone(),
                                                   inner: ()
                                               }
                                 )
                             }
            );

            assert_eq!(1, *(*f1).borrow(), "drop was not called");
            assert_eq!(1, *(*f2).borrow(), "drop was not called");
            obj
        };

        assert_eq!(0, *(*f1).borrow(), "drop was called");
        assert_eq!(0, *(*f2).borrow(), "drop was called");
    }
}