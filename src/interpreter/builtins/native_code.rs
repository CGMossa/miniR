//! Native code builtins — .Call(), dyn.load(), dyn.unload(), etc.
//!
//! These replace the stubs in `stubs.rs` when the `native` feature is enabled.

use std::path::PathBuf;

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

// region: .Call

/// .Call — invoke a compiled C function via the native code pipeline.
///
/// The first argument is the function name (character string).
/// Remaining arguments are passed as SEXP values to the C function.
///
/// @param .NAME character string naming the C function
/// @param ... arguments passed to the native function
/// @return the value returned by the native function
/// @namespace base
#[interpreter_builtin(name = ".Call")]
fn builtin_dot_call(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    if args.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            ".Call requires at least one argument (the function name)".to_string(),
        ));
    }

    // First arg is the symbol name (character string)
    let symbol_name = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                ".Call: first argument must be a character string naming the C function"
                    .to_string(),
            )
        })?;

    // Remaining args are passed to the native function
    let native_args = &args[1..];

    ctx.interpreter().dot_call(&symbol_name, native_args)
}

// endregion

// region: dyn.load / dyn.unload

/// dyn.load — load a shared library (.so/.dylib).
///
/// @param x character string: path to the shared library
/// @param local logical: whether to use local scope (ignored)
/// @param now logical: whether to resolve symbols immediately (ignored)
/// @param ... additional arguments (ignored)
/// @return invisible NULL
/// @namespace base
#[interpreter_builtin(name = "dyn.load", min_args = 1)]
fn builtin_dyn_load(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "dyn.load: argument must be a file path (character string)".to_string(),
            )
        })?;

    let dll_path = PathBuf::from(&path);
    ctx.interpreter().dyn_load(&dll_path)?;
    Ok(RValue::Null)
}

/// dyn.unload — unload a shared library.
///
/// @param x character string: path to the shared library
/// @return invisible NULL
/// @namespace base
#[interpreter_builtin(name = "dyn.unload", min_args = 1)]
fn builtin_dyn_unload(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "dyn.unload: argument must be a file path (character string)".to_string(),
            )
        })?;

    // Extract the library name from the path
    let name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&path);

    ctx.interpreter().dyn_unload(name)?;
    Ok(RValue::Null)
}

/// library.dynam — load a package's compiled code.
///
/// Called by `library()` when a package has `useDynLib` in NAMESPACE.
/// Looks for the .so/.dylib in the package's `libs/` directory.
///
/// @param chname character: the shared library name (package name)
/// @param package character: the package name
/// @param lib.loc character: library path
/// @return invisible NULL
/// @namespace base
#[interpreter_builtin(name = "library.dynam", min_args = 1)]
fn builtin_library_dynam(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let chname = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "library.dynam: 'chname' must be a character string".to_string(),
            )
        })?;

    // Try to find the .so/.dylib in the package directory
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };

    // Search in loaded namespaces for the package directory
    let namespaces = ctx.interpreter().loaded_namespaces.borrow();
    if let Some(ns) = namespaces.get(&chname) {
        let lib_path = ns.lib_path.join("libs").join(format!("{chname}.{ext}"));
        if lib_path.is_file() {
            drop(namespaces);
            ctx.interpreter().dyn_load(&lib_path)?;
            return Ok(RValue::Null);
        }
    }
    drop(namespaces);

    // If not found via namespace, try a direct load (the path might be absolute)
    let direct = PathBuf::from(format!("{chname}.{ext}"));
    if direct.is_file() {
        ctx.interpreter().dyn_load(&direct)?;
    }

    Ok(RValue::Null)
}

/// library.dynam.unload — unload a package's compiled code.
/// @namespace base
#[interpreter_builtin(name = "library.dynam.unload", min_args = 1)]
fn builtin_library_dynam_unload(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_default();
    ctx.interpreter().dyn_unload(&name)?;
    Ok(RValue::Null)
}

// endregion

// region: Symbol inspection

/// is.loaded — check if a native symbol is loaded.
///
/// @param symbol character: the symbol name
/// @return logical
/// @namespace base
#[interpreter_builtin(name = "is.loaded", min_args = 1)]
fn builtin_is_loaded(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_default();
    let loaded = ctx.interpreter().is_symbol_loaded(&name);
    Ok(RValue::vec(Vector::Logical(vec![Some(loaded)].into())))
}

/// getNativeSymbolInfo — get info about a loaded native symbol.
/// @namespace base
#[interpreter_builtin(name = "getNativeSymbolInfo", min_args = 1)]
fn builtin_get_native_symbol_info(
    args: &[RValue],
    _named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_default();

    // Check if the symbol exists
    match ctx.interpreter().find_native_symbol(&name) {
        Ok(_) => {
            // Return a simple list with the symbol name
            // (full NativeSymbolInfo struct is complex — this is a minimal impl)
            Ok(RValue::List(RList::new(vec![(
                Some("name".to_string()),
                RValue::vec(Vector::Character(vec![Some(name)].into())),
            )])))
        }
        Err(e) => Err(e),
    }
}

// endregion
