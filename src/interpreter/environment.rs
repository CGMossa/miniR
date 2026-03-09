use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::interpreter::value::RValue;

#[derive(Debug, Clone)]
pub struct Environment {
    inner: Rc<RefCell<EnvInner>>,
}

#[derive(Debug)]
struct EnvInner {
    bindings: HashMap<String, RValue>,
    parent: Option<Environment>,
    #[allow(dead_code)]
    name: Option<String>,
}

impl Environment {
    pub fn new_global() -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: HashMap::new(),
                parent: None,
                name: Some("R_GlobalEnv".to_string()),
            })),
        }
    }

    pub fn new_child(parent: &Environment) -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: HashMap::new(),
                parent: Some(parent.clone()),
                name: None,
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
    pub fn set_super(&self, name: String, value: RValue) {
        let inner = self.inner.borrow();
        if let Some(ref parent) = inner.parent {
            // Walk up to find where it's defined
            if parent.has_local(&name) {
                parent.set(name, value);
            } else {
                parent.set_super(name, value);
            }
        } else {
            // At global scope, just set it here
            drop(inner);
            self.set(name, value);
        }
    }

    pub fn has_local(&self, name: &str) -> bool {
        self.inner.borrow().bindings.contains_key(name)
    }

    #[allow(dead_code)]
    pub fn remove(&self, name: &str) -> bool {
        self.inner.borrow_mut().bindings.remove(name).is_some()
    }

    pub fn new_empty() -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: HashMap::new(),
                parent: None,
                name: Some("R_EmptyEnv".to_string()),
            })),
        }
    }

    pub fn ls(&self) -> Vec<String> {
        let mut names: Vec<String> = self.inner.borrow().bindings.keys().cloned().collect();
        names.sort();
        names
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
}
