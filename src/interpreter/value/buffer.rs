//! Arrow-style nullable buffer: contiguous values + validity bitmap.
//!
//! Replaces `Vec<Option<T>>` with a separate bitmap for NA tracking.
//! Memory: 8 bytes per f64 + 1 bit per element (vs 16 bytes per `Option<f64>`).

use std::ops::RangeBounds;

// region: BitVec

/// Compact bit vector stored as packed u64 words.
///
/// Bit `i` is stored in `words[i / 64]` at bit position `i % 64`.
/// A set bit (1) means the value is valid; a clear bit (0) means NA.
#[derive(Clone, Debug)]
pub struct BitVec {
    words: Vec<u64>,
    len: usize,
}

impl BitVec {
    /// Create a new `BitVec` with all bits set (all valid).
    pub fn all_valid(len: usize) -> Self {
        let n_words = len.div_ceil(64);
        let mut words = vec![u64::MAX; n_words];
        // Clear any excess bits in the last word so count_ones is accurate
        let excess = n_words * 64 - len;
        if excess > 0 && !words.is_empty() {
            let last = words.len() - 1;
            words[last] = u64::MAX >> excess;
        }
        BitVec { words, len }
    }

    /// Create a new `BitVec` with all bits clear (all NA).
    pub fn all_na(len: usize) -> Self {
        let n_words = len.div_ceil(64);
        BitVec {
            words: vec![0; n_words],
            len,
        }
    }

    /// Number of bits.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if the bit vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get bit `i`. Returns `true` if the value is valid (not NA).
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn get(&self, i: usize) -> bool {
        assert!(
            i < self.len,
            "BitVec::get: index {i} out of bounds (len {})",
            self.len
        );
        let word = self.words[i / 64];
        (word >> (i % 64)) & 1 == 1
    }

    /// Set bit `i`. `val = true` means valid, `val = false` means NA.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn set(&mut self, i: usize, val: bool) {
        assert!(
            i < self.len,
            "BitVec::set: index {i} out of bounds (len {})",
            self.len
        );
        if val {
            self.words[i / 64] |= 1u64 << (i % 64);
        } else {
            self.words[i / 64] &= !(1u64 << (i % 64));
        }
    }

    /// Count the number of clear bits (NAs).
    pub fn count_zeros(&self) -> usize {
        self.len - self.count_ones()
    }

    /// Count the number of set bits (valid values).
    pub fn count_ones(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// True if all bits are set (no NAs).
    pub fn all_set(&self) -> bool {
        self.count_ones() == self.len
    }

    /// Push a new bit onto the end.
    pub fn push(&mut self, val: bool) {
        let bit_idx = self.len;
        self.len += 1;
        let word_idx = bit_idx / 64;
        if word_idx >= self.words.len() {
            self.words.push(0);
        }
        if val {
            self.words[word_idx] |= 1u64 << (bit_idx % 64);
        }
    }

    /// Extract a sub-range as a new BitVec.
    pub fn slice(&self, start: usize, end: usize) -> Self {
        assert!(start <= end && end <= self.len);
        let new_len = end - start;
        let mut result = BitVec::all_na(new_len);
        for i in 0..new_len {
            if self.get(start + i) {
                result.set(i, true);
            }
        }
        result
    }
}

impl PartialEq for BitVec {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        // Compare word-by-word (excess bits are already cleared)
        self.words == other.words
    }
}

impl Eq for BitVec {}

// endregion

// region: NullableBuffer

/// A contiguous buffer of values with a separate validity bitmap for NA tracking.
///
/// When `validity` is `None`, all values are valid (common case optimization).
/// NA positions hold `Default::default()` in the values buffer.
#[derive(Clone, Debug)]
pub struct NullableBuffer<T: Clone + Default> {
    /// Dense values. NA positions hold `Default::default()`.
    values: Vec<T>,
    /// Validity bitmap: bit `i` is 1 if `values[i]` is valid, 0 if NA.
    /// `None` means all values are valid (common case optimization).
    validity: Option<BitVec>,
}

impl<T: Clone + Default> NullableBuffer<T> {
    /// Create a buffer of `len` elements, all NA.
    pub fn new(len: usize) -> Self {
        NullableBuffer {
            values: vec![T::default(); len],
            validity: Some(BitVec::all_na(len)),
        }
    }

    /// Create a buffer from values with no NAs.
    pub fn from_values(values: Vec<T>) -> Self {
        NullableBuffer {
            values,
            validity: None,
        }
    }

