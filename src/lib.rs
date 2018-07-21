use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::{Extend, FromIterator, IntoIterator};
use std::marker::PhantomData;
use std::mem;
use std::ptr;

pub mod iter;
use iter::{IntoIter, Iter, IterMut};

#[cfg(test)]
extern crate rand;

pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    capacity: usize,
    chunk_size: usize,
    allocations: Vec<(*mut LinkedNode<T>, usize)>,
    unused_nodes: *mut LinkedNode<T>,
}

// LinkedLists own their data, so the borrow checker should prevent data races.
unsafe impl<T: Send> Send for LinkedList<T> {}
unsafe impl<T: Sync> Sync for LinkedList<T> {}

struct LinkedNode<T> {
    next: *mut LinkedNode<T>,
    prev: *mut LinkedNode<T>,
    value: T,
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
            unused_nodes: ptr::null_mut(),
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
            unused_nodes: ptr::null_mut(),
        };
        list.allocate(cap);
        list
    }

    /// Add the element to the back of the linked list.
    ///
    /// This method is O(1).
    pub fn push_back(&mut self, value: T) {
        let tail = self.tail;
        let node = self.new_node(ptr::null_mut(), tail, value);

        if self.head.is_null() {
            self.head = node;
        }
        if !self.tail.is_null() {
            unsafe {
                (*self.tail).next = node;
            }
        }

        self.tail = node;
        self.len += 1;
    }
    /// Add the element to the front of the linked list.
    ///
    /// This method is O(1).
    pub fn push_front(&mut self, value: T) {
        let head = self.head;
        let node = self.new_node(head, ptr::null_mut(), value);

        if self.tail.is_null() {
            self.tail = node;
        }
        if !self.head.is_null() {
            unsafe {
                (*self.head).prev = node;
            }
        }

        self.head = node;
        self.len += 1;
    }
    /// Returns the last element in the list, or none if it is empty.
    ///
    /// This method is O(1).
    pub fn back(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&(*self.tail).value) }
        }
    }
    /// Returns the first element in the list, or none if it is empty.
    ///
    /// This method is O(1).
    pub fn front(&self) -> Option<&T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&(*self.head).value) }
        }
    }
    /// Returns the last element in the list, or none if it is empty.
    ///
    /// This method is O(1).
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.tail).value) }
        }
    }
    /// Returns the first element in the list, or none if it is empty.
    ///
    /// This method is O(1).
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.head).value) }
        }
    }
    /// Remove the last element in the list and return it, or none if it's empty.
    ///
    /// This method is O(1).
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
    ///
    /// This method is O(1).
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
    ///
    /// If `f` panics, the linked list will be left empty, and it may not call `drop` on
    /// some elements, although memory will still be properly deallocated when the linked
    /// list is dropped.
    ///
    /// This method is O(n).
    pub fn retain_map(&mut self, mut f: impl FnMut(T) -> Option<T>) {
        if self.is_empty() {
            return;
        }
        let mut ptr = self.head;
        let mut last_retain: *mut LinkedNode<T> = ptr::null_mut();
        let capacity = self.capacity;

        // If f panics, then we just throw away all the used nodes.
        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;
        // Since we are throwing away the used nodes, then the capacity is decreased by
        // the number of used nodes.
        self.capacity = self.capacity - self.len;
        // This means that if f panics, then we won't call drop on the remaining values,
        // but that's safe so it's ok.
        // We still deallocate the memory the nodes are stored in when the list is
        // dropped, since we didn't touch the allocations array.

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
                    }
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
        // we didn't panic so put capacity back at the actual value
        // we didn't allocate or deallocate in this method, so capacity is the same
        self.capacity = capacity;
    }
    /// Go through the list, calling `f` on each element, which may mutate the element,
    /// then removes it if `f` returns `false`.
    ///
    /// This method is O(n).
    pub fn retain_mut(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        self.retain_map(|mut val| if f(&mut val) { Some(val) } else { None });
    }
    /// Go through the list, calling `f` on each element, and removes it if `f` returns
    /// `false`.
    ///
    /// This method is O(n).
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.retain_map(|val| if f(&val) { Some(val) } else { None });
    }

    /// Append all nodes from other to this list.
    ///
    /// This method moves the nodes from the `other` linked list directly into this linked
    /// list, meaning that the running time doesn't depend on the size of the lists.
    ///
    /// Note that this method also moves all excess capacity from one list to the other,
    /// which is `O(min(self.len - self.capacity, other.len - other.capacity))`.
    ///
    /// This method guarantees that the capacity in `self` is increased by
    /// `other.capacity()`, and that `other` will have a capacity of zero when this method
    /// returns.
    ///
    /// Besides moving capacity, this method also moves the list of allocations, which
    /// takes `O(min(self.allocations.len(), other.allocations.len()))` time. This has the
    /// unfortunate effect that if the size of allocations isn't controlled with
    /// `reserve`, `with_capacity`, etc., then this is `O(min(self.len, other.len))` as
    /// every allocation will have a size of 64.
    ///
    /// This method is `O(min(excess_capacity) + min(number_of_allocations))`.
    pub fn append(&mut self, other: &mut LinkedList<T>) {
        if self.is_empty() {
            // just directly move the chain to self
            self.head = other.head;
            self.tail = other.tail;
            self.len = other.len;
        } else if other.is_empty() {
            // do nothing
        } else {
            // both have elements so we append the chain
            unsafe {
                (*self.tail).next = other.head;
                (*other.head).prev = self.tail;
                self.tail = other.tail;
            }
        }

        // move allocations
        if self.allocations.len() < other.allocations.len() {
            mem::swap(&mut self.allocations, &mut other.allocations);
        }
        // self.allocations is now the longest array
        self.allocations.extend(other.allocations.drain(..));

        // move unused capacity to self, since self now owns the memory
        self.capacity = self.capacity + other.capacity;
        self.combine_unused_nodes(other);

        // other is now empty
        other.head = ptr::null_mut();
        other.tail = ptr::null_mut();
        other.len = 0;
        other.capacity = 0;
        // allocations is emptied by drain
        debug_assert!(other.allocations.is_empty());
        // unused_nodes is moved by combined_unused_nodes
        debug_assert!(other.unused_nodes.is_null());
    }
    fn combine_unused_nodes(&mut self, other: &mut LinkedList<T>) {
        if self.capacity - self.len < other.capacity - other.len {
            mem::swap(&mut self.unused_nodes, &mut other.unused_nodes);
        }
        // self.unused_nodes is now a longer linked list than the one in other
        // let's find the last node in other.unused_nodes
        let mut ptr = other.unused_nodes;
        if ptr.is_null() {
            // other is null, so we moved all unused_nodes with the swap
            return;
        }
        unsafe {
            // iterate to the last node
            while !(*ptr).next.is_null() {
                ptr = (*ptr).next;
            }
            // we now put the unused_nodes in other in front of the ones in self
            (*ptr).next = self.unused_nodes;
            self.unused_nodes = other.unused_nodes;
            other.unused_nodes = ptr::null_mut();
        }
    }

    /// Borrows the list and returns an iterator through the elements in the list.
    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        Iter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }
    /// Mutably borrows the list and returns an iterator through the elements in the list.
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, T> {
        IterMut {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }

    /// Clears the linked list, but does not deallocate any memory.
    ///
    /// This is `O(self.len)` unless `T` has no destructor, in which case it's `O(1)`.
    pub fn clear(&mut self) {
        if mem::needs_drop::<T>() {
            // we need to drop this type, so let's go through every element and drop it
            let mut ptr = self.tail;
            while !ptr.is_null() {
                unsafe {
                    // this call drops the node and adds it to unused_nodes
                    self.drop_node(ptr);
                    // we go from the tail to the head, since then adding nodes with
                    // push_back will use nodes in the same order as they were before
                    // clear was called
                    ptr = (*ptr).prev;
                }
            }
        } else {
            unsafe {
                // just merge the linked list into the linked list in unused_nodes
                (*self.tail).next = self.unused_nodes;
                // unused_nodes is singly linked, so we don't need the other link
                self.unused_nodes = self.head;
            }
        }

        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;
    }

    /// Returns the capacity of the linked list.
    ///
    /// This is O(1).
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Returns the number of items in the linked list.
    ///
    /// This is O(1).
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns the number of items allocated when more memory is needed.
    ///
    /// This is O(1).
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
    /// Returns true if the list is empty.
    ///
    /// This is O(1).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set the number of items allocated when more memory is needed.
    ///
    /// This does not affect previous allocations.
    ///
    /// This is O(1).
    pub fn set_chunk_size(&mut self, size: usize) {
        assert!(size > 0);
        self.chunk_size = size;
    }
    /// Allocate enough memory for the linked list to contain at least `amount` memory, but
    /// may allocate up to `chunk_size` if `amount` is less than `chunk_size`.
    ///
    /// This is O(allocation_size).
    pub fn reserve(&mut self, amount: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= amount {
            return;
        }
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
    ///
    /// This is O(allocation_size).
    pub fn reserve_exact(&mut self, amount: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= amount {
            return;
        }
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
        value: T,
    ) -> *mut LinkedNode<T> {
        unsafe {
            if self.unused_nodes.is_null() {
                let chunk_size = self.chunk_size;
                self.allocate(chunk_size);
            }
            let node = self.unused_nodes;
            self.unused_nodes = (*node).next;

            ptr::write(
                node,
                LinkedNode {
                    next: next,
                    prev: prev,
                    value: value,
                },
            );
            node
        }
    }

    fn allocate(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }
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
        if self.len() != other.len() {
            return false;
        }
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
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T: Ord> Ord for LinkedList<T> {
    fn cmp(&self, other: &LinkedList<T>) -> Ordering {
        for (a, b) in self.iter().zip(other.iter()) {
            match a.cmp(b) {
                Ordering::Equal => {}
                ordering => {
                    return ordering;
                }
            }
        }
        Ordering::Equal
    }
}
impl<T: PartialOrd<U>, U> PartialOrd<LinkedList<U>> for LinkedList<T> {
    fn partial_cmp(&self, other: &LinkedList<U>) -> Option<Ordering> {
        for (a, b) in self.iter().zip(other.iter()) {
            match a.partial_cmp(b) {
                Some(Ordering::Equal) => {}
                ordering => {
                    return ordering;
                }
            }
        }
        Some(Ordering::Equal)
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
impl<'a, T: 'a + Copy> Extend<&'a T> for LinkedList<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.reserve(iter.size_hint().0);
        for item in iter {
            self.push_back(*item);
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
            allocations: unsafe { ptr::read(&mut self.allocations) },
        };
        mem::forget(self);
        iter
    }
}
impl<'a, T> IntoIterator for &'a LinkedList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}
impl<'a, T> IntoIterator for &'a mut LinkedList<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
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

        list.retain_map(|i| if mask[i] { Some(i + 1) } else { None });

        let nums: Vec<usize> = (0..16).filter(|&i| mask[i]).map(|i| i + 1).collect();

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

// serde impls
#[cfg(feature = "serde")]
extern crate serde;
#[cfg(all(feature = "serde", test))]
extern crate serde_json;
#[cfg(feature = "serde")]
use serde::{de::SeqAccess, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "serde")]
impl<T: Serialize> Serialize for LinkedList<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for e in self.iter() {
            seq.serialize_element(e)?;
        }
        seq.end()
    }
}
#[cfg(feature = "serde")]
struct LinkedListVisitor<T> {
    marker: PhantomData<T>,
}
#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Visitor<'de> for LinkedListVisitor<T> {
    type Value = LinkedList<T>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a sequence")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut list = match seq.size_hint() {
            Some(hint) => LinkedList::with_capacity(hint),
            None => LinkedList::new(),
        };
        while let Some(next) = seq.next_element()? {
            list.push_back(next);
        }
        Ok(list)
    }
}
#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Deserialize<'de> for LinkedList<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(LinkedListVisitor {
            marker: PhantomData,
        })
    }
}

#[cfg(all(feature = "serde", test))]
mod serde_test {
    use super::*;
    use rand::prelude::*;
    #[test]
    fn serialize() {
        let mut list: LinkedList<u32> = LinkedList::new();
        list.set_chunk_size(328);
        for _ in 0..1028 {
            list.push_back(random());
        }

        let json = serde_json::to_string(&list).unwrap();
        println!("{}", &json);
        let list2: LinkedList<u32> = serde_json::from_str(&json).unwrap();

        assert_eq!(list, list2);
    }
}
