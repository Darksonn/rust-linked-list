#![cfg_attr(feature = "nightly", feature(trusted_len))]

//! This crate provides a linked list with a special allocation method, allowing
//! allocations of several nodes in one allocation.
//!
//! # Cursors and iterators
//!
//! Besides basic access to the list on the two ends, you're going to need either an
//! iterator or a cursor to access the list.
//!
//! This crate supplies two cursor types, one for immutable access: [`CursorRef`], and one
//! for mutable access: [`CursorMut`].
//!
//! When accessing the interior of the linked list you can either do so with an iterator
//! or a [`CursorRef`].  You should use an iterator if you simply need to see every
//! element once, and if you want to move back and forth you should use a [`CursorRef`].
//!
//! As for mutating the linked list, a mutable iterator only allows modifying the values
//! of the list, and adding or removing values is not possible.  A [`CursorMut`] on the
//! other hand allows moving around arbitrarily, and allows insertion and removal of
//! items.  Note that a [`CursorMut`] doesn't allow obtaining simultaneous mutable
//! references to different elements like a mutable iterator does.
//!
//! Note that the list can also be modified using the [`retain_map`], [`retain_mut`] and
//! [`retain`] methods.
//!
//! # Features
//!
//! This crate provides a `serde` feature which implements [`Serialize`] and
//! [`Deserialize`] on `LinkedList`.
//!
//! A `nightly` feature is provided, which currently just adds implementations of
//! [`TrustedLen`] on iterators, but it may provide more nightly-only features in the
//! future.
//!
//! # Examples
//!
//! ```
//! use linked_list::LinkedList;
//!
//! let mut list: LinkedList<u32> = LinkedList::new();
//! list.push_back(3);
//! list.push_front(2);
//! list.push_front(1);
//!
//! let items: Vec<u32> = list.iter().map(|&i| 10*i).collect();
//! assert_eq!(items, [10, 20, 30]);
//! ```
//!
//! [`TrustedLen`]: https://doc.rust-lang.org/std/iter/trait.TrustedLen.html
//! [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
//! [`Deserialize`]: https://docs.serde.rs/serde/trait.Deserialize.html
//! [`CursorRef`]: struct.CursorRef.html
//! [`CursorMut`]: struct.CursorMut.html
//! [`retain_map`]: struct.LinkedList.html#method.retain_map
//! [`retain_mut`]: struct.LinkedList.html#method.retain_mut
//! [`retain`]: struct.LinkedList.html#method.retain

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::{Extend, FromIterator, IntoIterator};
use std::marker::PhantomData;
use std::mem;
use std::ptr;

mod cursor;
mod iter;
pub use cursor::{CursorMut, CursorRef};
pub use iter::{IntoIter, Iter, IterMut};

#[cfg(test)]
extern crate rand;

