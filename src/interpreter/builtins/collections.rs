//! Rust `std::collections` exposed as R data structures.
//!
//! Each collection is stored on the `Interpreter` struct in a `Vec<CollectionObject>`.
//! R sees a collection as an integer ID with a class attribute (e.g. `"hashmap"`,
//! `"btreemap"`, `"hashset"`, `"heap"`, `"deque"`). Functions like `hashmap_set()`,
//! `hashset_add()`, `heap_push()`, etc. mutate the collection in place through the ID.
//!
//! This gives R users O(1) hash lookups, ordered maps, priority queues, and deques —
//! data structures that have no native equivalent in base R.

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet, VecDeque};

use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::interpreter::Interpreter;
use minir_macros::interpreter_builtin;

// region: CollectionObject

/// A single collection object stored on the interpreter.
#[derive(Debug, Clone)]
pub enum CollectionObject {
    /// Unordered key-value store (`HashMap<String, RValue>`).
    HashMap(HashMap<String, RValue>),
    /// Ordered key-value store (`BTreeMap<String, RValue>`).
    BTreeMap(BTreeMap<String, RValue>),
    /// Unordered unique-element set (`HashSet<String>`).
    HashSet(HashSet<String>),
    /// Max-heap priority queue of `f64` values.
    BinaryHeap(BinaryHeap<OrdF64>),
    /// Double-ended queue of `RValue`.
    VecDeque(VecDeque<RValue>),
}

/// Wrapper around `f64` that implements `Ord` for use in `BinaryHeap`.
///
/// Uses `total_cmp` so NaN values have a deterministic position rather than
/// causing undefined ordering behavior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrdF64(pub f64);

impl Eq for OrdF64 {}

impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

// endregion

// region: Interpreter collection helpers

impl Interpreter {
    /// Allocate a new collection, returning its integer ID.
    pub(crate) fn add_collection(&self, obj: CollectionObject) -> usize {
        let mut collections = self.collections.borrow_mut();
        let id = collections.len();
        collections.push(obj);
        id
    }
}

// endregion

// region: Helpers

/// Build an integer scalar with a class attribute representing a collection.
fn collection_value(id: usize, class: &str) -> RValue {
    let mut rv = RVector::from(Vector::Integer(
        vec![Some(i64::try_from(id).unwrap_or(0))].into(),
    ));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some(class.to_string())].into())),
    );
    RValue::Vector(rv)
}

/// Extract a collection ID from an RValue (integer scalar, possibly with a class attribute).
fn collection_id(val: &RValue) -> Result<usize, RError> {
    val.as_vector()
        .and_then(|v| v.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "invalid collection handle — expected an integer ID returned by a collection constructor".to_string(),
            )
        })
}

/// Extract a string key from an argument at the given position.
fn require_string(args: &CallArgs, name: &str, pos: usize) -> Result<String, RError> {
    args.string(name, pos)
}

// endregion

// region: HashMap builtins

/// Create an empty HashMap (unordered key-value store).
///
/// Returns an integer ID with class "hashmap". Use `hashmap_set()`,
/// `hashmap_get()`, etc. to manipulate it.
///
/// @return integer scalar with class "hashmap"
#[interpreter_builtin(name = "hashmap", min_args = 0, max_args = 0)]
fn interp_hashmap(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let id = interp.add_collection(CollectionObject::HashMap(HashMap::new()));
    Ok(collection_value(id, "hashmap"))
}

