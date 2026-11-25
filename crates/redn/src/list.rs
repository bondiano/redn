//! Wrapper type for encoding and decoding EDN lists `(...)`.

use std::ops::{Deref, DerefMut};
use std::slice::{Iter, IterMut};
use std::vec::IntoIter;

/// A thin wrapper around `Vec<T>` that encodes to and decodes from an EDN list `(...)`.
///
/// Because plain `Vec<T>` maps to an EDN vector `[...]`, use `List<T>` (or
/// [`std::collections::LinkedList`]) whenever a typed value should serialize as
/// a parenthesized EDN list.
///
/// # Examples
///
/// ```
/// use redn::{List, decode_text, encode_text};
///
/// let list = List(vec![1_i64, 2, 3]);
/// assert_eq!(encode_text(&list), "(1 2 3)");
/// assert_eq!(encode_text(&vec![1_i64, 2, 3]), "[1 2 3]");
///
/// let decoded: List<i64> = decode_text("example", "(1 2 3)")?;
/// assert_eq!(decoded, list);
/// # Ok::<(), redn::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct List<T>(pub Vec<T>);

impl<T> List<T> {
    /// Creates an empty EDN list wrapper.
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Consumes the wrapper and returns the inner `Vec<T>`.
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    /// Returns the number of elements in the list.
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the list contains no elements.
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deref for List<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for List<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<[T]> for List<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T> From<Vec<T>> for List<T> {
    fn from(items: Vec<T>) -> Self {
        Self(items)
    }
}

impl<T> From<List<T>> for Vec<T> {
    fn from(list: List<T>) -> Self {
        list.0
    }
}

impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<T> IntoIterator for List<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a List<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut List<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}
