use derive_more::{Deref, DerefMut, From, Into};

/// Newtype for R logical vectors: `Vec<Option<bool>>` where `None` = NA.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, From, Into)]
pub struct Logical(pub Vec<Option<bool>>);
