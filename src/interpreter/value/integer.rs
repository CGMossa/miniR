//! R integer vectors backed by Apache Arrow `Int64Array`.
//!
//! The Arrow array provides: contiguous i64 buffer + validity bitmap for NA tracking.
//! Memory layout matches the old `NullableBuffer<i64>`: 8 bytes per i64 + 1 bit per element.

use std::fmt;

use arrow_array::builder::PrimitiveBuilder;
use arrow_array::types::Int64Type;
use arrow_array::{Array, Int64Array};

/// Newtype for R integer vectors backed by Arrow `Int64Array`.
///
/// Wraps `Int64Array` where NA tracking uses a validity bitmap
/// instead of `Option<i64>` per element (halving memory for dense integers).
#[derive(Clone)]
pub struct Integer(pub Int64Array);

impl Integer {
    /// Number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get element `i` as `Option<i64>` by value.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn get_opt(&self, i: usize) -> Option<i64> {
        if self.0.is_null(i) {
            None
        } else {
            Some(self.0.value(i))
        }
    }

    /// Get the first element as `Option<i64>`.
    pub fn first_opt(&self) -> Option<i64> {
        if self.is_empty() {
            None
        } else {
            self.get_opt(0)
        }
    }

    /// Iterate yielding `Option<i64>` by value.
    pub fn iter_opt(&self) -> impl Iterator<Item = Option<i64>> + Clone + '_ {
        self.0.iter()
    }

    /// Iterate yielding `Option<i64>` by value (alias for `iter_opt`).
    ///
    /// Matches the iteration signature of the old `NullableBuffer::iter()`.
    pub fn iter(&self) -> impl Iterator<Item = Option<i64>> + Clone + '_ {
        self.0.iter()
    }

    /// True if element `i` is NA.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn is_na(&self, i: usize) -> bool {
        self.0.is_null(i)
    }

    /// Number of NA values.
    pub fn na_count(&self) -> usize {
        self.0.null_count()
    }

    /// True if there are any NAs.
    pub fn has_na(&self) -> bool {
        self.0.null_count() > 0
    }

    /// Convert to `Vec<Option<i64>>`.
    pub fn into_vec(self) -> Vec<Option<i64>> {
        self.0.iter().collect()
    }

    /// Borrow as `Vec<Option<i64>>` (allocates).
    pub fn to_option_vec(&self) -> Vec<Option<i64>> {
        self.0.iter().collect()
    }

    /// Raw values slice. NA positions hold arbitrary values.
    #[inline]
    pub fn values_slice(&self) -> &[i64] {
        self.0.values().as_ref()
    }

    /// Create a buffer from values with no NAs.
    pub fn from_values(values: Vec<i64>) -> Self {
        Integer(Int64Array::from(values))
    }

    /// Collect indices, producing a new `Integer`. Out-of-bounds indices become NA.
    pub fn select_indices(&self, indices: &[usize]) -> Integer {
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(indices.len());
        for &i in indices {
            if i < self.len() {
                builder.append_option(self.get_opt(i));
            } else {
                builder.append_null();
            }
        }
        Integer(builder.finish())
    }

    /// Set element `i`.
    ///
    /// Since Arrow arrays are immutable, this rebuilds the array.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    pub fn set(&mut self, i: usize, val: Option<i64>) {
        assert!(
            i < self.len(),
            "Integer::set: index {i} out of bounds (len {})",
            self.len()
        );
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(self.len());
        for j in 0..self.len() {
            if j == i {
                builder.append_option(val);
            } else {
                builder.append_option(self.get_opt(j));
            }
        }
        self.0 = builder.finish();
    }

    /// Push an element onto the end.
    pub fn push(&mut self, val: Option<i64>) {
        let new_len = self.len() + 1;
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(new_len);
        for j in 0..self.len() {
            builder.append_option(self.get_opt(j));
        }
        builder.append_option(val);
        self.0 = builder.finish();
    }

    /// Extend this buffer with elements from another.
    pub fn extend(&mut self, other: &Integer) {
        let new_len = self.len() + other.len();
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(new_len);
        for j in 0..self.len() {
            builder.append_option(self.get_opt(j));
        }
        for j in 0..other.len() {
            builder.append_option(other.get_opt(j));
        }
        self.0 = builder.finish();
    }

    /// Truncate the buffer to `len` elements.
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        self.0 = self.0.slice(0, len);
    }

    /// Reverse the buffer in-place.
    pub fn reverse(&mut self) {
        let len = self.len();
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(len);
        for i in (0..len).rev() {
            builder.append_option(self.get_opt(i));
        }
        self.0 = builder.finish();
    }

    /// Extract a sub-range as a new `Integer`.
    pub fn slice(&self, offset: usize, length: usize) -> Integer {
        Integer(self.0.slice(offset, length))
    }

    /// Access the underlying Arrow array.
    #[inline]
    pub fn arrow_array(&self) -> &Int64Array {
        &self.0
    }

    /// Create a new `Integer` of `len` elements, all NA.
    pub fn new_na(len: usize) -> Self {
        let mut builder = PrimitiveBuilder::<Int64Type>::with_capacity(len);
        for _ in 0..len {
            builder.append_null();
        }
        Integer(builder.finish())
    }
}

// region: Display / Debug / PartialEq

impl fmt::Debug for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Integer(")?;
        f.debug_list().entries(self.0.iter()).finish()?;
        write!(f, ")")
    }
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for i in 0..self.len() {
            let a = self.get_opt(i);
            let b = other.get_opt(i);
            if a != b {
                return false;
            }
        }
        true
    }
}

// endregion

// region: Conversions

impl From<Vec<Option<i64>>> for Integer {
    fn from(v: Vec<Option<i64>>) -> Self {
        Integer(Int64Array::from(v))
    }
}

impl From<Integer> for Vec<Option<i64>> {
    fn from(i: Integer) -> Self {
        i.into_vec()
    }
}

impl From<Int64Array> for Integer {
    fn from(arr: Int64Array) -> Self {
        Integer(arr)
    }
}

impl FromIterator<Option<i64>> for Integer {
    fn from_iter<I: IntoIterator<Item = Option<i64>>>(iter: I) -> Self {
        let arr: Int64Array = iter.into_iter().collect();
        Integer(arr)
    }
}

// endregion
