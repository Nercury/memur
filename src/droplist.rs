/**!

This whole module might look not so bad, because it looks like a decent wrapper,
but it is incredibly unsafe!

Keep in mind that if you have a raw pointer to some data, you must ensure
this data DOES NOT MOVE IN MEMORY FOR THE LIFETIME OF THE USER.

Droplists can point to each other, that means THEY MUST NOT MOVE IN MEMORY.
Droplist item points to raw struct data, and it also MUST NOT MOVE IN MEMORY.
PRO TIP: pushing to a Vec WILL overwrite the memory. Don't store struct's data in a Vec!!!! Use
Vec to initialize data and convert it to a fixed memory chunk with `.into_boxed_slice()`!

What do droplists do? They help to deallocate huge amount of objects in an arena FAST.
You would usually store the droplist itself in the arena (as bytes), followed by contents
of the objects it can drop. So that when you execute the droplist, all the memory it drops
is nearby.

*/

const MAX_DROP_LIST_ITEMS: usize = 1022;

pub struct DropList {
    items: [Option<DropItem>; MAX_DROP_LIST_ITEMS],
    next_list: Option<*mut DropList>,
    used_items: u16,
}

impl DropList {
    #[inline(always)]
    pub fn empty() -> DropList {
        DropList {
            items: [None; MAX_DROP_LIST_ITEMS],
            next_list: None,
            used_items: 0,
        }
    }

    unsafe fn write_item(&mut self, item: DropItem) -> DropListWriteResult {
        self.items[self.used_items as usize] = Some(item);
        self.used_items += 1;
        if self.used_items as usize == MAX_DROP_LIST_ITEMS {
            DropListWriteResult::ListFull
        } else {
            DropListWriteResult::ListNotFull
        }
    }

    pub unsafe fn push_drop_fn<T>(&mut self, data: *const u8) -> DropListWriteResult {
        let drop_item = DropItem {
            fun: drop::<T>,
            data,
        };
        self.write_item(drop_item)
    }

    #[inline(always)]
    pub unsafe fn set_next_list(&mut self, list: *mut DropList) {
        self.next_list = Some(list)
    }

    /// Executes this drop list and also all lists linked to it.
    /// Destroys the data contained in the drop list and removes links, so that executing it again is a no-op.
    pub unsafe fn execute_drop_chain(&mut self) {
        let mut maybe_head = Some(self);
        while let Some(list) = maybe_head {
            for i in list.items.iter_mut() {
                if let Some(drop_item) = i {
                    drop_item.execute();
                } else {
                    return;
                }
                *i = None;
            }
            maybe_head = list.next_list
                .map(|ptr| std::mem::transmute::<*mut DropList, &mut DropList>(ptr));
            list.next_list = None;
        }
    }
}

pub enum DropListWriteResult {
    ListFull,
    ListNotFull,
}

#[derive(Copy, Clone)]
pub struct DropItem {
    pub fun: DropFn,
    pub data: *const u8,
}

impl DropItem {
    #[inline(always)]
    pub unsafe fn execute(&self) {
        (self.fun)(self.data);
    }
}

pub type DropFn = unsafe fn(*const u8) -> ();

#[inline(always)]
pub unsafe fn drop<T: Sized>(bytes: *const u8) {
    let ref_to_t = std::mem::transmute::<*const u8, &T>(bytes);
    std::mem::transmute_copy::<T, T>(ref_to_t);
}

#[cfg(test)]
mod tests {
    use crate::droplist::{DropList, DropListWriteResult};
    use crate::dontdothis;
    use crate::dropflag::{DropFlag, DropableWithData};
    use std::cell::RefCell;

    #[test]
    fn droplist() {
        let mut list = DropList::empty();
        let flag1 = DropFlag::new(RefCell::new(0));
        let droppable1 = DropableWithData { data: 42, dropflag: flag1.clone() };
        let mut bytes = vec![0u8; std::mem::size_of::<DropableWithData>()];

        // copy object contents to byte storage and forget it
        std::io::copy(
            &mut unsafe { dontdothis::value_as_slice(&droppable1) },
            &mut std::io::Cursor::new(&mut bytes)).unwrap();
        std::mem::forget(droppable1);

        // add drop fn to list with a pointer to a location where the struct data is now
        unsafe { list.push_drop_fn::<DropableWithData>(bytes.as_ptr()); }

        assert_eq!(0, *flag1.borrow());
        unsafe { list.execute_drop_chain() };
        assert_eq!(42, *flag1.borrow());

        // calling twice is no-op
        unsafe { list.execute_drop_chain() };
        assert_eq!(42, *flag1.borrow());
    }

    #[test]
    fn droplist_chain() {
        // two droplists
        let mut list1 = DropList::empty();
        let mut list2 = DropList::empty();

        // will create a number of items that do not fit into a single drop list

        let flags = (0..super::MAX_DROP_LIST_ITEMS+1)
            .map(|_| DropFlag::new(RefCell::new(0)))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let droppables: Vec<_> = flags.iter()
            .map(|flag| DropableWithData { data: 42, dropflag: flag.clone() })
            .collect();

        // will convert all objects to bytes and store those bytes in a continuous byte block

        let mut bytes = vec![0u8; std::mem::size_of::<DropableWithData>() * (super::MAX_DROP_LIST_ITEMS*2)]
            .into_boxed_slice();
        let mut copy_target_slice = &mut bytes[..];

        let mut first_full = false; // first droplist is full, use the second one

        for droppable in droppables {

            // copy bytes to byte block
            std::io::copy(
                &mut unsafe { dontdothis::value_as_slice(&droppable) },
                &mut copy_target_slice).unwrap();
            std::mem::forget(droppable);

            unsafe {
                // copying moves pointer forward, so calculate obj start by using a negative offset
                let location_of_struct_start = copy_target_slice.as_ptr().offset(-(std::mem::size_of::<DropableWithData>() as isize));

                if !first_full {
                    match list1.push_drop_fn::<DropableWithData>(location_of_struct_start) {
                        DropListWriteResult::ListFull => {
                            // if the first droplist is full, set the next droplist and push to the next from now on

                            {
                                first_full = true;
                                list1.set_next_list((&mut list2) as *mut DropList);
                            }
                            if let DropListWriteResult::ListFull = list2.push_drop_fn::<DropableWithData>(copy_target_slice.as_ptr()) {
                                panic!("second list full");
                            }
                        },
                        DropListWriteResult::ListNotFull => (),
                    }
                } else {
                    match list2.push_drop_fn::<DropableWithData>(location_of_struct_start) {
                        DropListWriteResult::ListFull => {
                            panic!("second list full");
                        },
                        DropListWriteResult::ListNotFull => (),
                    }
                }
            }
        }

        // check if all flags are at starting value
        for flag in flags.iter() {
            assert_eq!(0, *flag.borrow());
        }

        // execute first droplist, which should execute drop for all chain
        unsafe { list1.execute_drop_chain() };

        // check if drop was executed for all items by checking the flag
        for flag in flags.iter() {
            assert_eq!(42, *flag.borrow());
        }
    }
}