/// Insert or update a key-value pair in a HashMap.
///
/// @param h integer scalar: hashmap ID
/// @param key character scalar: the key to set
/// @param value any R value to store
/// @return the previous value for the key, or NULL if the key was new
#[interpreter_builtin(name = "hashmap_set", min_args = 3, max_args = 3)]
fn interp_hashmap_set(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;
    let value = call_args.value("value", 2).cloned().unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::HashMap(map)) => {
            let old = map.insert(key, value);
            Ok(old.unwrap_or(RValue::Null))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Look up a key in a HashMap, with an optional default.
///
/// @param h integer scalar: hashmap ID
/// @param key character scalar: the key to look up
/// @param default value to return if the key is not found (default NULL)
/// @return the stored value, or `default` if the key does not exist
#[interpreter_builtin(name = "hashmap_get", min_args = 2, max_args = 3)]
fn interp_hashmap_get(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;
    let default = call_args
        .value("default", 2)
        .cloned()
        .unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => Ok(map.get(&key).cloned().unwrap_or(default)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Check whether a key exists in a HashMap.
///
/// @param h integer scalar: hashmap ID
/// @param key character scalar: the key to check
/// @return logical scalar: TRUE if the key exists, FALSE otherwise
#[interpreter_builtin(name = "hashmap_has", min_args = 2, max_args = 2)]
fn interp_hashmap_has(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => Ok(RValue::vec(Vector::Logical(
            vec![Some(map.contains_key(&key))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Remove a key from a HashMap, returning the old value.
///
/// @param h integer scalar: hashmap ID
/// @param key character scalar: the key to remove
/// @return the removed value, or NULL if the key did not exist
#[interpreter_builtin(name = "hashmap_remove", min_args = 2, max_args = 2)]
fn interp_hashmap_remove(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::HashMap(map)) => Ok(map.remove(&key).unwrap_or(RValue::Null)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Return all keys of a HashMap as a character vector.
///
/// The order of keys is not guaranteed (HashMap is unordered).
///
/// @param h integer scalar: hashmap ID
/// @return character vector of keys
#[interpreter_builtin(name = "hashmap_keys", min_args = 1, max_args = 1)]
fn interp_hashmap_keys(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => {
            let keys: Vec<Option<String>> = map.keys().map(|k| Some(k.clone())).collect();
            Ok(RValue::vec(Vector::Character(keys.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Return all values of a HashMap as a list.
///
/// The order of values corresponds to the (unordered) key iteration order.
///
/// @param h integer scalar: hashmap ID
/// @return list of values
#[interpreter_builtin(name = "hashmap_values", min_args = 1, max_args = 1)]
fn interp_hashmap_values(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => {
            let values: Vec<(Option<String>, RValue)> =
                map.values().map(|v| (None, v.clone())).collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Return the number of key-value pairs in a HashMap.
///
/// @param h integer scalar: hashmap ID
/// @return integer scalar: the number of entries
#[interpreter_builtin(name = "hashmap_size", min_args = 1, max_args = 1)]
fn interp_hashmap_size(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(map.len()).unwrap_or(i64::MAX))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

/// Convert a HashMap to a named R list.
///
/// Each key becomes a name in the list, each value becomes the corresponding element.
///
/// @param h integer scalar: hashmap ID
/// @return named list
#[interpreter_builtin(name = "hashmap_to_list", min_args = 1, max_args = 1)]
fn interp_hashmap_to_list(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashMap(map)) => {
            let entries: Vec<(Option<String>, RValue)> = map
                .iter()
                .map(|(k, v)| (Some(k.clone()), v.clone()))
                .collect();
            Ok(RValue::List(RList::new(entries)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashmap"),
        )),
    }
}

// endregion

// region: BTreeMap builtins

/// Create an empty BTreeMap (ordered key-value store).
///
/// Keys are always maintained in sorted order. Returns an integer ID with
/// class "btreemap".
///
/// @return integer scalar with class "btreemap"
#[interpreter_builtin(name = "btreemap", min_args = 0, max_args = 0)]
fn interp_btreemap(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let id = interp.add_collection(CollectionObject::BTreeMap(BTreeMap::new()));
    Ok(collection_value(id, "btreemap"))
}

/// Insert or update a key-value pair in a BTreeMap.
///
/// @param h integer scalar: btreemap ID
/// @param key character scalar: the key to set
/// @param value any R value to store
/// @return the previous value for the key, or NULL if the key was new
#[interpreter_builtin(name = "btreemap_set", min_args = 3, max_args = 3)]
fn interp_btreemap_set(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;
    let value = call_args.value("value", 2).cloned().unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::BTreeMap(map)) => {
            let old = map.insert(key, value);
            Ok(old.unwrap_or(RValue::Null))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Look up a key in a BTreeMap, with an optional default.
///
/// @param h integer scalar: btreemap ID
/// @param key character scalar: the key to look up
/// @param default value to return if the key is not found (default NULL)
/// @return the stored value, or `default` if the key does not exist
#[interpreter_builtin(name = "btreemap_get", min_args = 2, max_args = 3)]
fn interp_btreemap_get(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;
    let default = call_args
        .value("default", 2)
        .cloned()
        .unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => Ok(map.get(&key).cloned().unwrap_or(default)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Check whether a key exists in a BTreeMap.
///
/// @param h integer scalar: btreemap ID
/// @param key character scalar: the key to check
/// @return logical scalar: TRUE if the key exists, FALSE otherwise
#[interpreter_builtin(name = "btreemap_has", min_args = 2, max_args = 2)]
fn interp_btreemap_has(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => Ok(RValue::vec(Vector::Logical(
            vec![Some(map.contains_key(&key))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Remove a key from a BTreeMap, returning the old value.
///
/// @param h integer scalar: btreemap ID
/// @param key character scalar: the key to remove
/// @return the removed value, or NULL if the key did not exist
#[interpreter_builtin(name = "btreemap_remove", min_args = 2, max_args = 2)]
fn interp_btreemap_remove(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let key = require_string(&call_args, "key", 1)?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::BTreeMap(map)) => Ok(map.remove(&key).unwrap_or(RValue::Null)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Return all keys of a BTreeMap as a sorted character vector.
///
/// @param h integer scalar: btreemap ID
/// @return character vector of keys in sorted order
#[interpreter_builtin(name = "btreemap_keys", min_args = 1, max_args = 1)]
fn interp_btreemap_keys(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => {
            let keys: Vec<Option<String>> = map.keys().map(|k| Some(k.clone())).collect();
            Ok(RValue::vec(Vector::Character(keys.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Return all values of a BTreeMap as a list (in key-sorted order).
///
/// @param h integer scalar: btreemap ID
/// @return list of values in key-sorted order
#[interpreter_builtin(name = "btreemap_values", min_args = 1, max_args = 1)]
fn interp_btreemap_values(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => {
            let values: Vec<(Option<String>, RValue)> =
                map.values().map(|v| (None, v.clone())).collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Return the number of key-value pairs in a BTreeMap.
///
/// @param h integer scalar: btreemap ID
/// @return integer scalar: the number of entries
#[interpreter_builtin(name = "btreemap_size", min_args = 1, max_args = 1)]
fn interp_btreemap_size(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(map.len()).unwrap_or(i64::MAX))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

/// Convert a BTreeMap to a named R list (in key-sorted order).
///
/// @param h integer scalar: btreemap ID
/// @return named list with keys in sorted order
#[interpreter_builtin(name = "btreemap_to_list", min_args = 1, max_args = 1)]
fn interp_btreemap_to_list(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BTreeMap(map)) => {
            let entries: Vec<(Option<String>, RValue)> = map
                .iter()
                .map(|(k, v)| (Some(k.clone()), v.clone()))
                .collect();
            Ok(RValue::List(RList::new(entries)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a btreemap"),
        )),
    }
}

// endregion

// region: HashSet builtins

/// Create an empty HashSet (unordered unique-element set).
///
/// Returns an integer ID with class "hashset". Use `hashset_add()`,
/// `hashset_has()`, etc. to manipulate it.
///
/// @return integer scalar with class "hashset"
#[interpreter_builtin(name = "hashset", min_args = 0, max_args = 0)]
fn interp_hashset(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let id = interp.add_collection(CollectionObject::HashSet(HashSet::new()));
    Ok(collection_value(id, "hashset"))
}

/// Add a string element to a HashSet.
///
/// @param s integer scalar: hashset ID
/// @param value character scalar: the element to add
/// @return logical scalar: TRUE if the element was new, FALSE if already present
#[interpreter_builtin(name = "hashset_add", min_args = 2, max_args = 2)]
fn interp_hashset_add(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("s", 0).unwrap_or(&RValue::Null))?;
    let value = require_string(&call_args, "value", 1)?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::HashSet(set)) => {
            let was_new = set.insert(value);
            Ok(RValue::vec(Vector::Logical(vec![Some(was_new)].into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashset"),
        )),
    }
}

/// Check whether a string element is in a HashSet.
///
/// @param s integer scalar: hashset ID
/// @param value character scalar: the element to check
/// @return logical scalar: TRUE if present, FALSE otherwise
#[interpreter_builtin(name = "hashset_has", min_args = 2, max_args = 2)]
fn interp_hashset_has(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("s", 0).unwrap_or(&RValue::Null))?;
    let value = require_string(&call_args, "value", 1)?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashSet(set)) => Ok(RValue::vec(Vector::Logical(
            vec![Some(set.contains(&value))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashset"),
        )),
    }
}

/// Remove a string element from a HashSet.
///
/// @param s integer scalar: hashset ID
/// @param value character scalar: the element to remove
/// @return logical scalar: TRUE if the element was present and removed, FALSE otherwise
#[interpreter_builtin(name = "hashset_remove", min_args = 2, max_args = 2)]
fn interp_hashset_remove(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("s", 0).unwrap_or(&RValue::Null))?;
    let value = require_string(&call_args, "value", 1)?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::HashSet(set)) => {
            let was_present = set.remove(&value);
            Ok(RValue::vec(Vector::Logical(vec![Some(was_present)].into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashset"),
        )),
    }
}

/// Return the number of elements in a HashSet.
///
/// @param s integer scalar: hashset ID
/// @return integer scalar: the number of elements
#[interpreter_builtin(name = "hashset_size", min_args = 1, max_args = 1)]
fn interp_hashset_size(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("s", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashSet(set)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(set.len()).unwrap_or(i64::MAX))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashset"),
        )),
    }
}

/// Convert a HashSet to a character vector.
///
/// The order of elements is not guaranteed (HashSet is unordered).
///
/// @param s integer scalar: hashset ID
/// @return character vector of the set's elements
#[interpreter_builtin(name = "hashset_to_vector", min_args = 1, max_args = 1)]
fn interp_hashset_to_vector(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("s", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::HashSet(set)) => {
            let elems: Vec<Option<String>> = set.iter().map(|e| Some(e.clone())).collect();
            Ok(RValue::vec(Vector::Character(elems.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a hashset"),
        )),
    }
}

/// Compute the union of two HashSets, returning a new HashSet.
///
/// @param s1 integer scalar: first hashset ID
/// @param s2 integer scalar: second hashset ID
/// @return integer scalar with class "hashset": the union
#[interpreter_builtin(name = "hashset_union", min_args = 2, max_args = 2)]
fn interp_hashset_union(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id1 = collection_id(call_args.value("s1", 0).unwrap_or(&RValue::Null))?;
    let id2 = collection_id(call_args.value("s2", 1).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    let set1 = match collections.get(id1) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id1} is not a hashset"),
            ))
        }
    };
    let set2 = match collections.get(id2) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id2} is not a hashset"),
            ))
        }
    };
    let result: HashSet<String> = set1.union(set2).cloned().collect();
    drop(collections);
    let id = interp.add_collection(CollectionObject::HashSet(result));
    Ok(collection_value(id, "hashset"))
}

/// Compute the intersection of two HashSets, returning a new HashSet.
///
/// @param s1 integer scalar: first hashset ID
/// @param s2 integer scalar: second hashset ID
/// @return integer scalar with class "hashset": the intersection
#[interpreter_builtin(name = "hashset_intersect", min_args = 2, max_args = 2)]
fn interp_hashset_intersect(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id1 = collection_id(call_args.value("s1", 0).unwrap_or(&RValue::Null))?;
    let id2 = collection_id(call_args.value("s2", 1).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    let set1 = match collections.get(id1) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id1} is not a hashset"),
            ))
        }
    };
    let set2 = match collections.get(id2) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id2} is not a hashset"),
            ))
        }
    };
    let result: HashSet<String> = set1.intersection(set2).cloned().collect();
    drop(collections);
    let id = interp.add_collection(CollectionObject::HashSet(result));
    Ok(collection_value(id, "hashset"))
}

/// Compute the difference of two HashSets (s1 minus s2), returning a new HashSet.
///
/// @param s1 integer scalar: first hashset ID
/// @param s2 integer scalar: second hashset ID
/// @return integer scalar with class "hashset": elements in s1 but not in s2
#[interpreter_builtin(name = "hashset_diff", min_args = 2, max_args = 2)]
fn interp_hashset_diff(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id1 = collection_id(call_args.value("s1", 0).unwrap_or(&RValue::Null))?;
    let id2 = collection_id(call_args.value("s2", 1).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    let set1 = match collections.get(id1) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id1} is not a hashset"),
            ))
        }
    };
    let set2 = match collections.get(id2) {
        Some(CollectionObject::HashSet(s)) => s,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("collection {id2} is not a hashset"),
            ))
        }
    };
    let result: HashSet<String> = set1.difference(set2).cloned().collect();
    drop(collections);
    let id = interp.add_collection(CollectionObject::HashSet(result));
    Ok(collection_value(id, "hashset"))
}

