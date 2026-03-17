//! Package loading runtime — discovers, loads, and attaches R packages.
//!
//! Implements the core package loading sequence:
//! 1. Find the package directory on `.libPaths()`
//! 2. Parse DESCRIPTION and NAMESPACE
//! 3. Create a namespace environment (parent = base env)
//! 4. Source all `R/*.R` files into the namespace env
//! 5. Build an exports environment (filtered view of namespace)
//! 6. For `library()`, attach the exports env to the search path
//! 7. Register S3 methods declared in NAMESPACE
//! 8. Run `.onLoad()` and `.onAttach()` hooks

use std::path::{Path, PathBuf};

use crate::interpreter::environment::Environment;
use crate::interpreter::value::{RError, RErrorKind, RValue, Vector};
use crate::interpreter::Interpreter;

use super::description::PackageDescription;
use super::namespace::PackageNamespace;

/// State for a single loaded package namespace.
#[derive(Debug, Clone)]
pub struct LoadedNamespace {
    /// Package name.
    pub name: String,
    /// The directory the package was loaded from.
    pub lib_path: PathBuf,
    /// Parsed DESCRIPTION metadata.
    pub description: PackageDescription,
    /// Parsed NAMESPACE directives.
    pub namespace: PackageNamespace,
    /// The namespace environment (all package code lives here).
    pub namespace_env: Environment,
    /// The exports environment (user-visible subset attached to search path).
    pub exports_env: Environment,
}

/// An entry on the search path. In R, the search path is:
/// `.GlobalEnv` -> `package:foo` -> `package:bar` -> ... -> `package:base`
#[derive(Debug, Clone)]
pub struct SearchPathEntry {
    /// Display name, e.g. "package:dplyr" or ".GlobalEnv".
    pub name: String,
    /// The environment on the search path.
    pub env: Environment,
}

impl Interpreter {
    /// Find a package directory by name, searching `.libPaths()`.
    ///
    /// Returns the path to the package directory (e.g. `/path/to/lib/myPkg/`)
    /// if found, or None if the package is not installed in any library path.
    pub(crate) fn find_package_dir(&self, pkg_name: &str) -> Option<PathBuf> {
        let lib_paths = self.get_lib_paths();
        for lib_path in &lib_paths {
            let pkg_dir = Path::new(lib_path).join(pkg_name);
            // A valid package directory must contain a DESCRIPTION file
            if pkg_dir.join("DESCRIPTION").is_file() {
                return Some(pkg_dir);
            }
        }
        None
    }

    /// Get the library search paths (same logic as `.libPaths()` builtin).
    pub(crate) fn get_lib_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = Vec::new();
        let sep = if cfg!(windows) { ';' } else { ':' };

        if let Some(r_libs) = self.get_env_var("R_LIBS") {
            for p in r_libs.split(sep) {
                let p = p.trim();
                if !p.is_empty() {
                    let resolved = self.resolve_path(p);
                    if resolved.is_dir() {
                        paths.push(resolved.to_string_lossy().to_string());
                    }
                }
            }
        }

        if let Some(r_libs_user) = self.get_env_var("R_LIBS_USER") {
            for p in r_libs_user.split(sep) {
                let p = p.trim();
                if !p.is_empty() {
                    let resolved = self.resolve_path(p);
                    if resolved.is_dir() {
                        paths.push(resolved.to_string_lossy().to_string());
                    }
                }
            }
        }

