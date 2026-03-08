use derive_more::{Deref, DerefMut, From, Into};

/// Newtype for R character vectors: `Vec<Option<String>>` where `None` = NA.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, From, Into)]
pub struct Character(pub Vec<Option<String>>);