    /// Number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.len() == 0
    }

    /// Get element `i`. Returns `None` if NA, `Some(&value)` if valid.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn get(&self, i: usize) -> Option<&T> {
        if self.is_na(i) {
            None
        } else {
            Some(&self.values[i])
        }
    }

    /// Set element `i`.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    pub fn set(&mut self, i: usize, val: Option<T>) {
        match val {
            Some(v) => {
                self.values[i] = v;
                if let Some(ref mut bv) = self.validity {
                    bv.set(i, true);
                }
                // if validity is None, all are valid already
            }
            None => {
                self.values[i] = T::default();
                self.ensure_validity();
                if let Some(ref mut bv) = self.validity {
                    bv.set(i, false);
                }
            }
        }
    }

    /// True if element `i` is NA.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn is_na(&self, i: usize) -> bool {
        match &self.validity {
            None => false,
            Some(bv) => !bv.get(i),
        }
    }

    /// Number of NA values.
    pub fn na_count(&self) -> usize {
        match &self.validity {
            None => 0,
            Some(bv) => bv.count_zeros(),
        }
    }

    /// True if there are any NAs.
    pub fn has_na(&self) -> bool {
        match &self.validity {
            None => false,
            Some(bv) => !bv.all_set(),
        }
    }

    /// Iterate yielding `Option<&T>` (None for NA positions).
    pub fn iter(&self) -> NullableIter<'_, T> {
        NullableIter { buf: self, pos: 0 }
    }

    /// Convert to `Vec<Option<T>>` (for backward compatibility).
    pub fn into_vec(self) -> Vec<Option<T>> {
        match self.validity {
            None => self.values.into_iter().map(Some).collect(),
            Some(bv) => self
                .values
                .into_iter()
                .enumerate()
                .map(|(i, v)| if bv.get(i) { Some(v) } else { None })
                .collect(),
        }
    }

    /// Borrow as `Vec<Option<T>>` (allocates).
    pub fn to_option_vec(&self) -> Vec<Option<T>> {
        match &self.validity {
            None => self.values.iter().cloned().map(Some).collect(),
            Some(bv) => self
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| if bv.get(i) { Some(v.clone()) } else { None })
                .collect(),
        }
    }

    /// Raw values slice. NA positions hold `Default::default()`.
    #[inline]
    pub fn values_slice(&self) -> &[T] {
        &self.values
    }

    /// Mutable access to the raw values slice.
    #[inline]
    pub fn values_slice_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    /// Access the validity bitmap (None means all valid).
    #[inline]
    pub fn validity(&self) -> Option<&BitVec> {
        self.validity.as_ref()
    }

    /// Extract a sub-range as a new `NullableBuffer`.
    pub fn slice<R: RangeBounds<usize>>(&self, range: R) -> NullableBuffer<T> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(&s) => s,
            std::ops::Bound::Excluded(&s) => s + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(&e) => e + 1,
            std::ops::Bound::Excluded(&e) => e,
            std::ops::Bound::Unbounded => self.len(),
        };
        assert!(start <= end && end <= self.len());
        let values = self.values[start..end].to_vec();
        let validity = self.validity.as_ref().map(|bv| bv.slice(start, end));
        // Optimize: if validity is all-valid after slicing, drop it
        let validity = match validity {
            Some(bv) if bv.all_set() => None,
            other => other,
        };
        NullableBuffer { values, validity }
    }

    /// Ensure the validity bitmap exists (lazily allocate when first NA is added).
    fn ensure_validity(&mut self) {
        if self.validity.is_none() {
            self.validity = Some(BitVec::all_valid(self.len()));
        }
    }

    /// Reverse the buffer in-place.
    pub fn reverse(&mut self) {
        self.values.reverse();
        if let Some(ref bv) = self.validity {
            // Rebuild the bitmap in reverse
            let len = bv.len();
            let mut new_bv = BitVec::all_na(len);
            for i in 0..len {
                if bv.get(i) {
                    new_bv.set(len - 1 - i, true);
                }
            }
            self.validity = Some(new_bv);
        }
    }

    /// Extend this buffer with elements from another.
    pub fn extend(&mut self, other: &NullableBuffer<T>) {
        for i in 0..other.len() {
            if other.is_na(i) {
                self.push(None);
            } else {
                self.push(Some(other.values[i].clone()));
            }
        }
    }

    /// Truncate the buffer to `len` elements.
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        self.values.truncate(len);
        if let Some(ref mut bv) = self.validity {
            // Rebuild bitmap at new length
            let new_bv = bv.slice(0, len);
            *bv = new_bv;
            // Optimize: drop bitmap if all valid now
            if bv.all_set() {
                self.validity = None;
            }
        }
    }

    /// Push an element onto the end.
    pub fn push(&mut self, val: Option<T>) {
        match val {
            Some(v) => {
                self.values.push(v);
                if let Some(ref mut bv) = self.validity {
                    bv.push(true);
                }
            }
            None => {
                // Ensure bitmap exists before pushing, so it covers existing valid elements.
                self.ensure_validity();
                self.values.push(T::default());
                if let Some(ref mut bv) = self.validity {
                    bv.push(false);
                }
            }
        }
    }
}

