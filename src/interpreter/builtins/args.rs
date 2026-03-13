//! Shared decoding helpers for builtin positional and named arguments.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::{RError, RErrorKind, RValue};

pub(crate) struct CallArgs<'a> {
    positional: &'a [RValue],
    named: &'a [(String, RValue)],
}

impl<'a> CallArgs<'a> {
    pub(crate) fn new(positional: &'a [RValue], named: &'a [(String, RValue)]) -> Self {
        Self { positional, named }
    }

    pub(crate) fn positional(&self, index: usize) -> Option<&'a RValue> {
        self.positional.get(index)
    }

    pub(crate) fn named(&self, name: &str) -> Option<&'a RValue> {
        self.named
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value)
    }

    pub(crate) fn value(&self, name: &str, position: usize) -> Option<&'a RValue> {
        self.named(name).or_else(|| self.positional(position))
    }

    pub(crate) fn string(&self, name: &str, position: usize) -> Result<String, RError> {
        self.value(name, position)
            .and_then(|value| value.as_vector()?.as_character_scalar())
            .ok_or_else(|| RError::new(RErrorKind::Argument, format!("invalid '{name}' argument")))
    }

    pub(crate) fn optional_string(&self, name: &str, position: usize) -> Option<String> {
        self.value(name, position)
            .and_then(|value| value.as_vector()?.as_character_scalar())
    }

    pub(crate) fn named_string(&self, name: &str) -> Option<String> {
        self.named(name)
            .and_then(|value| value.as_vector()?.as_character_scalar())
    }

    pub(crate) fn logical_flag(&self, name: &str, position: usize, default: bool) -> bool {
        self.value(name, position)
            .and_then(|value| value.as_vector()?.as_logical_scalar())
            .unwrap_or(default)
    }

    pub(crate) fn integer_or(&self, name: &str, position: usize, default: i64) -> i64 {
        self.value(name, position)
            .and_then(|value| value.as_vector()?.as_integer_scalar())
            .unwrap_or(default)
    }

    pub(crate) fn environment_or(
        &self,
        name: &str,
        position: usize,
        default: &Environment,
    ) -> Result<Environment, RError> {
        match self.value(name, position) {
            Some(RValue::Environment(env)) => Ok(env.clone()),
            Some(_) => Err(RError::new(
                RErrorKind::Argument,
                format!("invalid '{name}' argument"),
            )),
            None => Ok(default.clone()),
        }
    }
}