/// A doubly-linked list with nodes allocated in large owned chunks.
///
/// The difference between this linked list and the one in the standard library is the
/// allocation method. The standard library linked list allocates each node in it's own
/// `Box`, while this allocates a `Vec` with many nodes at a time, and keeps an internal
/// list of unused nodes as well as a list of allocations.
///
/// This has the advantage that the nodes are more likely to be closer to each other on
/// the heap, thus increasing CPU cache efficieny, as well as decreasing the number of
/// allocations. It has the downside that you can't deallocate individual nodes, so the
/// only way to deallocate memory owned by this list is to drop it.
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
    /// Creates an empty `LinkedList` with a chunk size of 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(64, list.chunk_size());
    /// ```
    #[inline]
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
    /// Creates an empty `LinkedList` with a chunk size of 64 and makes a single
    /// allocation with the specified amount of nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let list: LinkedList<u32> = LinkedList::with_capacity(293);
    /// assert_eq!(293, list.capacity());
    /// ```
    #[inline]
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

    /// Add the element to the back of the linked list in `O(1)`, unless it has to
    /// allocate, which is `O(chunk_size)`.
    ///
    /// This will not make any allocation unless `len = capacity`, in which case it will
    /// allocate `chunk_size` nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(0, list.capacity());
    /// // add an element, this will cause an allocation
    /// list.push_back(35);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&35), list.back());
    ///
    /// // if we add another, then since the allocation is large enough, this shouldn't
    /// // change the capacity
    /// list.push_back(29);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&29), list.back());
    /// // the first element should still be at the front of the list
    /// assert_eq!(Some(&35), list.front());
    /// ```
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
    /// Add the element to the front of the linked list in `O(1)`, unless it has to
    /// allocate, which is `O(chunk_size)`.
    ///
    /// This will not make any allocation unless `len = capacity`, in which case it will
    /// allocate `chunk_size` nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(0, list.capacity());
    /// // add an element, this will cause an allocation
    /// list.push_front(35);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&35), list.front());
    ///
    /// // if we add another, then since the allocation is large enough, this shouldn't
    /// // change the capacity
    /// list.push_front(29);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&29), list.front());
    /// // the first element should still be at the back of the list
    /// assert_eq!(Some(&35), list.back());
    /// ```
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
    /// Provides a reference to the back element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.back());
    ///
    /// // add an element
    /// list.push_back(32);
    /// assert_eq!(Some(&32), list.back());
    ///
    /// // add another
    /// list.push_back(45);
    /// assert_eq!(Some(&45), list.back());
    ///
    /// // if we add an element in the other end, we still see 45
    /// list.push_front(12);
    /// assert_eq!(Some(&45), list.back());
    /// ```
    #[inline]
    pub fn back(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&(*self.tail).value) }
        }
    }
    /// Provides a reference to the front element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.front());
    ///
    /// // add an element
    /// list.push_front(32);
    /// assert_eq!(Some(&32), list.front());
    ///
    /// // add another
    /// list.push_front(45);
    /// assert_eq!(Some(&45), list.front());
    ///
    /// // if we add an element in the other end, we still see 45
    /// list.push_back(12);
    /// assert_eq!(Some(&45), list.front());
    /// ```
    #[inline]
    pub fn front(&self) -> Option<&T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&(*self.head).value) }
        }
    }
    /// Provides a mutable reference to the back element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.back_mut());
    ///
    /// // add an element
    /// list.push_back(32);
    ///
    /// // let's change the element we just added
    /// if let Some(back) = list.back_mut() {
    ///     assert_eq!(32, *back);
    ///     *back = 45;
    ///     assert_eq!(45, *back);
    /// }
    /// # else { unreachable!(); }
    ///
    /// // This changed the element in the list.
    /// assert_eq!(Some(&45), list.back());
    /// ```
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.tail).value) }
        }
    }
    /// Provides a mutable reference to the front element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.front_mut());
    ///
    /// // add an element
    /// list.push_front(32);
    ///
    /// // let's change the element we just added
    /// if let Some(front) = list.front_mut() {
    ///     assert_eq!(32, *front);
    ///     *front = 45;
    ///     assert_eq!(45, *front);
    /// }
    /// # else { unreachable!(); }
    ///
    /// // This changed the element in the list.
    /// assert_eq!(Some(&45), list.front());
    /// ```
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.head).value) }
        }
    }
    /// Removes the back element and returns it, or `None` if the list is empty.
    ///
    /// This is an `O(1)` operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// // the list is empty
    /// assert_eq!(None, list.pop_back());
    ///
    /// // add some elements
    /// list.push_back(3);
    /// list.push_back(2);
    /// list.push_back(1);
    /// // other end too
    /// list.push_front(4);
    ///
    /// assert_eq!(4, list.len());
    ///
    /// // let's pop them
    /// assert_eq!(Some(1), list.pop_back());
    /// assert_eq!(Some(2), list.pop_back());
    /// assert_eq!(Some(3), list.pop_back());
    /// assert_eq!(Some(4), list.pop_back());
    /// // we removed all the items
    /// assert_eq!(None, list.pop_back());
    ///
    /// assert_eq!(0, list.len());
    /// ```
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

                self.len -= 1;

                let value = ptr::read(&(*tail).value);
                self.discard_node(tail);
                Some(value)
            }
        }
    }
    /// Removes the front element and returns it, or `None` if the list is empty.
    ///
    /// This is an `O(1)` operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// // the list is empty
    /// assert_eq!(None, list.pop_front());
    ///
    /// // add some elements
    /// list.push_front(2);
    /// list.push_front(1);
    /// // other end too
    /// list.push_back(3);
    /// list.push_back(4);
    ///
    /// assert_eq!(4, list.len());
    ///
    /// // let's pop them
    /// assert_eq!(Some(1), list.pop_front());
    /// assert_eq!(Some(2), list.pop_front());
    /// assert_eq!(Some(3), list.pop_front());
    /// assert_eq!(Some(4), list.pop_front());
    /// // we removed all the items
    /// assert_eq!(None, list.pop_front());
    ///
    /// assert_eq!(0, list.len());
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                let head = self.head;
                self.head = (*head).next;

                if self.tail == head {
                    self.tail = ptr::null_mut();
                }

                self.len -= 1;

                let value = ptr::read(&(*head).value);
                self.discard_node(head);
                Some(value)
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`. This
    /// method operates in place and preserves the order of the retained elements.
    ///
    /// If the closure or drop panics then the list is cleared without calling drop and some
    /// capacity may be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.extend(&[0,1,2,3,4,5,6,7,8,9,10]);
    ///
    /// // remove all odd values
    /// list.retain(|&val| val % 2 == 0);
    ///
    /// assert_eq!(list, vec![0,2,4,6,8,10]);
    /// ```
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.retain_map(|val| if f(&val) { Some(val) } else { None });
    }
    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`. This
    /// method operates in place and preserves the order of the retained elements.
    ///
    /// Note that `retain_mut` lets you mutate every element in the list, regardless of
    /// whether you choose to keep or remove it.
    ///
    /// If the closure or drop panics then the list is cleared without calling drop and
    /// some capacity may be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.extend(&[0,1,2,3,4,5,6,7,8,9,10]);
    ///
    /// // add one to the value, then keep the odd values
    /// list.retain_mut(|val| {
    ///     *val += 1;
    ///     *val % 2 == 1
    /// });
    ///
    /// assert_eq!(list, vec![1,3,5,7,9,11]);
    /// ```
    pub fn retain_mut(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        self.retain_map(|mut val| if f(&mut val) { Some(val) } else { None });
    }
    /// Apply a mapping to the list in place, optionally removing elements.
    ///
    /// This method applies the closure to every element in the list, and replaces it with
    /// the value returned by the closure, or removes it if the closure returned `None`.
    /// This method preserves the order of the retained elements.
    ///
    /// Note that this method allows the closure to take ownership of removed elements.
    ///
    /// If the closure panics then the list is cleared without calling drop and some capacity may
    /// be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// // Create a list of owned strings.
    /// let mut list: LinkedList<String> = LinkedList::new();
    /// list.extend(vec!["first".to_string(), "second".to_string(), "third".to_string()]);
    ///
    /// // this example removes the middle element and makes the two other uppercase
    /// let mut variable_outside_list = "not second".to_string();
    ///
    /// list.retain_map(|string| {
    ///     if string == "second" {
    ///         // store the element outside the list and remove it
    ///
    ///         variable_outside_list = string;
    ///         None
    ///     } else {
    ///         // replace the element with the uppercase version
    ///
    ///         Some(string.to_uppercase())
    ///     }
    /// });
    ///
    /// assert_eq!(list, vec!["FIRST", "THIRD"]);
    /// assert_eq!(variable_outside_list, "second");
    /// ```
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
        self.capacity -= self.len;
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

    /// Moves all elements from `other` to the back of the list.
    ///
    /// This reuses all the nodes from `other` and moves them into `self`. After this
    /// operation, `other` becomes empty.
    /// Excess capacity as well as ownership of allocations in `other` is also moved into
    /// `self`.
    ///
    /// This method guarantees that the capacity in `self` is increased by
    /// `other.capacity()`, and that `other` will have a capacity of zero when this method
    /// returns.
    ///
    /// Moving the nodes from `other` to `self` is `O(1)`, but moving the excess capacity
    /// and the ownership of allocations requires a full iteration through one of them,
    /// meaning it is linear time, although `append` will always iterate through the
    /// shorter one.
    ///
    /// This method is `O(min(excess_capacity) + min(number_of_allocations))`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list_a: LinkedList<u32> = LinkedList::new();
    /// let mut list_b: LinkedList<u32> = LinkedList::new();
    ///
    /// // add elements to both lists
    /// list_a.extend(&[0,1,2,3,4]);
    /// list_b.extend(&[5,6,7,8,9]);
    ///
    /// // remember their capacities before appending
    /// let cap_a = list_a.capacity();
    /// let cap_b = list_b.capacity();
    ///
    /// list_a.append(&mut list_b);
    ///
    /// // check that the elements were moved
    /// assert_eq!(list_a, vec![0,1,2,3,4,5,6,7,8,9]);
    /// assert_eq!(list_b, vec![]);
    ///
    /// // check that the capacity was moved
    /// assert_eq!(cap_a + cap_b, list_a.capacity());
    /// assert_eq!(0, list_b.capacity());
    /// ```
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
                self.len += other.len;
            }
        }

        // move allocations
        if self.allocations.len() < other.allocations.len() {
            mem::swap(&mut self.allocations, &mut other.allocations);
        }
        // self.allocations is now the longest array
        self.allocations.extend(other.allocations.drain(..));

        // move unused capacity to self, since self now owns the memory
        self.capacity += other.capacity;
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

    /// Provides a forward iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(Some(&0), iter.next());
    /// assert_eq!(Some(&1), iter.next());
    /// assert_eq!(Some(&2), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        Iter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }
    /// Provides a forward iterator with mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// for element in list.iter_mut() {
    ///     *element += 10;
    /// }
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(Some(&10), iter.next());
    /// assert_eq!(Some(&11), iter.next());
    /// assert_eq!(Some(&12), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }
    /// Provides a cursor to the contents of the linked list, positioned at the back
    /// element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.cursor_ref_back().is_none());
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// if let Some(cursor) = list.cursor_ref_back() {
    ///     assert_eq!(&6, cursor.get());
    ///     assert_eq!(Some(&5), cursor.prev().map(|cursor| cursor.get()));
    ///     assert!(cursor.next().is_none());
    /// }
    /// # else { unreachable!(); }
    /// ```
    #[inline]
    pub fn cursor_ref_back(&self) -> Option<CursorRef<T>> {
        if self.tail.is_null() {
            None
        } else {
            Some(CursorRef::create(self.tail, self.len - 1))
        }
    }
    /// Provides a cursor to the contents of the linked list, positioned at the front
    /// element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.cursor_ref_front().is_none());
    /// list.push_front(5);
    /// list.push_front(6);
    ///
    /// if let Some(cursor) = list.cursor_ref_front() {
    ///     assert_eq!(&6, cursor.get());
    ///     assert_eq!(Some(&5), cursor.next().map(|cursor| cursor.get()));
    ///     assert!(cursor.prev().is_none());
    /// }
    /// # else { unreachable!(); }
    /// ```
    #[inline]
    pub fn cursor_ref_front(&self) -> Option<CursorRef<T>> {
        if self.head.is_null() {
            None
        } else {
            Some(CursorRef::create(self.head, 0))
        }
    }

    pub fn cursor_mut_back(&mut self) -> Option<CursorMut<T>> {
        if self.tail.is_null() {
            None
        } else {
            let tail = self.tail;
            let len = self.len;
            Some(CursorMut::create(self, tail, len - 1))
        }
    }
    pub fn cursor_mut_front(&mut self) -> Option<CursorMut<T>> {
        if self.head.is_null() {
            None
        } else {
            let head = self.head;
            Some(CursorMut::create(self, head, 0))
        }
    }

    /// Removes all elements from the `LinkedList`. This method guarantees that capacity
    /// is unchanged.
    ///
    /// This is `O(self.len)` unless `T` has no destructor, in which case it's `O(1)`.
    ///
    /// If drop on any element panics, this method won't drop the remaining nodes, but
    /// the list will still be cleared and no capacity is lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_front(2);
    /// list.push_front(1);
    /// assert_eq!(2, list.len());
    /// assert_eq!(Some(&1), list.front());
    ///
    /// let capacity_before_clear = list.capacity();
    ///
    /// list.clear();
    /// assert_eq!(0, list.len());
    /// assert_eq!(None, list.front());
    ///
    /// // no allocation was lost
    /// assert_eq!(capacity_before_clear, list.capacity());
    /// ```
    pub fn clear(&mut self) {
        if self.tail.is_null() {
            return;
        }

        let tail = self.tail;

        unsafe {
            // just append unused_nodes to the linked list, and make the result into the
            // new unused_nodes
            (*self.tail).next = self.unused_nodes;
            // unused_nodes is singly linked, so we don't need the other link
            self.unused_nodes = self.head;
        }
        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;

        if mem::needs_drop::<T>() {
            let mut ptr = tail;
            while !ptr.is_null() {
                unsafe {
                    ptr::drop_in_place(&mut (*ptr).value);
                    ptr = (*ptr).prev;
                }
            }
        }
    }

    /// Returns the number of elements the list can hold without allocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(48);
    /// assert_eq!(48, list.capacity());
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Returns the number of items in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_front(2);
    /// assert_eq!(1, list.len());
    ///
    /// list.push_back(3);
    /// assert_eq!(2, list.len());
    ///
    /// list.pop_front();
    /// assert_eq!(1, list.len());
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns `true` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.is_empty());
    ///
    /// list.push_back(3);
    /// assert!(!list.is_empty());
    ///
    /// list.pop_front();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Change the size of future allocations. This has no effect on previous allocations.
    ///
    /// When some operation increases the size of the linked list past the capacity, the
    /// linked list will allocate at least `chunk_size` nodes in one allocation.
    ///
    /// # Panics
    ///
    /// This method panics if `chunk_size` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// // default chunk size is 64
    /// assert_eq!(64, list.chunk_size());
    ///
    /// list.set_chunk_size(3);
    /// assert_eq!(3, list.chunk_size());
    ///
    /// // add an element, which allocates 3 nodes
    /// list.push_back(4);
    /// assert_eq!(3, list.capacity());
    /// ```
    #[inline]
    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        assert!(chunk_size > 0);
        self.chunk_size = chunk_size;
    }
    /// Returns the minimum size of future allocations.  See [`set_chunk_size`].
    ///
    /// [`set_chunk_size`]: #method.set_chunk_size
    #[inline]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Reserves capacity for at least `additional` more elements to be inserted in the
    /// list. This method will not reserve less than [`chunk_size`] nodes to avoid
    /// frequent allocations.
    ///
    /// This is `O(allocation_size)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(5);
    /// assert_eq!(5, list.capacity());
    ///
    /// list.push_back(3);
    /// list.reserve(84); // 84 is larger than the default chunk size
    ///
    /// // there's already one element in the list, so it increases the capacity to 85
    /// // the actual size of the allocation is 80, since the previous capacity was 5
    /// assert_eq!(85, list.capacity());
    /// ```
    ///
    /// [`chunk_size`]: #method.chunk_size
    pub fn reserve(&mut self, additional: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= additional {
            return;
        }
        let to_allocate = additional - free_capacity;

        let chunk_size = self.chunk_size;
        if to_allocate < chunk_size {
            self.allocate(chunk_size);
        } else {
            self.allocate(to_allocate);
        }
    }
    /// Reserves capacity for exactly `additional` more elements to be inserted in the
    /// list.
    ///
    /// This is `O(additional)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(5);
    /// assert_eq!(5, list.capacity());
    ///
    /// list.push_back(3);
    /// list.reserve_exact(5);
    ///
    /// // there's already one element in the list, so it increases the capacity to 6
    /// // the actual size of the allocation is 1, since the previous capacity was 5
    /// assert_eq!(6, list.capacity());
    /// ```
    ///
    /// [`chunk_size`]: #method.chunk_size
    pub fn reserve_exact(&mut self, additional: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= additional {
            return;
        }
        let to_allocate = additional - free_capacity;
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
        value: T,
    ) -> *mut LinkedNode<T> {
        unsafe {
            if self.unused_nodes.is_null() {
                let chunk_size = self.chunk_size;
                self.allocate(chunk_size);
            }
            let node = self.unused_nodes;
            self.unused_nodes = (*node).next;

            ptr::write(node, LinkedNode { next, prev, value });
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
impl<T: PartialEq<U>, U> PartialEq<[U]> for LinkedList<T> {
    fn eq(&self, other: &[U]) -> bool {
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
impl<'a, T: PartialEq<U>, U> PartialEq<&'a [U]> for LinkedList<T> {
    fn eq(&self, other: &&'a [U]) -> bool {
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
impl<'a, T: PartialEq<U>, U> PartialEq<&'a mut [U]> for LinkedList<T> {
    fn eq(&self, other: &&'a mut [U]) -> bool {
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
    fn into_iter(self) -> IntoIter<T> {
        let iter = IntoIter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            allocations: unsafe { ptr::read(&self.allocations) },
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
        let mut out = f.debug_list();
        for item in self.iter() {
            out.entry(item);
        }
        out.finish()
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use std::fmt::Write;
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
    #[test]
    fn debug_print_list() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);
        write!(output, "{:?}", list).unwrap();
        assert_eq!(output, "[1, 2, 3, 4]");
    }
    #[test]
    fn debug_print_iter() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.iter();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::Iter[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::Iter[2, 3]");
        output.clear();
    }
    #[test]
    fn debug_print_iter_mut() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.iter_mut();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IterMut[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IterMut[2, 3]");
        output.clear();
    }
    #[test]
    fn debug_print_into_iter() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.into_iter();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IntoIter[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IntoIter[2, 3]");
        output.clear();
    }
    #[test]
    fn iter_mut_several_mut_ref() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        {
            let mut iter_mut = list.iter_mut();
            let ref1 = iter_mut.next().unwrap();
            let ref2 = iter_mut.next().unwrap();
            drop(iter_mut);
            *ref1 = 6;
            *ref2 = 7;
        }

        assert_eq!(list, vec![6, 7, 3, 4]);
    }
}
