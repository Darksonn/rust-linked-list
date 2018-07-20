use std::ptr;
use std::mem;
use std::fmt;
use std::iter::{
    FromIterator,
    IntoIterator,
    Extend
};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub mod iter;
use iter::{Iter, IterMut, IntoIter};

#[cfg(test)]
extern crate rand;

pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    capacity: usize,
    chunk_size: usize,
    allocations: Vec<(*mut LinkedNode<T>, usize)>,
    unused_nodes: *mut LinkedNode<T>
}

// LinkedLists own their data, so the borrow checker should prevent data races.
unsafe impl<T: Send> Send for LinkedList<T> {}
unsafe impl<T: Sync> Sync for LinkedList<T> {}

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

    /// Go through the list, calling `f` on each element, replacing the element with the
    /// return value of `f`, or removing it if `f` returns `None`.
    pub fn retain_map(&mut self, mut f: impl FnMut(T) -> Option<T>) {
        if self.is_empty() { return; }
        let mut ptr = self.head;
        let mut last_retain: *mut LinkedNode<T> = ptr::null_mut();

        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;

        let mut new_head = ptr::null_mut();
        let mut retained = 0;

        unsafe {
            while !ptr.is_null() {
                let value_ptr = &mut (*ptr).value as *mut T;
                let next_ptr = (*ptr).next;
                match f(ptr::read(value_ptr)) {
                    Some(new_value) => {
                        ptr::write(value_ptr, new_value);
                        if last_retain.is_null() {
                            new_head = ptr;
                        } else {
                            (*last_retain).next = ptr;
                        }
                        (*ptr).prev = last_retain;
                        last_retain = ptr;
                        retained += 1;
                    },
                    None => {
                        self.discard_node(ptr);
                    }
                }
                ptr = next_ptr;
            }
        }

        self.head = new_head;
        self.tail = last_retain;
        self.len = retained;

    }
    /// Go through the list, calling `f` on each element, which may mutate the element,
    /// then removes it if `f` returns `false`.
    pub fn retain_mut(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        self.retain_map(|mut val| {
            if f(&mut val) {
                Some(val)
            } else {
                None
            }
        });
    }
    /// Go through the list, calling `f` on each element, and removes it if `f` returns
    /// `false`.
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.retain_map(|val| {
            if f(&val) {
                Some(val)
            } else {
                None
            }
        });
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        Iter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData
        }
    }
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, T> {
        IterMut {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData
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
                self.drop_node(ptr);
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
    fn drop_node(&mut self, node: *mut LinkedNode<T>) {
        unsafe {
            ptr::drop_in_place(&mut (*node).value);
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

        // add them to the unused_nodes list in reverse order, so they end up in the
        // correct order if lots of elements are added with push_back
        for i in (0..capacity).rev() {
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
impl<T> Default for LinkedList<T> {
    fn default() -> LinkedList<T> {
        LinkedList::new()
    }
}
impl<T: Clone> Clone for LinkedList<T> {
    fn clone(&self) -> LinkedList<T> {
        let mut list = LinkedList::with_capacity(self.len());
        for item in self.iter() {
            list.push_back(item.clone());
        }
        list
    }
    fn clone_from(&mut self, source: &Self) {
        self.clear();
        self.reserve_exact(source.len());
        for item in source.iter() {
            self.push_back(item.clone());
        }
    }
}
impl<T> FromIterator<T> for LinkedList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut list = LinkedList::with_capacity(iter.size_hint().0);
        for item in iter {
            list.push_back(item);
        }
        list
    }
}
impl<T: Eq> Eq for LinkedList<T> {}
impl<T: PartialEq<U>, U> PartialEq<LinkedList<U>> for LinkedList<T> {
    fn eq(&self, other: &LinkedList<U>) -> bool {
        if self.len() != other.len() { return false; }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T: PartialEq<U>, U> PartialEq<Vec<U>> for LinkedList<T> {
    fn eq(&self, other: &Vec<U>) -> bool {
        if self.len() != other.len() { return false; }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T> Extend<T> for LinkedList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.reserve(iter.size_hint().0);
        for item in iter {
            self.push_back(item);
        }
    }
}
impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(mut self) -> IntoIter<T> {
        let iter = IntoIter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            allocations: unsafe { ptr::read(&mut self.allocations) }
        };
        mem::forget(self);
        iter
    }
}
impl<T: Hash> Hash for LinkedList<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for LinkedList<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut tuple = f.debug_list();
        for item in self.iter() {
            tuple.entry(item);
        }
        tuple.finish()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    #[test]
    fn retain() {
        let mut list: LinkedList<usize> = LinkedList::new();
        for i in 0..16 {
            list.push_back(i);
        }

        let mut rng = thread_rng();

        let mut mask = [false; 16];
        for val in mask.iter_mut() {
            *val = rng.gen();
        }

        list.retain_map(|i| {
            if mask[i] {
                Some(i + 1)
            } else {
                None
            }
        });

        let nums: Vec<usize> = (0..16).filter(|&i| mask[i]).map(|i| i+1).collect();

        println!("{:?}", mask);
        for (a, b) in list.into_iter().zip(nums.into_iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn iter_collect_compare() {
        let mut list = LinkedList::new();
        for i in 0..64usize {
            list.push_back(i);
        }
        let list2: LinkedList<u32> = list.iter().map(|&i| i as u32).collect();
        let vec: Vec<u32> = list.into_iter().map(|i| i as u32).collect();

        assert_eq!(list2, vec);
    }
}
