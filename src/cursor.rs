//! This module provides cursors on a linked list, allowing more complicated list
//! operations.
use super::*;

use std::fmt;
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
    pub fn next(self) -> Option<CursorRef<'a, T>> {
        let next = unsafe { (*self.cursor).next };
        if next.is_null() {
            None
        } else {
            Some(CursorRef::create(next, self.index + 1))
        }
    }
    /// Returns the previous cursor, or `None` if this is the front of the list.
    pub fn prev(self) -> Option<CursorRef<'a, T>> {
        let prev = unsafe { (*self.cursor).prev };
        if prev.is_null() {
            None
        } else {
            Some(CursorRef::create(prev, self.index - 1))
        }
    }
    /// Provides a immutable reference to the element this cursor currently points at.
    pub fn get(self) -> &'a T {
        unsafe { &(*self.cursor).value }
    }
    /// Returns the index of the cursor in the linked list. The front of the list has
    /// index zero and the back of the list has index `len - 1`.
    pub fn index(self) -> usize {
        self.index
    }
    /// Return `true` if the cursors point to the same element. Note that this does not
    /// compare the actual values they point to. Returns `false` if the cursors are from
    /// different `LinkedList`s, even if their `index` is equal.
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
        f.debug_tuple("CursorRef")
            .field(self.get())
            .finish()
    }
}
