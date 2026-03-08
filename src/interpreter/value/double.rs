use derive_more::{Deref, DerefMut, From, Into};

/// Newtype for R double (numeric) vectors: `Vec<Option<f64>>` where `None` = NA.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, From, Into)]
pub struct Double(pub Vec<Option<f64>>);
