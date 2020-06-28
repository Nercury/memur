//! This module is for testing only

use std::rc::Rc;
use std::cell::RefCell;

pub type DropFlag<T> = Rc<RefCell<T>>;

pub struct Droppable {
    pub dropflag: DropFlag<bool>,
}

impl Drop for Droppable {
    fn drop(&mut self) {
        *self.dropflag.borrow_mut() = true;
    }
}

pub struct DropableWithData {
    pub data: i32,
    pub dropflag: DropFlag<i32>,
}

impl Drop for DropableWithData {
    fn drop(&mut self) {
        let ret = self.data;
        self.data += 1;
        *self.dropflag.borrow_mut() = ret;
    }
}

#[test]
fn dropflag() {
    let flag = DropFlag::new(RefCell::new(false));
    let droppable = Droppable { dropflag: flag.clone() };
    assert_eq!(false, *flag.borrow());
    std::mem::drop(droppable);
    assert_eq!(true, *flag.borrow());
}
