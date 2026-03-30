//! R double (numeric) vectors backed by Apache Arrow `Float64Array`.
//!
//! The Arrow array provides: contiguous f64 buffer + validity bitmap for NA tracking.
//! Memory layout matches the old `NullableBuffer<f64>`: 8 bytes per f64 + 1 bit per element.

use std::fmt;

use arrow_array::builder::PrimitiveBuilder;
use arrow_array::types::Float64Type;
use arrow_array::{Array, Float64Array};

/// Newtype for R double (numeric) vectors backed by Arrow `Float64Array`.
///
/// Wraps `Float64Array` where NA tracking uses a validity bitmap
/// instead of `Option<f64>` per element (halving memory for dense doubles).
#[derive(Clone)]
pub struct Double(pub Float64Array);

impl Double {
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

    /// Get element `i` as `Option<f64>` by value.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn get_opt(&self, i: usize) -> Option<f64> {
        if self.0.is_null(i) {
            None
        } else {
            Some(self.0.value(i))
        }
    }

    /// Get the first element as `Option<f64>`.
    pub fn first_opt(&self) -> Option<f64> {
        if self.is_empty() {
            None
        } else {
            self.get_opt(0)
        }
    }

    /// Iterate yielding `Option<f64>` by value.
    pub fn iter_opt(&self) -> impl Iterator<Item = Option<f64>> + Clone + '_ {
        self.0.iter()
    }

    /// Iterate yielding `Option<f64>` by value (alias for `iter_opt`).
    ///
    /// Matches the iteration signature of the old `NullableBuffer::iter()`.
    pub fn iter(&self) -> impl Iterator<Item = Option<f64>> + Clone + '_ {
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

    /// Convert to `Vec<Option<f64>>`.
    pub fn into_vec(self) -> Vec<Option<f64>> {
        self.0.iter().collect()
    }

    /// Borrow as `Vec<Option<f64>>` (allocates).
    pub fn to_option_vec(&self) -> Vec<Option<f64>> {
        self.0.iter().collect()
    }

    /// Raw values slice. NA positions hold arbitrary values.
    #[inline]
    pub fn values_slice(&self) -> &[f64] {
        self.0.values().as_ref()
    }

    /// Create a buffer from values with no NAs.
    pub fn from_values(values: Vec<f64>) -> Self {
        Double(Float64Array::from(values))
    }

    /// Collect indices, producing a new `Double`. Out-of-bounds indices become NA.
    pub fn select_indices(&self, indices: &[usize]) -> Double {
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(indices.len());
        for &i in indices {
            if i < self.len() {
                builder.append_option(self.get_opt(i));
            } else {
                builder.append_null();
            }
        }
        Double(builder.finish())
    }

    /// Set element `i`.
    ///
    /// Since Arrow arrays are immutable, this rebuilds the array.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    pub fn set(&mut self, i: usize, val: Option<f64>) {
        assert!(
            i < self.len(),
            "Double::set: index {i} out of bounds (len {})",
            self.len()
        );
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(self.len());
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
    pub fn push(&mut self, val: Option<f64>) {
        let new_len = self.len() + 1;
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(new_len);
        for j in 0..self.len() {
            builder.append_option(self.get_opt(j));
        }
        builder.append_option(val);
        self.0 = builder.finish();
    }

    /// Extend this buffer with elements from another.
    pub fn extend(&mut self, other: &Double) {
        let new_len = self.len() + other.len();
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(new_len);
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
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(len);
        for i in (0..len).rev() {
            builder.append_option(self.get_opt(i));
        }
        self.0 = builder.finish();
    }

    /// Extract a sub-range as a new `Double`.
    pub fn slice(&self, offset: usize, length: usize) -> Double {
        Double(self.0.slice(offset, length))
    }

    /// Access the underlying Arrow array.
    #[inline]
    pub fn arrow_array(&self) -> &Float64Array {
        &self.0
    }

    /// Create a new `Double` of `len` elements, all NA.
    pub fn new_na(len: usize) -> Self {
        let mut builder = PrimitiveBuilder::<Float64Type>::with_capacity(len);
        for _ in 0..len {
            builder.append_null();
        }
        Double(builder.finish())
    }
}

// region: Display / Debug / PartialEq

impl fmt::Debug for Double {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Double(")?;
        f.debug_list().entries(self.0.iter()).finish()?;
        write!(f, ")")
    }
}

impl PartialEq for Double {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for i in 0..self.len() {
            let a = self.get_opt(i);
            let b = other.get_opt(i);
            match (a, b) {
                (None, None) => continue,
                (Some(x), Some(y)) if x.to_bits() == y.to_bits() => continue,
                _ => return false,
            }
        }
        true
    }
}

// endregion

// region: Conversions

impl From<Vec<Option<f64>>> for Double {
    fn from(v: Vec<Option<f64>>) -> Self {
        Double(Float64Array::from(v))
    }
}

impl From<Double> for Vec<Option<f64>> {
    fn from(d: Double) -> Self {
        d.into_vec()
    }
}

impl From<Float64Array> for Double {
    fn from(arr: Float64Array) -> Self {
        Double(arr)
    }
}

impl FromIterator<Option<f64>> for Double {
    fn from_iter<I: IntoIterator<Item = Option<f64>>>(iter: I) -> Self {
        let arr: Float64Array = iter.into_iter().collect();
        Double(arr)
    }
}

// endregion
