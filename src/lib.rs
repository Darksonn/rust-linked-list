use std::ptr;
use std::mem;

pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    capacity: usize,
    chunk_size: usize,
    allocations: Vec<(*mut LinkedNode<T>, usize)>,
    unused_nodes: *mut LinkedNode<T>
}

struct LinkedNode<T> {
    next: *mut LinkedNode<T>,
    prev: *mut LinkedNode<T>,
    value: T
}

impl<T> LinkedList<T> {
    /// Create a new LinkedList with a chunk size of 64.
    pub fn new() -> LinkedList<T> {
        LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            capacity: 0,
            chunk_size: 64,
            allocations: Vec::new(),
            unused_nodes: ptr::null_mut()
        }
    }
    /// Create a new LinkedList with a chunk size of 64 and the specified capacity.
    pub fn with_capacity(cap: usize) -> LinkedList<T> {
        let mut list = LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            capacity: 0,
            chunk_size: 64,
            allocations: Vec::with_capacity(1),
            unused_nodes: ptr::null_mut()
        };
        list.allocate(cap);
        list
    }

    /// Add the element to the back of the linked list.
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
    /// Add the element to the front of the linked list.
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
    /// Returns the last element in the list, or none if it is empty.
    pub fn back(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                Some(&(*self.tail).value)
            }
        }
    }
    /// Returns the first element in the list, or none if it is empty.
    pub fn front(&self) -> Option<&T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                Some(&(*self.head).value)
            }
        }
    }
    /// Returns the last element in the list, or none if it is empty.
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                Some(&mut (*self.tail).value)
            }
        }
    }
    /// Returns the first element in the list, or none if it is empty.
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                Some(&mut (*self.head).value)
            }
        }
    }
    /// Remove the last element in the list and return it, or none if it's empty.
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
    /// Remove the first element in the list and return it, or none if it's empty.
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

    /// Clears the linked list, but does not deallocate any memory.
    pub fn clear(&mut self) {
        let mut ptr = self.head;
        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;
        while !ptr.is_null() {
            unsafe {
                ptr::drop_in_place(&mut (*ptr).value);
                self.discard_node(ptr);
                ptr = (*ptr).next;
            }
        }
    }
    /// Clears the linked list, and deallocates all memory.
    pub fn clear_and_deallocate(&mut self) {
        let chunk_size = self.chunk_size;
        *self = LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            capacity: 0,
            chunk_size: chunk_size,
            allocations: Vec::new(),
            unused_nodes: ptr::null_mut()
        };
    }

    /// Returns the capacity of the linked list.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Returns the number of items in the linked list.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns the number of items allocated when more memory is needed.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set the number of items allocated when more memory is needed.
    pub fn set_chunk_size(&mut self, size: usize) {
        assert!(size > 0);
        self.chunk_size = size;
    }
    /// Allocate enough memory for the linked list to contain at least `amount` memory, but
    /// may allocate up to `chunk_size` if `amount` is less than `chunk_size`.
    pub fn reserve(&mut self, amount: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= amount { return; }
        let to_allocate = amount - free_capacity;

        let chunk_size = self.chunk_size;
        if to_allocate < chunk_size {
            self.allocate(chunk_size);
        } else {
            self.allocate(to_allocate);
        }
    }
    /// If the linked list does not have space for another `amount` nodes, then allocate
    /// exactly enough memory for that many nodes.
    pub fn reserve_exact(&mut self, amount: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= amount { return; }
        let to_allocate = amount - free_capacity;
        self.allocate(to_allocate);
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
                let chunk_size = self.chunk_size;
                self.allocate(chunk_size);
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

    fn allocate(&mut self, amount: usize) {
        if amount == 0 { return; }
        let mut vec = Vec::with_capacity(amount);
        let base = vec.as_mut_ptr();
        let capacity = vec.capacity();
        self.capacity += capacity;

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

