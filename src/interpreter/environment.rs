use std::cell::RefCell;
use std::rc::Rc;

use fnv::FnvHashMap;
use itertools::Itertools;
use tracing::trace;

use crate::interpreter::value::RValue;
use crate::parser::ast::Expr;

/// Source expressions for function arguments (promise expressions).
///
/// When a closure is called, the original unevaluated expressions for each
/// argument are stored here so that `substitute()` can retrieve them.
type PromiseExprs = FnvHashMap<String, Expr>;

#[derive(Debug, Clone)]
pub struct Environment {
    inner: Rc<RefCell<EnvInner>>,
}

impl Environment {
    /// Check if two environments are the same object (pointer equality).
    pub fn ptr_eq(&self, other: &Environment) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

#[derive(Debug)]
pub(crate) struct EnvInner {
    bindings: FnvHashMap<String, RValue>,
    parent: Option<Environment>,
    #[allow(dead_code)]
    name: Option<String>,
    /// Expressions registered via on.exit() to run when this frame exits.
    on_exit: Vec<Expr>,
    /// Whether the environment is locked (cannot add new bindings).
    locked: bool,
    /// Set of binding names that are individually locked (cannot be modified).
    locked_bindings: std::collections::HashSet<String>,
    /// Active bindings: names mapped to zero-argument functions that are called on every access.
    active_bindings: FnvHashMap<String, RValue>,
    /// Promise expressions: original unevaluated expressions for function arguments.
    /// Used by `substitute()` to retrieve the source expression for a parameter.
    promise_exprs: PromiseExprs,
}

impl Environment {
    pub fn new_global() -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: FnvHashMap::default(),
                parent: None,
                name: Some("R_GlobalEnv".to_string()),
                on_exit: Vec::new(),
                locked: false,
                locked_bindings: std::collections::HashSet::new(),
                active_bindings: FnvHashMap::default(),
                promise_exprs: FnvHashMap::default(),
            })),
        }
    }

    pub fn new_child(parent: &Environment) -> Self {
        trace!(
            "new child env (parent: {})",
            parent.name().as_deref().unwrap_or("<anonymous>")
        );
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: FnvHashMap::default(),
                parent: Some(parent.clone()),
                name: None,
                on_exit: Vec::new(),
                locked: false,
                locked_bindings: std::collections::HashSet::new(),
                active_bindings: FnvHashMap::default(),
                promise_exprs: FnvHashMap::default(),
            })),
        }
    }

    pub fn get(&self, name: &str) -> Option<RValue> {
        let inner = self.inner.borrow();
        if let Some(val) = inner.bindings.get(name) {
            Some(val.clone())
        } else if let Some(ref parent) = inner.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn set(&self, name: String, value: RValue) {
        self.inner.borrow_mut().bindings.insert(name, value);
    }

    /// Super-assignment: assign in parent environment (<<-)
    ///
    /// Walks up the environment chain looking for an existing binding.
    /// If found, overwrites it in place. If not found, creates the binding
    /// in the global environment (not base) — R treats global as the
    /// creation boundary for `<<-`.
    pub fn set_super(&self, name: String, value: RValue) {
        // At global level, <<- assigns locally — there's no enclosing function
        // scope to search. Without this, set_super recurses into base env.
        if self.is_global() {
            self.set(name, value);
            return;
        }
        let inner = self.inner.borrow();
        if let Some(ref parent) = inner.parent {
            if parent.has_local(&name) {
                parent.set(name, value);
            } else if parent.is_global() {
                // Reached global without finding the binding — create it here
                parent.set(name, value);
            } else {
                parent.set_super(name, value);
            }
        } else {
            // No parent at all (we ARE base) — set locally
            drop(inner);
            self.set(name, value);
        }
    }

    /// Returns true if this is the global environment.
    fn is_global(&self) -> bool {
        self.inner.borrow().name.as_deref() == Some("R_GlobalEnv")
    }

    pub fn has_local(&self, name: &str) -> bool {
        let inner = self.inner.borrow();
        inner.bindings.contains_key(name) || inner.active_bindings.contains_key(name)
    }

    pub fn remove(&self, name: &str) -> bool {
        let mut inner = self.inner.borrow_mut();
        let removed_binding = inner.bindings.remove(name).is_some();
        let removed_active = inner.active_bindings.remove(name).is_some();
        removed_binding || removed_active
    }

    /// Register an expression to run when this frame exits (on.exit).
    /// If `add` is false (default), replaces existing on.exit expressions.
    /// If `after` is true (default), appends after existing; if false, prepends before.
    pub fn push_on_exit(&self, expr: Expr, add: bool, after: bool) {
        let mut inner = self.inner.borrow_mut();
        if add {
            if after {
                inner.on_exit.push(expr);
            } else {
                inner.on_exit.insert(0, expr);
            }
        } else {
            inner.on_exit = vec![expr];
        }
    }

    /// Take all on.exit expressions (empties the list).
    pub fn take_on_exit(&self) -> Vec<Expr> {
        std::mem::take(&mut self.inner.borrow_mut().on_exit)
    }

    /// Return the currently registered on.exit expressions without clearing them.
    pub fn peek_on_exit(&self) -> Vec<Expr> {
        self.inner.borrow().on_exit.clone()
    }

    pub fn new_empty() -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: FnvHashMap::default(),
                parent: None,
                name: Some("R_EmptyEnv".to_string()),
                on_exit: Vec::new(),
                locked: false,
                locked_bindings: std::collections::HashSet::new(),
                active_bindings: FnvHashMap::default(),
                promise_exprs: FnvHashMap::default(),
            })),
        }
    }

    pub fn ls(&self) -> Vec<String> {
        let inner = self.inner.borrow();
        inner
            .bindings
            .keys()
            .chain(inner.active_bindings.keys())
            .cloned()
            .sorted()
            .collect()
    }

    /// Return all local (non-active) bindings as name-value pairs, sorted by name.
    pub fn local_bindings(&self) -> Vec<(String, RValue)> {
        let inner = self.inner.borrow();
        inner
            .bindings
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .sorted_by(|(a, _), (b, _)| a.cmp(b))
            .collect()
    }

    pub fn name(&self) -> Option<String> {
        self.inner.borrow().name.clone()
    }

    pub fn set_name(&self, name: String) {
        self.inner.borrow_mut().name = Some(name);
    }

    pub fn parent(&self) -> Option<Environment> {
        self.inner.borrow().parent.clone()
    }

    /// Set the parent (enclosing) environment.
    pub fn set_parent(&self, parent: Option<Environment>) {
        self.inner.borrow_mut().parent = parent;
    }

    /// Look up a name, skipping non-function values (like R's findFun).
    /// This implements R's behavior where `c(1,2)` still calls the `c` function
    /// even if `c` has been assigned a non-function value in the current env.
    pub fn get_function(&self, name: &str) -> Option<RValue> {
        let inner = self.inner.borrow();
        if let Some(val) = inner.bindings.get(name) {
            if matches!(val, RValue::Function(_)) {
                return Some(val.clone());
            }
        }
        if let Some(ref parent) = inner.parent {
            parent.get_function(name)
        } else {
            None
        }
    }

    /// Lock the environment so no new bindings can be added.
    /// If `bindings` is true, also lock all existing bindings.
    pub fn lock(&self, bindings: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.locked = true;
        if bindings {
            let names: Vec<String> = inner.bindings.keys().cloned().collect();
            for name in names {
                inner.locked_bindings.insert(name);
            }
        }
    }

    /// Return whether this environment is locked.
    pub fn is_locked(&self) -> bool {
        self.inner.borrow().locked
    }

    /// Lock a specific binding in this environment.
    pub fn lock_binding(&self, name: &str) {
        self.inner
            .borrow_mut()
            .locked_bindings
            .insert(name.to_string());
    }

    /// Return whether a specific binding is locked.
    pub fn binding_is_locked(&self, name: &str) -> bool {
        self.inner.borrow().locked_bindings.contains(name)
    }

    // region: Active bindings

    /// Register an active binding: a zero-argument function called on every access.
    pub fn set_active_binding(&self, name: String, fun: RValue) {
        let mut inner = self.inner.borrow_mut();
        // Remove any regular binding with the same name
        inner.bindings.remove(&name);
        inner.active_bindings.insert(name, fun);
    }

    /// Get the function for an active binding in this environment (local only).
    pub fn get_local_active_binding(&self, name: &str) -> Option<RValue> {
        self.inner.borrow().active_bindings.get(name).cloned()
    }

    /// Walk the environment chain looking for an active binding.
    /// Returns the function if found.
    pub fn get_active_binding(&self, name: &str) -> Option<RValue> {
        let inner = self.inner.borrow();
        if let Some(fun) = inner.active_bindings.get(name) {
            Some(fun.clone())
        } else if let Some(ref parent) = inner.parent {
            parent.get_active_binding(name)
        } else {
            None
        }
    }

    /// Check if a name is an active binding (walks the chain).
    pub fn is_active_binding(&self, name: &str) -> bool {
        let inner = self.inner.borrow();
        if inner.active_bindings.contains_key(name) {
            true
        } else if let Some(ref parent) = inner.parent {
            parent.is_active_binding(name)
        } else {
            false
        }
    }

    /// Check if a name is a local active binding (does not walk the chain).
    pub fn is_local_active_binding(&self, name: &str) -> bool {
        self.inner.borrow().active_bindings.contains_key(name)
    }

    // endregion

    // region: Promise expressions

    /// Store the original unevaluated expression for a function parameter.
    /// Used by `substitute()` to recover the source expression.
    pub fn set_promise_expr(&self, name: String, expr: Expr) {
        self.inner.borrow_mut().promise_exprs.insert(name, expr);
    }

    /// Get the original unevaluated expression for a function parameter.
    /// Checks this environment only (local), not parents.
    pub fn get_promise_expr(&self, name: &str) -> Option<Expr> {
        self.inner.borrow().promise_exprs.get(name).cloned()
    }

    // endregion
}
