use derive_more::{Deref, DerefMut, From, Into};

/// Newtype for R integer vectors: `Vec<Option<i64>>` where `None` = NA.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, From, Into)]
pub struct Integer(pub Vec<Option<i64>>);
