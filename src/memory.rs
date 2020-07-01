use std::collections::VecDeque;

struct ArenaMemoryInstance {
    free_blocks: VecDeque<Box<[u8]>>,
    max_free_blocks_to_initialize_or_cleanup_to: i32,
    min_free_blocks_before_allocating_new: i32,
    new_block_size: usize,
}

impl ArenaMemoryInstance {
    pub fn new(options: &MemoryBuilder) -> ArenaMemoryInstance {
        let mut i = ArenaMemoryInstance {
            free_blocks: VecDeque::with_capacity(1024),
            max_free_blocks_to_initialize_or_cleanup_to: options.max_free_blocks_to_initialize_or_cleanup_to,
            min_free_blocks_before_allocating_new: options.min_free_blocks_before_allocating_new,
            new_block_size: options.new_block_size,
        };
        i.check_if_not_enough_blocks_and_initialize();
        i
    }

    fn check_if_not_enough_blocks_and_initialize(&mut self) {
        if self.free_blocks.len() < self.min_free_blocks_before_allocating_new as usize {
            while self.free_blocks.len() < self.max_free_blocks_to_initialize_or_cleanup_to as usize {
                //println!("-- init   block of size {}", self.new_block_size);
                self.free_blocks.push_front(vec![0u8; self.new_block_size as usize].into_boxed_slice());
            }
        }
    }

    /// Cleans up the memory and returns cleaned-up memory size.
    pub fn cleanup(&mut self) -> usize {
        let mut cleaned_up_size = 0;
        while self.free_blocks.len() > self.max_free_blocks_to_initialize_or_cleanup_to as usize {
            if let Some(block) = self.free_blocks.pop_front() {
                cleaned_up_size += block.len();
                //println!("-- clean  block of size {}", block.len());
            }
        }
        cleaned_up_size
    }

    pub fn take_block(&mut self) -> Box<[u8]> {
        self.check_if_not_enough_blocks_and_initialize();
        let block = self.free_blocks.pop_back().expect("no allocated memory");

        //println!("-- take   block of size {}", block.len());

        block
    }

    pub fn return_block(&mut self, block: Box<[u8]>) {

        //println!("-- return block of size {}", block.len());

        self.free_blocks.push_back(block);
    }
}

/// Memory options builder.
pub struct MemoryBuilder {
    max_free_blocks_to_initialize_or_cleanup_to: i32,
    min_free_blocks_before_allocating_new: i32,
    new_block_size: usize,
}

impl MemoryBuilder {
    /// Specify the amount of blocks to keep around.
    ///
    /// Memory immediately allocates the `max` blocks when created.
    ///
    /// If the amount of unused allocated blocks reaches `min`, new block allocation kicks in
    /// and allocates up to `max` blocks again.
    ///
    /// Memory blocks returned back to memory can increase count above `max`, because blocks are
    /// not deallocated automatically. Use `cleanup` function for that.
    pub fn with_min_max_blocks(mut self, min: i32, max: i32) -> MemoryBuilder {
        self.min_free_blocks_before_allocating_new = min;
        self.max_free_blocks_to_initialize_or_cleanup_to = max;
        self
    }

    /// Specify the size of a new block.
    ///
    /// Make sure it is considerably bigger than any structures you want to keep in it.
    pub fn with_block_size(mut self, size: usize) -> MemoryBuilder {
        self.new_block_size = size;
        self
    }

    pub fn build(self) -> Memory {
        Memory {
            shared: std::sync::Arc::new(std::sync::Mutex::new(ArenaMemoryInstance::new(&self)))
        }
    }
}

/// Container of shared memory blocks.
/// Does not automatically de-allocate memory!
/// Call `cleanup` method to de-allocate when it is the most convenient.
#[derive(Clone)]
pub struct Memory {
    shared: std::sync::Arc<std::sync::Mutex<ArenaMemoryInstance>>,
}

impl Memory {
    pub fn builder() -> MemoryBuilder {
        MemoryBuilder {
            max_free_blocks_to_initialize_or_cleanup_to: 4,
            min_free_blocks_before_allocating_new: 2,
            new_block_size: 1024*64,
        }
    }

    pub fn new() -> Memory {
        let builder = MemoryBuilder {
            max_free_blocks_to_initialize_or_cleanup_to: 4,
            min_free_blocks_before_allocating_new: 2,
            new_block_size: 1024*64,
        };

        Memory {
            shared: std::sync::Arc::new(std::sync::Mutex::new(ArenaMemoryInstance::new(&builder)))
        }
    }

    /// Cleans up the memory and returns cleaned-up memory size if the amount of free blocks is
    /// above `max`.
    #[inline(always)]
    pub fn cleanup(&self) -> usize {
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