// endregion

// region: BinaryHeap builtins

/// Create an empty max-heap (priority queue of numeric values).
///
/// Returns an integer ID with class "heap". Use `heap_push()`, `heap_pop()`,
/// etc. to manipulate it. The largest value is always at the top.
///
/// @return integer scalar with class "heap"
#[interpreter_builtin(name = "heap", min_args = 0, max_args = 0)]
fn interp_heap(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let id = interp.add_collection(CollectionObject::BinaryHeap(BinaryHeap::new()));
    Ok(collection_value(id, "heap"))
}

/// Push a numeric value onto a max-heap.
///
/// @param h integer scalar: heap ID
/// @param value numeric scalar: the value to push
/// @return NULL (invisibly)
#[interpreter_builtin(name = "heap_push", min_args = 2, max_args = 2)]
fn interp_heap_push(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;
    let val = call_args
        .value("value", 1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "heap_push() requires a numeric scalar value".to_string(),
            )
        })?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::BinaryHeap(heap)) => {
            heap.push(OrdF64(val));
            Ok(RValue::Null)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a heap"),
        )),
    }
}

/// Pop and return the maximum value from a max-heap.
///
/// Returns NULL if the heap is empty.
///
/// @param h integer scalar: heap ID
/// @return numeric scalar (the max value), or NULL if empty
#[interpreter_builtin(name = "heap_pop", min_args = 1, max_args = 1)]
fn interp_heap_pop(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::BinaryHeap(heap)) => match heap.pop() {
            Some(OrdF64(val)) => Ok(RValue::vec(Vector::Double(vec![Some(val)].into()))),
            None => Ok(RValue::Null),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a heap"),
        )),
    }
}