impl<T: Clone + Default + Copy> NullableBuffer<T> {
    /// Get element `i` as `Option<T>` by value (for Copy types).
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub fn get_opt(&self, i: usize) -> Option<T> {
        if self.is_na(i) {
            None
        } else {
            Some(self.values[i])
        }
    }

    /// Get the first element as `Option<T>`.
    pub fn first_opt(&self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.get_opt(0)
        }
    }

    /// Iterate yielding `Option<T>` by value (for Copy types).
    /// This is more convenient than `iter()` when you need owned values.
    pub fn iter_opt(&self) -> impl Iterator<Item = Option<T>> + Clone + '_ {
        (0..self.len()).map(move |i| self.get_opt(i))
    }

    /// Collect indices, producing a new `NullableBuffer`. Out-of-bounds indices become NA.
    pub fn select_indices(&self, indices: &[usize]) -> NullableBuffer<T> {
        let mut values = Vec::with_capacity(indices.len());
        let mut has_na = false;
        for &i in indices {
            if i < self.len() {
                if let Some(v) = self.get_opt(i) {
                    values.push(Some(v));
                } else {
                    values.push(None);
                    has_na = true;
                }
            } else {
                values.push(None);
                has_na = true;
            }
        }
        if has_na {
            NullableBuffer::from(values)
        } else {
            NullableBuffer::from_values(values.into_iter().map(|v| v.unwrap_or_default()).collect())
        }
    }
}

// endregion

// region: Iterator

/// Iterator over `NullableBuffer` yielding `Option<&T>`.
#[derive(Clone)]
pub struct NullableIter<'a, T: Clone + Default> {
    buf: &'a NullableBuffer<T>,
    pos: usize,
}

impl<'a, T: Clone + Default> Iterator for NullableIter<'a, T> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buf.len() {
            None
        } else {
            let i = self.pos;
            self.pos += 1;
            Some(self.buf.get(i))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buf.len() - self.pos;
        (remaining, Some(remaining))
    }
}

impl<T: Clone + Default> ExactSizeIterator for NullableIter<'_, T> {}

// endregion

// region: Conversions

impl<T: Clone + Default> From<Vec<Option<T>>> for NullableBuffer<T> {
    fn from(v: Vec<Option<T>>) -> Self {
        let len = v.len();
        let mut values = Vec::with_capacity(len);
        let mut has_na = false;

        for item in &v {
            match item {
                Some(val) => values.push(val.clone()),
                None => {
                    values.push(T::default());
                    has_na = true;
                }
            }
        }

        let validity = if has_na {
            let mut bv = BitVec::all_valid(len);
            for (i, item) in v.iter().enumerate() {
                if item.is_none() {
                    bv.set(i, false);
                }
            }
            Some(bv)
        } else {
            None
        };

        NullableBuffer { values, validity }
    }
}

impl<T: Clone + Default> From<NullableBuffer<T>> for Vec<Option<T>> {
    fn from(buf: NullableBuffer<T>) -> Self {
        buf.into_vec()
    }
}

impl<T: Clone + Default + PartialEq> PartialEq for NullableBuffer<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for i in 0..self.len() {
            let a_na = self.is_na(i);
            let b_na = other.is_na(i);
            if a_na != b_na {
                return false;
            }
            if !a_na && self.values[i] != other.values[i] {
                return false;
            }
        }
        true
    }
}

// endregion

// region: IntoIterator

/// Owned iterator over `NullableBuffer` yielding `Option<T>`.
pub struct NullableIntoIter<T: Clone + Default> {
    values: std::vec::IntoIter<T>,
    validity: Option<BitVec>,
    pos: usize,
}

