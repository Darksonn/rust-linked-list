use std::ptr;
use std::mem;

pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    allocations: Vec<(*mut LinkedNode<T>, usize)>,
    unused_nodes: *mut LinkedNode<T>
}

struct LinkedNode<T> {
    next: *mut LinkedNode<T>,
    prev: *mut LinkedNode<T>,
    value: T
}

impl<T> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            allocations: Vec::new(),
            unused_nodes: ptr::null_mut()
        }
    }
    pub fn push(&mut self, value: T) {
        let tail = self.tail;
        let node = self.new_node(ptr::null_mut(), tail, value);

        if self.head.is_null() {
            self.head = node;
        }
        if !self.tail.is_null() {
            unsafe { (*self.tail).next = node; }
        }

        self.tail = node;
        self.len += 1;
    }
    pub fn push_front(&mut self, value: T) {
        let head = self.head;
        let node = self.new_node(head, ptr::null_mut(), value);

        if self.tail.is_null() {
            self.tail = node;
        }
        if !self.head.is_null() {
            unsafe { (*self.head).prev = node; }
        }

        self.head = node;
        self.len += 1;
    }
    pub fn peek(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                Some(&(*self.tail).value)
            }
        }
    }
    pub fn get(&self, i: usize) -> Option<&T> {
        self.move_right(self.head, i).map(|ptr| unsafe { &(*ptr).value })
    }
    pub fn remove(&mut self, i: usize) -> T {
        match self.move_right(self.head, i) {
            None => {
                panic!("Remove out of bounds {} with len {}", i, self.len());
            },
            Some(node) => unsafe {
                let value = ptr::read(&(*node).value as *const T);

                if !(*node).prev.is_null() {
                    (*(*node).prev).next = (*node).next;
                }
                if !(*node).next.is_null() {
                    (*(*node).next).prev = (*node).prev;
                }

                if node == self.head {
                    self.head = (*node).next;
                }
                if node == self.tail {
                    self.tail = (*node).prev;
                }

                self.discard_node(node);

                value
            }
        }
    }


    fn move_right(&self, mut ptr: *mut LinkedNode<T>, dist: usize) -> Option<*mut LinkedNode<T>> {
        for _ in 0..dist {
            if ptr.is_null() {
                return None;
            }
            unsafe { ptr = (*ptr).next };
        }
        if ptr.is_null() {
            return None;
        }
        Some(ptr)
    }

    pub fn len(&self) -> usize {
        self.len
    }
    fn discard_node(&mut self, node: *mut LinkedNode<T>) {
        unsafe {
            ptr::write(&mut (*node).next, self.unused_nodes);
        }
        self.unused_nodes = node;
    }
    fn new_node(
        &mut self,
        next: *mut LinkedNode<T>,
        prev: *mut LinkedNode<T>,
        value: T
    ) -> *mut LinkedNode<T> {
        unsafe {
            if self.unused_nodes.is_null() {
                self.allocate(64);
            }
            let node = self.unused_nodes;
            self.unused_nodes = (*node).next;

            ptr::write(node, LinkedNode {
                next: next,
                prev: prev,
                value: value
            });
            node
        }
    }

    // allocates a lot of linked nodes
    fn allocate(&mut self, amount: usize) {
        assert!(amount > 0);
        let mut vec = Vec::with_capacity(amount);
        let mut ptr = vec.as_mut_ptr();
        let capacity = vec.capacity();

        mem::forget(vec);

        self.allocations.push((ptr, capacity));

        for _ in 0..capacity {

            unsafe {
                ptr::write(&mut (*ptr).next, self.unused_nodes);
            }
            self.unused_nodes = ptr;

            ptr = unsafe { ptr.offset(1) };
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        unsafe {
            let mut ptr = self.head;
            while !ptr.is_null() {
                ptr::drop_in_place(&mut (*ptr).value as *mut T);
                ptr = (*ptr).next;
            }

            for &(vecptr, capacity) in &self.allocations {
                let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                drop(vec);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get() {

        let mut to_drop = 0;

        struct DropTest {
            value: usize,
            to_drop: *mut usize
        }
        impl DropTest {
            fn new(val: usize, to_drop: &mut usize) -> DropTest {
                *to_drop += 1;
                DropTest {
                    value: val,
                    to_drop: to_drop as *mut usize
                }
            }
        }
        impl Drop for DropTest {
            fn drop(&mut self) {
                unsafe {
                    (*self.to_drop) -= 1;
                }
            }
        }

        let mut list = LinkedList::new();
        for i in 64..128 {
            list.push(DropTest::new(i, &mut to_drop));
        }
        list.push_front(DropTest::new(3000, &mut to_drop));
        for i in (0..64).rev() {
            list.push_front(DropTest::new(i, &mut to_drop));
        }
        assert_eq!(to_drop, 129);
        list.remove(64);
        assert_eq!(to_drop, 128);
        for i in 0..128 {
            assert_eq!(Some(i), list.get(i).map(|dt| dt.value));
        }
        drop(list);
        assert_eq!(to_drop, 0);
    }
}
