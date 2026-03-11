use derive_more::{Deref, DerefMut, From, Into};
use num_complex::Complex64;

/// Newtype for R complex vectors: `Vec<Option<Complex64>>` where `None` = NA.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, From, Into)]
pub struct ComplexVec(pub Vec<Option<Complex64>>);
