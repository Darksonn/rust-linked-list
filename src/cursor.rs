//! This module provides cursors on a linked list, allowing more complicated list
//! operations.
use super::*;

use std::fmt;
use std::iter::Rev;
use std::marker::PhantomData;

/// A cursor with immutable access to the `LinkedList`.
///
/// A `CursorRef` always points to a valid element in a linked list, and allows immutable
/// access with the [`get`] method. The cursor allows moving around the `LinkedList`
/// in both directions and is created using the [`cursor_ref_front`] and
/// [`cursor_ref_back`] methods.
///
/// A cursor is simply a pointer, and is therefore `Copy`, allowing duplicating a cursor
/// to some element.
///
/// [`get`]: #method.get
/// [`cursor_ref_front`]: struct.LinkedList.html#method.cursor_ref_front
/// [`cursor_ref_back`]: struct.LinkedList.html#method.cursor_ref_back
pub struct CursorRef<'a, T: 'a> {
    cursor: *const LinkedNode<T>,
    index: usize,
    marker: PhantomData<&'a T>,
}

impl<'a, T: 'a> CursorRef<'a, T> {
    pub(crate) fn create(cursor: *const LinkedNode<T>, index: usize) -> Self {
        CursorRef {
            cursor,
            index,
            marker: PhantomData,
        }
    }
    /// Returns the next cursor, or `None` if this is the back of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// let front = list.cursor_ref_front().unwrap();
    /// let back = list.cursor_ref_back().unwrap();
    ///
    /// assert!(back.next().is_none());
    /// assert!(front.next().unwrap().ptr_eq(back));
    /// ```
    pub fn next(self) -> Option<CursorRef<'a, T>> {
        let next = unsafe { (*self.cursor).next };
        if next.is_null() {
            None
        } else {
            Some(CursorRef::create(next, self.index + 1))
        }
    }
    /// Returns the previous cursor, or `None` if this is the front of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// let front = list.cursor_ref_front().unwrap();
    /// let back = list.cursor_ref_back().unwrap();
    ///
    /// assert!(front.prev().is_none());
    /// assert!(back.prev().unwrap().ptr_eq(front));
    /// ```
    pub fn prev(self) -> Option<CursorRef<'a, T>> {
        let prev = unsafe { (*self.cursor).prev };
        if prev.is_null() {
            None
        } else {
            Some(CursorRef::create(prev, self.index - 1))
        }
    }
    /// Provides a immutable reference to the element this cursor currently points at. The
    /// reference is bound to the list and can outlive the cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// if let Some(cursor) = list.cursor_ref_front() {
    ///     assert_eq!(&1, cursor.get());
    ///     assert_eq!(&2, cursor.next().unwrap().get());
    /// }
    /// # else { unreachable!(); }
    /// ```
    pub fn get(self) -> &'a T {
        unsafe { &(*self.cursor).value }
    }
    /// Returns the index of the cursor in the linked list. The front of the list has
    /// index zero and the back of the list has index `len - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    /// list.push_back(4);
    ///
    /// if let Some(front) = list.cursor_ref_front() {
    ///     assert_eq!(0, front.index());
    ///     assert_eq!(1, front.next().unwrap().index());
    /// }
    /// # else { unreachable!(); }
    /// if let Some(back) = list.cursor_ref_back() {
    ///     assert_eq!(list.len() - 1, back.index());
    ///     assert_eq!(list.len() - 2, back.prev().unwrap().index());
    /// }
    /// # else { unreachable!(); }
    /// ```
    pub fn index(self) -> usize {
        self.index
    }
    /// Returns `true` if the cursor points to the front of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// assert!(list.cursor_ref_front().unwrap().is_front());
    /// assert!(!list.cursor_ref_back().unwrap().is_front());
    /// ```
    pub fn is_front(&self) -> bool {
        unsafe { (*self.cursor).prev.is_null() }
    }
    /// Returns `true` if the cursor points to the back of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// assert!(!list.cursor_ref_front().unwrap().is_back());
    /// assert!(list.cursor_ref_back().unwrap().is_back());
    /// ```
    pub fn is_back(&self) -> bool {
        unsafe { (*self.cursor).next.is_null() }
    }
    /// Return `true` if the cursors point to the same element. Note that this does not
    /// compare the actual values they point to. Returns `false` if the cursors are from
    /// different `LinkedList`s, even if their `index` is equal.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1); // front
    /// list.push_back(2); // middle
    /// list.push_back(3); // back
    ///
    /// let front = list.cursor_ref_front().unwrap();
    /// let back = list.cursor_ref_back().unwrap();
    /// assert!(!front.ptr_eq(back));
    ///
    /// let middle = front.next().unwrap();
    /// assert!(middle.ptr_eq(back.prev().unwrap()));
    ///
    /// assert!(middle.next().unwrap().ptr_eq(back));
    ///
    /// let mut other_list: LinkedList<u32> = LinkedList::new();
    /// other_list.push_back(1);
    /// other_list.push_back(2);
    /// other_list.push_back(3);
    /// assert!(!back.ptr_eq(other_list.cursor_ref_back().unwrap()));
    /// ```
    pub fn ptr_eq(self, other: CursorRef<T>) -> bool {
        self.cursor == other.cursor
    }
}
impl<'a, T: 'a> Clone for CursorRef<'a, T> {
    fn clone(&self) -> Self {
        CursorRef {
            cursor: self.cursor,
            index: self.index,
            marker: PhantomData,
        }
    }
}
impl<'a, T: 'a> Copy for CursorRef<'a, T> {}
unsafe impl<'a, T: Sync + 'a> Send for CursorRef<'a, T> {}
unsafe impl<'a, T: Sync + 'a> Sync for CursorRef<'a, T> {}
impl<'a, T: fmt::Debug> fmt::Debug for CursorRef<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_tuple("CursorRef").field(self.get()).finish()
    }
}

