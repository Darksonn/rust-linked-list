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
    pub fn with_capacity(cap: usize) -> LinkedList<T> {
        let mut list = LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            allocations: Vec::with_capacity(1),
            unused_nodes: ptr::null_mut()
        };
        list.allocate(cap);
        list
    }
    pub fn push_back(&mut self, value: T) {
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
    pub fn back(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                Some(&(*self.tail).value)
            }
        }
    }
    pub fn front(&self) -> Option<&T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                Some(&(*self.head).value)
            }
        }
    }
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                Some(&mut (*self.tail).value)
            }
        }
    }
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                Some(&mut (*self.head).value)
            }
        }
    }
    pub fn pop_back(&mut self) -> Option<T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                let tail = self.tail;
                self.tail = (*tail).prev;

                if self.head == tail {
                    self.head = ptr::null_mut();
                }

                let value = ptr::read(&(*tail).value);
                self.discard_node(tail);
                Some(value)
            }
        }
    }
    pub fn pop_front(&mut self) -> Option<T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                let head = self.head;
                self.head = (*head).prev;

                if self.tail == head {
                    self.tail = ptr::null_mut();
                }

                let value = ptr::read(&(*head).value);
                self.discard_node(head);
                Some(value)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    fn discard_node(&mut self, node: *mut LinkedNode<T>) {
        unsafe {
            (*node).next = self.unused_nodes;
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
        if amount == 0 { return; }
        let mut vec = Vec::with_capacity(amount);
        let base = vec.as_mut_ptr();
        let capacity = vec.capacity();

        mem::forget(vec);

        self.allocations.push((base, capacity));

        for i in 0..capacity {
            let ptr = unsafe { base.add(i) };

            unsafe {
                (*ptr).next = self.unused_nodes;
            }
            self.unused_nodes = ptr;
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        unsafe {
            let mut ptr = self.head;
            while !ptr.is_null() {
                ptr::drop_in_place(&mut (*ptr).value);
                ptr = (*ptr).next;
            }

            for &(vecptr, capacity) in &self.allocations {
                let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                drop(vec);
            }
        }
    }
}