        paths
    }

    /// Load a package namespace without attaching it to the search path.
    ///
    /// This is the core of `loadNamespace()`. It:
    /// 1. Finds the package on `.libPaths()`
    /// 2. Parses DESCRIPTION and NAMESPACE
    /// 3. Creates a namespace environment
    /// 4. Sources R files
    /// 5. Builds exports
    /// 6. Registers S3 methods
    /// 7. Calls `.onLoad()`
    ///
    /// Returns the namespace environment.
    pub(crate) fn load_namespace(&self, pkg_name: &str) -> Result<Environment, RError> {
        // Check if already loaded
        if let Some(ns) = self.loaded_namespaces.borrow().get(pkg_name) {
            return Ok(ns.namespace_env.clone());
        }

        let pkg_dir = self.find_package_dir(pkg_name).ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                format!(
                    "there is no package called '{pkg_name}'\n  \
                     Hint: check that the package is installed in one of the library paths \
                     returned by .libPaths()"
                ),
            )
        })?;

        self.load_namespace_from_dir(pkg_name, &pkg_dir)
    }

    /// Load a namespace from a specific directory.
    fn load_namespace_from_dir(
        &self,
        pkg_name: &str,
        pkg_dir: &Path,
    ) -> Result<Environment, RError> {
        // Parse DESCRIPTION
        let desc_path = pkg_dir.join("DESCRIPTION");
        let desc_text = std::fs::read_to_string(&desc_path).map_err(|e| {
            RError::other(format!(
                "cannot read DESCRIPTION for package '{}': {}",
                pkg_name, e
            ))
        })?;
        let description = PackageDescription::parse(&desc_text).map_err(|e| {
            RError::other(format!(
                "cannot parse DESCRIPTION for package '{}': {}",
                pkg_name, e
            ))
        })?;

        // Parse NAMESPACE
        let ns_path = pkg_dir.join("NAMESPACE");
        let namespace = if ns_path.is_file() {
            let ns_text = std::fs::read_to_string(&ns_path).map_err(|e| {
                RError::other(format!(
                    "cannot read NAMESPACE for package '{}': {}",
                    pkg_name, e
                ))
            })?;
            PackageNamespace::parse(&ns_text).map_err(|e| {
                RError::other(format!(
                    "cannot parse NAMESPACE for package '{}': {}",
                    pkg_name, e
                ))
            })?
        } else {
            // Packages without NAMESPACE export everything (legacy behavior)
            PackageNamespace::default()
        };

        // Load dependencies from Imports
        for dep in &description.imports {
            if dep.package == "R" || is_base_package(&dep.package) {
                continue;
            }
            // Silently skip unresolvable imports for now — they may be
            // packages we can't load (native deps, etc.)
            let _ = self.load_namespace(&dep.package);
        }

        // Load Depends (non-R) namespaces too
        for dep in &description.depends {
            if dep.package == "R" || is_base_package(&dep.package) {
                continue;
            }
            let _ = self.load_namespace(&dep.package);
        }

        // Create namespace environment with base env as parent
        let base_env = self.base_env();
        let namespace_env = Environment::new_child(&base_env);
        namespace_env.set_name(format!("namespace:{}", pkg_name));

        // Populate imports into the namespace env
        self.populate_imports(&namespace, &namespace_env)?;

        // Source all R files from the R/ directory
        let r_dir = pkg_dir.join("R");
        if r_dir.is_dir() {
            self.source_r_directory(&r_dir, &namespace_env)?;
        }

        // Build exports environment
        let exports_env = Environment::new_child(&base_env);
        exports_env.set_name(format!("package:{}", pkg_name));
        self.build_exports(&namespace, &namespace_env, &exports_env);

        // Register S3 methods declared in NAMESPACE
        self.register_s3_methods(&namespace, &namespace_env);

        // Store the loaded namespace
        let loaded = LoadedNamespace {
            name: pkg_name.to_string(),
            lib_path: pkg_dir.to_path_buf(),
            description,
            namespace,
            namespace_env: namespace_env.clone(),
            exports_env,
        };
        self.loaded_namespaces
            .borrow_mut()
            .insert(pkg_name.to_string(), loaded);

        // Call .onLoad() if it exists
        if let Some(on_load) = namespace_env.get(".onLoad") {
            let lib_path_str = pkg_dir
                .parent()
                .unwrap_or(pkg_dir)
                .to_string_lossy()
                .to_string();
            let lib_val = RValue::vec(Vector::Character(vec![Some(lib_path_str)].into()));
            let pkg_val = RValue::vec(Vector::Character(vec![Some(pkg_name.to_string())].into()));
            // Best-effort: ignore errors from .onLoad
            let _ = self.call_function(&on_load, &[lib_val, pkg_val], &[], &namespace_env);
        }

        Ok(namespace_env)
    }

    /// Get the base environment (root of the environment chain).
    pub(crate) fn base_env(&self) -> Environment {
        let mut current = self.global_env.clone();
        while let Some(parent) = current.parent() {
            current = parent;
        }
        current
    }

    /// Populate the namespace environment with imports from other packages.
    fn populate_imports(
        &self,
        namespace: &PackageNamespace,
        namespace_env: &Environment,
    ) -> Result<(), RError> {
        // Handle `import(pkg)` — import all exports from a package
        for pkg_name in &namespace.imports {
            if is_base_package(pkg_name) {
                // Base package bindings are already accessible through the parent chain
                continue;
            }
            if let Some(ns) = self.loaded_namespaces.borrow().get(pkg_name) {
                // Copy all exports into our namespace
                for name in ns.exports_env.ls() {
                    if let Some(val) = ns.exports_env.get(&name) {
                        namespace_env.set(name, val);
                    }
                }
            }
        }

        // Handle `importFrom(pkg, sym)` — import specific symbols
        for (pkg_name, sym_name) in &namespace.imports_from {
            if is_base_package(pkg_name) {
                // Try to get from base env
                let base = self.base_env();
                if let Some(val) = base.get(sym_name) {
                    namespace_env.set(sym_name.clone(), val);
                }
                continue;
            }
            if let Some(ns) = self.loaded_namespaces.borrow().get(pkg_name) {
                // Try exports first, then namespace
                if let Some(val) = ns.exports_env.get(sym_name) {
                    namespace_env.set(sym_name.clone(), val);
                } else if let Some(val) = ns.namespace_env.get(sym_name) {
                    namespace_env.set(sym_name.clone(), val);
                }
            }
        }

        Ok(())
    }

    /// Source all .R files from a directory into an environment.
    fn source_r_directory(&self, r_dir: &Path, env: &Environment) -> Result<(), RError> {
        let mut r_files: Vec<PathBuf> = Vec::new();

        let entries = std::fs::read_dir(r_dir).map_err(|e| {
            RError::other(format!(
                "cannot read R/ directory '{}': {}",
                r_dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry
                .map_err(|e| RError::other(format!("error reading R/ directory entry: {}", e)))?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if ext_lower == "r" || ext_lower == "s" {
                    r_files.push(path);
                }
            }
        }

        // Sort for deterministic load order (R sorts alphabetically)
        r_files.sort();

        let mut errors = Vec::new();
        for r_file in &r_files {
            if let Err(e) = self.source_file_into(r_file, env) {
                errors.push(format!("{}: {}", r_file.display(), e));
            }
        }

        if !errors.is_empty() {
            return Err(RError::other(format!(
                "errors sourcing R files:\n  {}",
                errors.join("\n  ")
            )));
        }

        Ok(())
    }

    /// Source a single R file into an environment.
    fn source_file_into(&self, path: &Path, env: &Environment) -> Result<(), RError> {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                let bytes = std::fs::read(path).map_err(|e2| {
                    RError::other(format!("cannot read '{}': {}", path.display(), e2))
                })?;
                String::from_utf8_lossy(&bytes).into_owned()
            }
            Err(e) => {
                return Err(RError::other(format!(
                    "cannot read '{}': {}",
                    path.display(),
                    e
                )));
            }
        };

        let ast = crate::parser::parse_program(&source)
            .map_err(|e| RError::other(format!("parse error in '{}': {}", path.display(), e)))?;

        self.eval_in(&ast, env).map_err(RError::from)?;
        Ok(())
    }

    /// Build the exports environment from namespace directives.
    fn build_exports(
        &self,
        namespace: &PackageNamespace,
        namespace_env: &Environment,
        exports_env: &Environment,
    ) {
        // Handle explicit exports
        for name in &namespace.exports {
            if let Some(val) = namespace_env.get(name) {
                exports_env.set(name.clone(), val);
            }
        }

        // Handle exportPattern — match regex against all namespace bindings
        let patterns: Vec<regex::Regex> = namespace
            .export_patterns
            .iter()
            .filter_map(|pat| regex::Regex::new(pat).ok())
            .collect();

        if !patterns.is_empty() {
            for name in namespace_env.ls() {
                if patterns.iter().any(|pat| pat.is_match(&name)) {
                    if let Some(val) = namespace_env.get(&name) {
                        exports_env.set(name, val);
                    }
                }
            }
        }

        // If no export directives at all, export everything (legacy packages
        // without NAMESPACE or with empty NAMESPACE)
        if namespace.exports.is_empty() && namespace.export_patterns.is_empty() {
            for name in namespace_env.ls() {
                if let Some(val) = namespace_env.get(&name) {
                    exports_env.set(name, val);
                }
            }
        }
    }

    /// Register S3 methods declared in NAMESPACE into the base environment
    /// so they're discoverable by S3 dispatch.
    fn register_s3_methods(&self, namespace: &PackageNamespace, namespace_env: &Environment) {
        let base_env = self.base_env();

        for reg in &namespace.s3_methods {
            let method_name = reg
                .method
                .clone()
                .unwrap_or_else(|| format!("{}.{}", reg.generic, reg.class));

            // Look up the method function in the namespace
            if let Some(method_fn) = namespace_env.get(&method_name) {
                // Register as generic.class in base so S3 dispatch finds it
                let dispatch_name = format!("{}.{}", reg.generic, reg.class);
                base_env.set(dispatch_name, method_fn);
            }
        }
    }

    /// Attach a loaded package to the search path.
    ///
    /// Inserts the package's exports environment right after `.GlobalEnv`
    /// in the environment parent chain, and adds it to the search path list.
    pub(crate) fn attach_package(&self, pkg_name: &str) -> Result<(), RError> {
        let loaded = self
            .loaded_namespaces
            .borrow()
            .get(pkg_name)
            .cloned()
            .ok_or_else(|| {
                RError::other(format!(
                    "namespace '{}' is not loaded — cannot attach",
                    pkg_name
                ))
            })?;

        let entry_name = format!("package:{}", pkg_name);

        // Check if already on search path
        {
            let sp = self.search_path.borrow();
            if sp.iter().any(|e| e.name == entry_name) {
                return Ok(());
            }
        }

        // Insert between global env and its current parent.
        // R's search path: global -> pkg1 -> pkg2 -> ... -> base
        let current_parent = self.global_env.parent();
        loaded.exports_env.set_parent(current_parent);
        self.global_env.set_parent(Some(loaded.exports_env.clone()));

        // Add to search path list
        self.search_path.borrow_mut().push(SearchPathEntry {
            name: entry_name,
            env: loaded.exports_env.clone(),
        });

        // Call .onAttach() if it exists
        let namespace_env = loaded.namespace_env.clone();
        let lib_path = loaded.lib_path.clone();
        if let Some(on_attach) = namespace_env.get(".onAttach") {
            let lib_path_str = lib_path
                .parent()
                .unwrap_or(&lib_path)
                .to_string_lossy()
                .to_string();
            let lib_val = RValue::vec(Vector::Character(vec![Some(lib_path_str)].into()));
            let pkg_val = RValue::vec(Vector::Character(vec![Some(pkg_name.to_string())].into()));
            // Best-effort: ignore errors from .onAttach
            let _ = self.call_function(&on_attach, &[lib_val, pkg_val], &[], &namespace_env);
        }

        Ok(())
    }

    /// Detach a package from the search path by name (e.g. "package:dplyr").
    pub(crate) fn detach_package(&self, entry_name: &str) -> Result<(), RError> {
        let mut sp = self.search_path.borrow_mut();
        let idx = sp
            .iter()
            .position(|e| e.name == entry_name)
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    format!(
                        "invalid 'name' argument: '{}' not found on search path",
                        entry_name
                    ),
                )
            })?;

        let entry = sp.remove(idx);

        // Rewire the environment parent chain: find who points to this env
        // and make them point to this env's parent instead.
        let detached_parent = entry.env.parent();

        // Walk from global env to find the env whose parent is entry.env
        let mut current = self.global_env.clone();
        loop {
            let parent = current.parent();
            match parent {
                Some(ref p) if p.ptr_eq(&entry.env) => {
                    current.set_parent(detached_parent);
                    break;
                }
                Some(p) => current = p,
                None => break,
            }
        }

        Ok(())
    }

    /// Get the search path as a vector of names.
    pub(crate) fn get_search_path(&self) -> Vec<String> {
        let mut result = vec![".GlobalEnv".to_string()];
        for entry in self.search_path.borrow().iter() {
            result.push(entry.name.clone());
        }
        result.push("package:base".to_string());
        result
    }
}

/// Check if a package name refers to a "base" package that's always available.
fn is_base_package(name: &str) -> bool {
    matches!(
        name,
        "base"
            | "utils"
            | "stats"
            | "grDevices"
            | "graphics"
            | "methods"
            | "datasets"
            | "tools"
            | "compiler"
            | "grid"
            | "splines"
            | "parallel"
            | "tcltk"
    )
}
