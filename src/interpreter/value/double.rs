use std::ops::{Deref, DerefMut};

use super::buffer::NullableBuffer;

/// Newtype for R double (numeric) vectors backed by arrow-style bitmap+buffer.
///
/// Wraps `NullableBuffer<f64>` where NA tracking uses a validity bitmap
/// instead of `Option<f64>` per element (halving memory for dense doubles).
#[derive(Debug, Clone, PartialEq)]
pub struct Double(pub NullableBuffer<f64>);

impl Deref for Double {
    type Target = NullableBuffer<f64>;
    fn deref(&self) -> &NullableBuffer<f64> {
        &self.0
    }
}

impl DerefMut for Double {
    fn deref_mut(&mut self) -> &mut NullableBuffer<f64> {
        &mut self.0
    }
}

impl From<Vec<Option<f64>>> for Double {
    fn from(v: Vec<Option<f64>>) -> Self {
        Double(NullableBuffer::from(v))
    }
}

impl From<Double> for Vec<Option<f64>> {
    fn from(d: Double) -> Self {
        d.0.into_vec()
    }
}

impl From<NullableBuffer<f64>> for Double {
    fn from(buf: NullableBuffer<f64>) -> Self {
        Double(buf)
    }
}
