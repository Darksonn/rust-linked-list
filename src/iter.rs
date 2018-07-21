//! This module provides various iterators over the linked list.
use super::*;

#[cfg(feature = "nightly")]
use std::iter::TrustedLen;
use std::iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator};
use std::fmt;
use std::marker::PhantomData;
use std::ptr;

/// An iterator over borrowed values from a linked list.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Iter<'a, T: 'a> {
    pub(crate) head: *mut LinkedNode<T>,
    pub(crate) tail: *mut LinkedNode<T>,
    pub(crate) len: usize,
    pub(crate) marker: PhantomData<&'a T>,
}
#[cfg(feature = "nightly")]
unsafe impl<'a, T> TrustedLen for Iter<'a, T> {}
unsafe impl<'a, T: Send> Send for Iter<'a, T> {}
unsafe impl<'a, T: Sync> Sync for Iter<'a, T> {}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        if self.len > 0 {
            debug_assert!(!self.head.is_null());
            unsafe {
                let value = Some(&(*self.head).value);
                self.head = (*self.head).next;
                self.len -= 1;
                value
            }
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
    fn count(self) -> usize {
        self.len
    }
    fn last(self) -> Option<&'a T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe { Some(&(*self.tail).value) }
        } else {
            None
        }
    }
}
impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<&'a T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe {
                let value = Some(&(*self.tail).value);
                self.tail = (*self.tail).prev;
                self.len -= 1;
                value
            }
        } else {
            None
        }
    }
}
impl<'a, T> FusedIterator for Iter<'a, T> {}
impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}
impl<'a, T: fmt::Debug> fmt::Debug for Iter<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str("LinkedList::Iter")?;
        let mut out = f.debug_list();
        let mut ptr = self.head;
        for _ in 0..self.len {
            unsafe {
                out.entry(&(*ptr).value);
                ptr = (*ptr).next;
            }
        }
        out.finish()
    }
}

/// An iterator over mutably borrowed values from a linked list.
pub struct IterMut<'a, T: 'a> {
    pub(crate) head: *mut LinkedNode<T>,
    pub(crate) tail: *mut LinkedNode<T>,
    pub(crate) len: usize,
    pub(crate) marker: PhantomData<&'a mut T>,
}
#[cfg(feature = "nightly")]
unsafe impl<'a, T> TrustedLen for IterMut<'a, T> {}
unsafe impl<'a, T: Send> Send for IterMut<'a, T> {}
unsafe impl<'a, T: Sync> Sync for IterMut<'a, T> {}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        if self.len > 0 {
            debug_assert!(!self.head.is_null());
            unsafe {
                let value = Some(&mut (*self.head).value);
                self.head = (*self.head).next;
                self.len -= 1;
                value
            }
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
    fn count(self) -> usize {
        self.len
    }
    fn last(self) -> Option<&'a mut T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe { Some(&mut (*self.tail).value) }
        } else {
            None
        }
    }
}
impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<&'a mut T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe {
                let value = Some(&mut (*self.tail).value);
                self.tail = (*self.tail).prev;
                self.len -= 1;
                value
            }
        } else {
            None
        }
    }
}
impl<'a, T> FusedIterator for IterMut<'a, T> {}
impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}
impl<'a, T: fmt::Debug> fmt::Debug for IterMut<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str("LinkedList::IterMut")?;
        let mut out = f.debug_list();
        let mut ptr = self.head;
        for _ in 0..self.len {
            unsafe {
                // creating this reference cannot alias with any mutable reference
                // returned by the iterator, since it only prints the values not yet
                // returned
                out.entry(&(*ptr).value);
                ptr = (*ptr).next;
            }
        }
        out.finish()
    }
}

/// An iterator over values from a linked list.
pub struct IntoIter<T> {
    pub(crate) head: *mut LinkedNode<T>,
    pub(crate) tail: *mut LinkedNode<T>,
    pub(crate) len: usize,
    pub(crate) allocations: Vec<(*mut LinkedNode<T>, usize)>,
}
#[cfg(feature = "nightly")]
unsafe impl<T> TrustedLen for IntoIter<T> {}
unsafe impl<T: Send> Send for IntoIter<T> {}
unsafe impl<T: Sync> Sync for IntoIter<T> {}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.len > 0 {
            debug_assert!(!self.head.is_null());
            unsafe {
                let value = ptr::read(&(*self.head).value);
                self.head = (*self.head).next;
                self.len -= 1;
                Some(value)
            }
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
    fn count(self) -> usize {
        self.len
    }
    fn last(self) -> Option<T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe { Some(ptr::read(&(*self.tail).value)) }
        } else {
            None
        }
    }
}
impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.len > 0 {
            debug_assert!(!self.tail.is_null());
            unsafe {
                let value = ptr::read(&(*self.tail).value);
                self.tail = (*self.tail).prev;
                self.len -= 1;
                Some(value)
            }
        } else {
            None
        }
    }
}
impl<T> FusedIterator for IntoIter<T> {}
impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.len
    }
}
impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        unsafe {
            // drop remaining elements
            while let Some(_) = self.next() {}

            // deallocate memory
            for &(vecptr, capacity) in &self.allocations {
                let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                drop(vec);
            }
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str("LinkedList::IntoIter")?;
        let mut out = f.debug_list();
        let mut ptr = self.head;
        for _ in 0..self.len {
            unsafe {
                out.entry(&(*ptr).value);
                ptr = (*ptr).next;
            }
        }
        out.finish()
    }
}