impl<T: Clone + Default> Iterator for NullableIntoIter<T> {
    type Item = Option<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.values.next()?;
        let i = self.pos;
        self.pos += 1;
        match &self.validity {
            None => Some(Some(val)),
            Some(bv) => {
                if bv.get(i) {
                    Some(Some(val))
                } else {
                    Some(None)
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T: Clone + Default> ExactSizeIterator for NullableIntoIter<T> {}

impl<T: Clone + Default> IntoIterator for NullableBuffer<T> {
    type Item = Option<T>;
    type IntoIter = NullableIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        NullableIntoIter {
            values: self.values.into_iter(),
            validity: self.validity,
            pos: 0,
        }
    }
}

impl<T: Clone + Default> FromIterator<Option<T>> for NullableBuffer<T> {
    fn from_iter<I: IntoIterator<Item = Option<T>>>(iter: I) -> Self {
        let v: Vec<Option<T>> = iter.into_iter().collect();
        NullableBuffer::from(v)
    }
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    // region: BitVec tests

    #[test]
    fn bitvec_all_valid() {
        let bv = BitVec::all_valid(100);
        assert_eq!(bv.len(), 100);
        assert_eq!(bv.count_ones(), 100);
        assert_eq!(bv.count_zeros(), 0);
        assert!(bv.all_set());
        for i in 0..100 {
            assert!(bv.get(i));
        }
    }

    #[test]
    fn bitvec_all_na() {
        let bv = BitVec::all_na(100);
        assert_eq!(bv.len(), 100);
        assert_eq!(bv.count_ones(), 0);
        assert_eq!(bv.count_zeros(), 100);
        assert!(!bv.all_set());
        for i in 0..100 {
            assert!(!bv.get(i));
        }
    }

    #[test]
    fn bitvec_set_and_get() {
        let mut bv = BitVec::all_na(10);
        bv.set(0, true);
        bv.set(5, true);
        bv.set(9, true);
        assert!(bv.get(0));
        assert!(!bv.get(1));
        assert!(bv.get(5));
        assert!(!bv.get(6));
        assert!(bv.get(9));
        assert_eq!(bv.count_ones(), 3);
        assert_eq!(bv.count_zeros(), 7);
    }

    #[test]
    fn bitvec_push() {
        let mut bv = BitVec::all_valid(0);
        bv.push(true);
        bv.push(false);
        bv.push(true);
        assert_eq!(bv.len(), 3);
        assert!(bv.get(0));
        assert!(!bv.get(1));
        assert!(bv.get(2));
    }

    #[test]
    fn bitvec_equality() {
        let a = BitVec::all_valid(64);
        let b = BitVec::all_valid(64);
        assert_eq!(a, b);
        let mut c = BitVec::all_valid(64);
        c.set(0, false);
        assert_ne!(a, c);
    }

    #[test]
    fn bitvec_slice() {
        let mut bv = BitVec::all_valid(10);
        bv.set(3, false);
        bv.set(7, false);
        let sliced = bv.slice(2, 8);
        assert_eq!(sliced.len(), 6);
        assert!(sliced.get(0)); // original index 2
        assert!(!sliced.get(1)); // original index 3 (NA)
        assert!(sliced.get(2)); // original index 4
        assert!(!sliced.get(5)); // original index 7 (NA)
    }

    // endregion

    // region: NullableBuffer tests

    #[test]
    fn buffer_from_values_no_na() {
        let buf = NullableBuffer::from_values(vec![1.0, 2.0, 3.0]);
        assert_eq!(buf.len(), 3);
        assert!(!buf.has_na());
        assert_eq!(buf.na_count(), 0);
        assert_eq!(buf.get(0), Some(&1.0));
        assert_eq!(buf.get(1), Some(&2.0));
        assert_eq!(buf.get(2), Some(&3.0));
    }

    #[test]
    fn buffer_from_option_vec() {
        let buf: NullableBuffer<f64> = vec![Some(1.0), None, Some(3.0)].into();
        assert_eq!(buf.len(), 3);
        assert!(buf.has_na());
        assert_eq!(buf.na_count(), 1);
        assert_eq!(buf.get(0), Some(&1.0));
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), Some(&3.0));
    }

    #[test]
    fn buffer_all_na() {
        let buf: NullableBuffer<i64> = NullableBuffer::new(3);
        assert_eq!(buf.len(), 3);
        assert!(buf.has_na());
        assert_eq!(buf.na_count(), 3);
        assert_eq!(buf.get(0), None);
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), None);
    }

    #[test]
    fn buffer_set() {
        let mut buf: NullableBuffer<f64> = NullableBuffer::from_values(vec![1.0, 2.0, 3.0]);
        buf.set(1, None);
        assert!(buf.is_na(1));
        assert_eq!(buf.get(1), None);
        buf.set(1, Some(42.0));
        assert!(!buf.is_na(1));
        assert_eq!(buf.get(1), Some(&42.0));
    }

    #[test]
    fn buffer_iter() {
        let buf: NullableBuffer<i64> = vec![Some(10), None, Some(30)].into();
        let collected: Vec<Option<&i64>> = buf.iter().collect();
        assert_eq!(collected, vec![Some(&10), None, Some(&30)]);
    }

    #[test]
    fn buffer_into_vec() {
        let buf: NullableBuffer<f64> = vec![Some(1.0), None, Some(3.0)].into();
        let v: Vec<Option<f64>> = buf.into_vec();
        assert_eq!(v, vec![Some(1.0), None, Some(3.0)]);
    }

    #[test]
    fn buffer_into_vec_no_na() {
        let buf = NullableBuffer::from_values(vec![1.0, 2.0]);
        let v: Vec<Option<f64>> = buf.into_vec();
        assert_eq!(v, vec![Some(1.0), Some(2.0)]);
    }

    #[test]
    fn buffer_get_opt() {
        let buf: NullableBuffer<i64> = vec![Some(10), None, Some(30)].into();
        assert_eq!(buf.get_opt(0), Some(10));
        assert_eq!(buf.get_opt(1), None);
        assert_eq!(buf.get_opt(2), Some(30));
    }

    #[test]
    fn buffer_first_opt() {
        let buf: NullableBuffer<f64> = vec![Some(1.0), None].into();
        assert_eq!(buf.first_opt(), Some(1.0));

        let buf2: NullableBuffer<f64> = vec![None, Some(2.0)].into();
        assert_eq!(buf2.first_opt(), None);

        let buf3: NullableBuffer<f64> = NullableBuffer::from_values(vec![]);
        assert_eq!(buf3.first_opt(), None);
    }

    #[test]
    fn buffer_slice() {
        let buf: NullableBuffer<i64> = vec![Some(1), Some(2), None, Some(4), Some(5)].into();
        let sub = buf.slice(1..4);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.get_opt(0), Some(2));
        assert_eq!(sub.get_opt(1), None);
        assert_eq!(sub.get_opt(2), Some(4));
    }

    #[test]
    fn buffer_push() {
        let mut buf = NullableBuffer::<i64>::from_values(vec![1, 2]);
        buf.push(Some(3));
        buf.push(None);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.get_opt(2), Some(3));
        assert_eq!(buf.get_opt(3), None);
    }

    #[test]
    fn buffer_equality() {
        let a: NullableBuffer<f64> = vec![Some(1.0), None, Some(3.0)].into();
        let b: NullableBuffer<f64> = vec![Some(1.0), None, Some(3.0)].into();
        assert_eq!(a, b);

        let c = NullableBuffer::from_values(vec![1.0, 2.0]);
        let d = NullableBuffer::from_values(vec![1.0, 2.0]);
        assert_eq!(c, d);

        assert_ne!(a, c);
    }

    #[test]
    fn buffer_into_iter() {
        let buf: NullableBuffer<i64> = vec![Some(1), None, Some(3)].into();
        let collected: Vec<Option<i64>> = buf.into_iter().collect();
        assert_eq!(collected, vec![Some(1), None, Some(3)]);
    }

    #[test]
    fn buffer_from_iter() {
        let buf: NullableBuffer<f64> = vec![Some(1.0), None].into_iter().collect();
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get_opt(0), Some(1.0));
        assert_eq!(buf.get_opt(1), None);
    }

    #[test]
    fn buffer_select_indices() {
        let buf: NullableBuffer<i64> = vec![Some(10), Some(20), None, Some(40)].into();
        let selected = buf.select_indices(&[3, 0, 2, 99]);
        assert_eq!(selected.len(), 4);
        assert_eq!(selected.get_opt(0), Some(40));
        assert_eq!(selected.get_opt(1), Some(10));
        assert_eq!(selected.get_opt(2), None); // was NA
        assert_eq!(selected.get_opt(3), None); // out of bounds
    }

    // endregion
}