/// Peek at the maximum value in a max-heap without removing it.
///
/// Returns NULL if the heap is empty.
///
/// @param h integer scalar: heap ID
/// @return numeric scalar (the max value), or NULL if empty
#[interpreter_builtin(name = "heap_peek", min_args = 1, max_args = 1)]
fn interp_heap_peek(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BinaryHeap(heap)) => match heap.peek() {
            Some(OrdF64(val)) => Ok(RValue::vec(Vector::Double(vec![Some(*val)].into()))),
            None => Ok(RValue::Null),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a heap"),
        )),
    }
}

/// Return the number of elements in a max-heap.
///
/// @param h integer scalar: heap ID
/// @return integer scalar: the number of elements
#[interpreter_builtin(name = "heap_size", min_args = 1, max_args = 1)]
fn interp_heap_size(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("h", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::BinaryHeap(heap)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(heap.len()).unwrap_or(i64::MAX))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a heap"),
        )),
    }
}

// endregion

// region: VecDeque builtins

/// Create an empty deque (double-ended queue of R values).
///
/// Returns an integer ID with class "deque". Use `deque_push_back()`,
/// `deque_push_front()`, `deque_pop_back()`, `deque_pop_front()` to manipulate it.
///
/// @return integer scalar with class "deque"
#[interpreter_builtin(name = "deque", min_args = 0, max_args = 0)]
fn interp_deque(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let id = interp.add_collection(CollectionObject::VecDeque(VecDeque::new()));
    Ok(collection_value(id, "deque"))
}

