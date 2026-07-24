use std::cell::RefCell;

use std::rc::Rc;

pub(crate) struct ConnectionGuard {
    pub(crate) counter: Rc<RefCell<usize>>,
}

impl ConnectionGuard {
    pub(crate) fn new(counter: Rc<RefCell<usize>>) -> Self {
        *counter.borrow_mut() += 1;
        Self { counter }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        *self.counter.borrow_mut() -= 1;
    }
}
