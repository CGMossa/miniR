use std::ops::{Deref, DerefMut};

use super::buffer::NullableBuffer;

/// Newtype for R integer vectors backed by arrow-style bitmap+buffer.
///
/// Wraps `NullableBuffer<i64>` where NA tracking uses a validity bitmap
/// instead of `Option<i64>` per element (halving memory for dense integers).
#[derive(Debug, Clone, PartialEq)]
pub struct Integer(pub NullableBuffer<i64>);

impl Deref for Integer {
    type Target = NullableBuffer<i64>;
    fn deref(&self) -> &NullableBuffer<i64> {
        &self.0
    }
}

impl DerefMut for Integer {
    fn deref_mut(&mut self) -> &mut NullableBuffer<i64> {
        &mut self.0
    }
}

impl From<Vec<Option<i64>>> for Integer {
    fn from(v: Vec<Option<i64>>) -> Self {
        Integer(NullableBuffer::from(v))
    }
}

impl From<Integer> for Vec<Option<i64>> {
    fn from(i: Integer) -> Self {
        i.0.into_vec()
    }
}

impl From<NullableBuffer<i64>> for Integer {
    fn from(buf: NullableBuffer<i64>) -> Self {
        Integer(buf)
    }
}