/// An unique cursor with mutable access to the `LinkedList`.
///
/// A `CursorMut` always points to a valid element in a linked list, and allows mutable
/// access with the [`get`] method. The cursor allows moving around the `LinkedList`
/// in both directions and is created using the [`cursor_mut_front`] and
/// [`cursor_mut_back`] methods.
///
/// [`get`]: #method.get
/// [`cursor_mut_front`]: struct.LinkedList.html#method.cursor_mut_front
/// [`cursor_mut_back`]: struct.LinkedList.html#method.cursor_mut_back
pub struct CursorMut<'a, T: 'a> {
    list: &'a mut LinkedList<T>,
    cursor: *mut LinkedNode<T>,
    index: usize,
}

impl<'a, T: 'a> CursorMut<'a, T> {
    pub(crate) fn create(
        list: &'a mut LinkedList<T>,
        cursor: *mut LinkedNode<T>,
        index: usize,
    ) -> Self {
        CursorMut {
            list,
            cursor,
            index,
        }
    }
    /// Move the cursor to the next element, unless it's the back element of the list.
    /// Returns `true` if the cursor was moved.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// // we can't have two mutable cursors at the same time
    /// {
    ///     let mut front = list.cursor_mut_front().unwrap();
    ///     assert!(front.go_next());  // first go_next succeeds
    ///     assert!(!front.go_next()); // second go_next fails
    /// }
    /// {
    ///     let mut back = list.cursor_mut_back().unwrap();
    ///     assert!(!back.go_next()); // go_next fails
    /// }
    /// ```
    pub fn go_next(&mut self) -> bool {
        let next = unsafe { (*self.cursor).next };
        if next.is_null() {
            false
        } else {
            self.cursor = next;
            self.index += 1;
            true
        }
    }
    /// Consume this cursor and return the next cursor, unless this is the back of the
    /// list, in which case `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// // we can't have two mutable cursors at the same time
    /// {
    ///     let mut front = list.cursor_mut_front().unwrap();
    ///     assert!(front.next().is_some());
    /// }
    /// {
    ///     let mut back = list.cursor_mut_back().unwrap();
    ///     assert!(back.next().is_none());
    /// }
    /// ```
    pub fn next(mut self) -> Option<CursorMut<'a, T>> {
        if self.go_next() {
            Some(self)
        } else {
            None
        }
    }
    /// Move the cursor to the previous element, unless it's the front element of the
    /// list.  Returns `true` if the cursor was moved.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// // we can't have two mutable cursors at the same time
    /// {
    ///     let mut front = list.cursor_mut_back().unwrap();
    ///     assert!(front.go_prev());  // first go_prev succeeds
    ///     assert!(!front.go_prev()); // second go_prev fails
    /// }
    /// {
    ///     let mut back = list.cursor_mut_front().unwrap();
    ///     assert!(!back.go_prev()); // go_prev fails
    /// }
    /// ```
    pub fn go_prev(&mut self) -> bool {
        let prev = unsafe { (*self.cursor).prev };
        if prev.is_null() {
            false
        } else {
            self.cursor = prev;
            self.index -= 1;
            true
        }
    }
    /// Consume this cursor and return the previous cursor, unless this is the front of
    /// the list, in which case `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// // we can't have two mutable cursors at the same time
    /// {
    ///     let mut front = list.cursor_mut_front().unwrap();
    ///     assert!(front.prev().is_none());
    /// }
    /// {
    ///     let mut back = list.cursor_mut_back().unwrap();
    ///     assert!(back.prev().is_some());
    /// }
    /// ```
    pub fn prev(mut self) -> Option<CursorMut<'a, T>> {
        if self.go_prev() {
            Some(self)
        } else {
            None
        }
    }

    /// Insert a new node into the linked list. This method does not move the cursor, and
    /// the newly created element will be the next element when it returns.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(3);
    ///
    /// if let Some(mut front) = list.cursor_mut_front() {
    ///     assert_eq!(&1, front.get_ref());
    ///#    assert_eq!(0, front.index());
    ///     front.insert_next(2);
    ///#    assert_eq!(0, front.index());
    ///     assert_eq!(&1, front.get_ref());
    ///     assert_eq!(&2, front.next().unwrap().get_ref());
    /// }
    ///# else { unreachable!(); }
    /// assert_eq!(list, vec![1, 2, 3]);
    /// ```
    pub fn insert_next(&mut self, value: T) {
        let nextnext = unsafe { (*self.cursor).next };
        let node = self.list.new_node(nextnext, self.cursor, value);
        self.list.len += 1;

        unsafe {
            (*self.cursor).next = node;
            if nextnext.is_null() {
                self.list.tail = node;
            } else {
                (*nextnext).prev = node;
            }
        }
    }
    /// Insert a new node into the linked list. This method does not move the cursor, and
    /// the newly created element will be the previous element when it returns.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(mut front) = list.cursor_mut_front() {
    ///     assert_eq!(&2, front.get_ref());
    ///#    assert_eq!(0, front.index());
    ///     front.insert_prev(1);
    ///#    assert_eq!(1, front.index());
    ///     assert_eq!(&2, front.get_ref());
    ///     assert_eq!(&1, front.prev().unwrap().get_ref());
    /// }
    ///# else { unreachable!(); }
    /// assert_eq!(list, vec![1, 2, 3]);
    /// ```
    pub fn insert_prev(&mut self, value: T) {
        let prevprev = unsafe { (*self.cursor).prev };
        let node = self.list.new_node(self.cursor, prevprev, value);
        self.index += 1;
        self.list.len += 1;

        unsafe {
            (*self.cursor).prev = node;
            if prevprev.is_null() {
                self.list.head = node;
            } else {
                (*prevprev).next = node;
            }
        }
    }

    /// Remove the value and consume the cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// for i in 0..8 {
    ///     list.push_back(i*i);
    /// }
    ///
    /// // let's remove the value at index 4
    /// if let Some(mut cursor) = list.cursor_mut_front() {
    ///     while cursor.index() != 4 {
    ///         assert!(cursor.go_next());
    ///     }
    ///     assert_eq!(cursor.remove(), 16);
    /// }
    /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
    /// ```
    pub fn remove(self) -> T {
        unsafe {
            let prev = (*self.cursor).prev;
            let next = (*self.cursor).next;

            if prev.is_null() {
                self.list.head = next;
            } else {
                (*prev).next = next;
            }

            if next.is_null() {
                self.list.tail = prev;
            } else {
                (*next).prev = prev;
            }

            let value = ptr::read(&(*self.cursor).value);
            self.list.discard_node(self.cursor);
            self.list.len -= 1;
            value
        }
    }
    /// Remove the value and return the cursor to the next element, or `None` if this is
    /// the back.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// for i in 0..8 {
    ///     list.push_back(i*i);
    /// }
    ///
    /// // let's remove the value at index 4
    /// if let Some(mut cursor) = list.cursor_mut_front() {
    ///     while cursor.index() != 4 {
    ///         assert!(cursor.go_next());
    ///     }
    ///#    assert_eq!(4, cursor.index());
    ///     if let (removed, Some(next)) = cursor.remove_go_next() {
    ///#        assert_eq!(4, next.index());
    ///         assert_eq!(16, removed);
    ///         assert_eq!(&25, next.get_ref());
    ///         assert_eq!(&36, next.next().unwrap().get_ref());
    ///     }
    ///#    else { unreachable!(); }
    /// }
    ///# else { unreachable!(); }
    ///
    /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
    /// ```
    pub fn remove_go_next(self) -> (T, Option<CursorMut<'a, T>>) {
        unsafe {
            let cursor = self.cursor;
            let prev = (*cursor).prev;
            let next = (*cursor).next;

            if prev.is_null() {
                self.list.head = next;
            } else {
                (*prev).next = next;
            }

            if next.is_null() {
                self.list.tail = prev;
            } else {
                (*next).prev = prev;
            }

            let value = ptr::read(&(*cursor).value);
            self.list.discard_node(cursor);
            self.list.len -= 1;
            if next.is_null() {
                (value, None)
            } else {
                (value, Some(CursorMut::create(self.list, next, self.index)))
            }
        }
    }
    /// Remove the value and return the cursor to the previous element, or `None` if this
    /// is the front.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// for i in 0..8 {
    ///     list.push_back(i*i);
    /// }
    ///
    /// // let's remove the value at index 4
    /// if let Some(mut cursor) = list.cursor_mut_front() {
    ///     while cursor.index() != 4 {
    ///         assert!(cursor.go_next());
    ///     }
    ///#    assert_eq!(4, cursor.index());
    ///     if let (removed, Some(next)) = cursor.remove_go_prev() {
    ///#        assert_eq!(3, next.index());
    ///         assert_eq!(16, removed);
    ///         assert_eq!(&9, next.get_ref());
    ///         assert_eq!(&25, next.next().unwrap().get_ref());
    ///     }
    ///#    else { unreachable!(); }
    /// }
    ///# else { unreachable!(); }
    ///
    /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
    /// ```
    pub fn remove_go_prev(self) -> (T, Option<CursorMut<'a, T>>) {
        unsafe {
            let cursor = self.cursor;
            let prev = (*cursor).prev;
            let next = (*cursor).next;

            if prev.is_null() {
                self.list.head = next;
            } else {
                (*prev).next = next;
            }

            if next.is_null() {
                self.list.tail = prev;
            } else {
                (*next).prev = prev;
            }

            let value = ptr::read(&(*cursor).value);
            self.list.discard_node(cursor);
            self.list.len -= 1;
            if next.is_null() {
                (value, None)
            } else {
                (
                    value,
                    Some(CursorMut::create(self.list, prev, self.index - 1)),
                )
            }
        }
    }

    /// Swap the current value for a new value.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(mut cursor) = list.cursor_mut_front() {
    ///     assert_eq!(cursor.swap(5), 1);
    ///     cursor.go_next();
    ///     assert_eq!(cursor.swap(8), 2);
    ///     assert_eq!(cursor.swap(3), 8);
    ///     cursor.go_next();
    ///     assert_eq!(cursor.swap(100), 3);
    /// }
    ///# else { unreachable!(); }
    ///
    /// assert_eq!(list, vec![5, 3, 100]);
    /// ```
    pub fn swap(&mut self, value: T) -> T {
        unsafe {
            let previous_value = ptr::read(&(*self.cursor).value);
            ptr::write(&mut (*self.cursor).value, value);
            previous_value
        }
    }

    /// Provides a mutable reference to the element this cursor currently points at.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// if let Some(mut cursor) = list.cursor_mut_front() {
    ///     *cursor.get() = 3;
    /// }
    /// # else { unreachable!(); }
    ///
    /// assert_eq!(list, vec![3, 2]);
    /// ```
    #[allow(unknown_lints)]
    #[allow(needless_lifetimes)]
    pub fn get<'cursor>(&'cursor mut self) -> &'cursor mut T {
        unsafe { &mut (*self.cursor).value }
    }
    /// Provides an immutable reference to the element this cursor currently points at.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// if let Some(cursor) = list.cursor_mut_front() {
    ///     assert_eq!(&1, cursor.get_ref());
    ///     assert_eq!(&2, cursor.next().unwrap().get_ref());
    /// }
    /// # else { unreachable!(); }
    /// ```
    #[allow(unknown_lints)]
    #[allow(needless_lifetimes)]
    pub fn get_ref<'cursor>(&'cursor self) -> &'cursor T {
        unsafe { &(*self.cursor).value }
    }
    /// Consume the cursor and return a mutable reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// {
    ///     // this reference outlives the cursor
    ///     let reference = list.cursor_mut_front().unwrap().into_mut();
    ///     assert_eq!(*reference, 1);
    ///     *reference = 5;
    /// }
    /// assert_eq!(Some(&5), list.front());
    /// ```
    pub fn into_mut(self) -> &'a mut T {
        unsafe { &mut (*self.cursor).value }
    }
    /// Returns the index of the cursor in the linked list. The front of the list has
    /// index zero and the back of the list has index `len - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    /// list.push_back(4);
    ///
    /// if let Some(front) = list.cursor_mut_front() {
    ///     assert_eq!(0, front.index());
    ///     assert_eq!(1, front.next().unwrap().index());
    /// }
    /// # else { unreachable!(); }
    /// if let Some(back) = list.cursor_mut_back() {
    ///     assert_eq!(3, back.index());
    ///     assert_eq!(2, back.prev().unwrap().index());
    /// }
    /// # else { unreachable!(); }
    /// ```
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns `true` if the cursor points to the front of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// assert!(list.cursor_mut_front().unwrap().is_front());
    /// assert!(!list.cursor_mut_back().unwrap().is_front());
    /// ```
    pub fn is_front(&self) -> bool {
        unsafe { (*self.cursor).prev.is_null() }
    }
    /// Returns `true` if the cursor points to the back of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// assert!(!list.cursor_mut_front().unwrap().is_back());
    /// assert!(list.cursor_mut_back().unwrap().is_back());
    /// ```
    pub fn is_back(&self) -> bool {
        unsafe { (*self.cursor).next.is_null() }
    }

    /// Return an iterator from this element to the tail of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(head) = list.cursor_mut_front() {
    ///     let iter_from_2 = head.next().unwrap().iter_to_tail();
    ///     let vec: Vec<u32> = iter_from_2.map(|&mut v| v).collect();
    ///     assert_eq!(vec, [2, 3]);
    /// }
    ///# else { unreachable!(); }
    /// ```
    pub fn iter_to_tail(self) -> IterMut<'a, T> {
        let len = self.list.len - self.index;
        IterMut {
            head: self.cursor,
            tail: self.list.tail,
            marker: PhantomData,
            len,
        }
    }
    /// Return an iterator from the tail of the list to this element (inclusive). This is
    /// the same as calling `cursor.iter_to_tail().rev()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(head) = list.cursor_mut_front() {
    ///     let iter_to_2 = head.next().unwrap().iter_from_tail();
    ///     let vec: Vec<u32> = iter_to_2.map(|&mut v| v).collect();
    ///     assert_eq!(vec, [3, 2]);
    /// }
    ///# else { unreachable!(); }
    /// ```
    pub fn iter_from_tail(self) -> Rev<IterMut<'a, T>> {
        self.iter_to_tail().rev()
    }
    /// Return an iterator from this element to the head of the list. This is the same as
    /// calling `cursor.iter_from_head().rev()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(head) = list.cursor_mut_front() {
    ///     let iter_from_2 = head.next().unwrap().iter_to_head();
    ///     let vec: Vec<u32> = iter_from_2.map(|&mut v| v).collect();
    ///     assert_eq!(vec, [2, 1]);
    /// }
    ///# else { unreachable!(); }
    /// ```
    pub fn iter_to_head(self) -> Rev<IterMut<'a, T>> {
        self.iter_from_head().rev()
    }
    /// Return an iterator from the head of the list to this element (inclusive).
    ///
    /// # Examples
    ///
    /// ```
    /// use linked_list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    ///
    /// if let Some(head) = list.cursor_mut_front() {
    ///     let iter_to_2 = head.next().unwrap().iter_from_head();
    ///     let vec: Vec<u32> = iter_to_2.map(|&mut v| v).collect();
    ///     assert_eq!(vec, [1, 2]);
    /// }
    ///# else { unreachable!(); }
    /// ```
    pub fn iter_from_head(self) -> IterMut<'a, T> {
        IterMut {
            head: self.list.head,
            tail: self.cursor,
            len: self.index + 1,
            marker: PhantomData,
        }
    }
}
unsafe impl<'a, T: Send + 'a> Send for CursorMut<'a, T> {}
unsafe impl<'a, T: Sync + 'a> Sync for CursorMut<'a, T> {}
impl<'a, T: fmt::Debug> fmt::Debug for CursorMut<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_tuple("CursorMut").field(self.get_ref()).finish()
    }
}