/// Append a value to the back of a deque.
///
/// @param d integer scalar: deque ID
/// @param value any R value to append
/// @return NULL (invisibly)
#[interpreter_builtin(name = "deque_push_back", min_args = 2, max_args = 2)]
fn interp_deque_push_back(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;
    let value = call_args.value("value", 1).cloned().unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::VecDeque(deque)) => {
            deque.push_back(value);
            Ok(RValue::Null)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

/// Prepend a value to the front of a deque.
///
/// @param d integer scalar: deque ID
/// @param value any R value to prepend
/// @return NULL (invisibly)
#[interpreter_builtin(name = "deque_push_front", min_args = 2, max_args = 2)]
fn interp_deque_push_front(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;
    let value = call_args.value("value", 1).cloned().unwrap_or(RValue::Null);

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::VecDeque(deque)) => {
            deque.push_front(value);
            Ok(RValue::Null)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

/// Remove and return the last element of a deque.
///
/// Returns NULL if the deque is empty.
///
/// @param d integer scalar: deque ID
/// @return the removed value, or NULL if empty
#[interpreter_builtin(name = "deque_pop_back", min_args = 1, max_args = 1)]
fn interp_deque_pop_back(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::VecDeque(deque)) => Ok(deque.pop_back().unwrap_or(RValue::Null)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

/// Remove and return the first element of a deque.
///
/// Returns NULL if the deque is empty.
///
/// @param d integer scalar: deque ID
/// @return the removed value, or NULL if empty
#[interpreter_builtin(name = "deque_pop_front", min_args = 1, max_args = 1)]
fn interp_deque_pop_front(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let mut collections = interp.collections.borrow_mut();
    match collections.get_mut(id) {
        Some(CollectionObject::VecDeque(deque)) => Ok(deque.pop_front().unwrap_or(RValue::Null)),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

/// Return the number of elements in a deque.
///
/// @param d integer scalar: deque ID
/// @return integer scalar: the number of elements
#[interpreter_builtin(name = "deque_size", min_args = 1, max_args = 1)]
fn interp_deque_size(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::VecDeque(deque)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(deque.len()).unwrap_or(i64::MAX))].into(),
        ))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

/// Convert a deque to an R list.
///
/// @param d integer scalar: deque ID
/// @return list of the deque's elements (front to back)
#[interpreter_builtin(name = "deque_to_list", min_args = 1, max_args = 1)]
fn interp_deque_to_list(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let id = collection_id(call_args.value("d", 0).unwrap_or(&RValue::Null))?;

    let interp = context.interpreter();
    let collections = interp.collections.borrow();
    match collections.get(id) {
        Some(CollectionObject::VecDeque(deque)) => {
            let values: Vec<(Option<String>, RValue)> =
                deque.iter().map(|v| (None, v.clone())).collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("collection {id} is not a deque"),
        )),
    }
}

// endregion
