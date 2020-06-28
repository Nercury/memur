use std::collections::VecDeque;

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

/// Container of shared memory blocks.
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