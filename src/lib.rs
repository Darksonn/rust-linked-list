use std::ptr;
use std::mem;

pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    dealloc_offset: isize,
    unused_nodes: Vec<*mut LinkedNode<T>>
}

enum DeallocInfo<T> {
    NoDealloc(),
    Dealloc(*mut LinkedNode<T>, usize)
}
struct LinkedNode<T> {
    next: *mut LinkedNode<T>,
    prev: *mut LinkedNode<T>,
    dealloc: DeallocInfo<T>,
    value: T
}

impl<T> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        if mem::size_of::<T>() == 0 { panic!("LinkedList with zero sized type"); }
        LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            dealloc_offset: 0,
            len: 0,
            unused_nodes: Vec::with_capacity(64)
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

                self.unused_nodes.push(node);

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
    fn new_node(&mut self, next: *mut LinkedNode<T>, prev: *mut LinkedNode<T>, value: T) -> *mut LinkedNode<T> {
        match self.unused_nodes.pop() {
            Some(node) => unsafe {
                let node_dealloc = ptr::read(self.dealloc_ptr(node));
                ptr::write(node, LinkedNode {
                    next: next,
                    prev: prev,
                    dealloc: node_dealloc,
                    value: value
                });
                node
            },
            None => {
                self.allocate(64, next, prev, value)
            }
        }
    }

    fn dealloc_ptr(&self, ptr: *mut LinkedNode<T>) -> *mut DeallocInfo<T> {
        debug_assert!(self.dealloc_offset > 0);
        ((ptr as isize) + self.dealloc_offset) as *mut DeallocInfo<T>
    }

    // allocates a lot of linked nodes and returns the first
    fn allocate(&mut self, amount: usize, next: *mut LinkedNode<T>, prev: *mut LinkedNode<T>, value: T) -> *mut LinkedNode<T> {
        assert!(amount > 0);
        let mut vec = Vec::with_capacity(amount);
        let ptr = vec.as_mut_ptr();
        let capacity = vec.capacity();

        let first = LinkedNode {
            next: next,
            prev: prev,
            dealloc: DeallocInfo::Dealloc(ptr, capacity),
            value: value
        };
        mem::forget(vec);
        unsafe { ptr::write(ptr, first); }

        let dealloc_offset = unsafe { (&mut (*ptr).dealloc as *mut DeallocInfo<T> as isize) - (ptr as isize) };
        self.dealloc_offset = dealloc_offset;

        let mut nptr = unsafe { ptr.offset(1) };
        self.unused_nodes.reserve(capacity - 1);
        for _ in 1..capacity {
            let dptr = self.dealloc_ptr(nptr);
            unsafe { ptr::write(dptr, DeallocInfo::NoDealloc()); }
            self.unused_nodes.push(nptr);
            nptr = unsafe { nptr.offset(1) };
        }

        ptr
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        use DeallocInfo::*;
        unsafe {
            for i in (0..self.unused_nodes.len()).rev() {
                let node = self.unused_nodes[i];
                let node_dealloc = ptr::read(self.dealloc_ptr(node));
                match node_dealloc {
                    NoDealloc() => {
                        self.unused_nodes.swap_remove(i);
                    },
                    _ => {}
                }
            }

            let mut ptr = self.head;
            while !ptr.is_null() {
                match &(*ptr).dealloc {
                    Dealloc(_, _) => {
                        self.unused_nodes.push(ptr);
                    },
                    _ => {}
                }
                ptr::drop_in_place(&mut (*ptr).value as *mut T);
                ptr = (*ptr).next;
            }

            for &ptr in &self.unused_nodes {
                let node_dealloc = ptr::read(self.dealloc_ptr(ptr));
                match node_dealloc {
                    NoDealloc() => {
                        panic!("We removed all NoDealloc, but we still have one");
                    },
                    Dealloc(vecptr, capacity) => {
                        let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                        drop(vec);
                    }
                }
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
