//! System, file, directory, path, and temp builtins.
//!
//! Each function is auto-registered via `#[builtin]` + linkme.

use crate::interpreter::coerce::*;
use crate::interpreter::value::*;
use crate::interpreter::{BuiltinContext, Interpreter};
use derive_more::{Display, Error};
use itertools::Itertools;
use minir_macros::{builtin, interpreter_builtin};
use std::fs;
use std::path::Path;

// region: SystemError

/// Structured error type for system/file operations.
#[derive(Debug, Display, Error)]
#[allow(dead_code)]
pub enum SystemError {
    #[display("cannot copy '{}' to '{}': {}", from, to, source)]
    Copy {
        from: String,
        to: String,
        source: std::io::Error,
    },
    #[display("cannot rename '{}' to '{}': {}", from, to, source)]
    Rename {
        from: String,
        to: String,
        source: std::io::Error,
    },
    #[display("cannot remove '{}': {}", path, source)]
    Remove {
        path: String,
        source: std::io::Error,
    },
    #[display("cannot create directory '{}': {}", path, source)]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[display("cannot read directory '{}': {}", path, source)]
    ReadDir {
        path: String,
        source: std::io::Error,
    },
    #[display("cannot execute command '{}': {}", command, source)]
    Command {
        command: String,
        source: std::io::Error,
    },
    #[display("cannot get current directory: {}", source)]
    GetCwd {
        #[error(source)]
        source: std::io::Error,
    },
    #[display("cannot set current directory '{}': {}", path, source)]
    SetCwd {
        path: String,
        source: std::io::Error,
    },
}

impl From<SystemError> for RError {
    fn from(e: SystemError) -> Self {
        RError::from_source(RErrorKind::Other, e)
    }
}

// endregion

fn resolved_path_string(interp: &Interpreter, path: &str) -> String {
    interp.resolve_path(path).to_string_lossy().to_string()
}

fn home_dir_string(interp: &Interpreter) -> Option<String> {
    interp
        .get_env_var("HOME")
        .or_else(|| interp.get_env_var("USERPROFILE"))
        .or_else(|| {
            #[cfg(feature = "dirs-support")]
            {
                dirs::home_dir().map(|home| home.to_string_lossy().to_string())
            }
            #[cfg(not(feature = "dirs-support"))]
            {
                None
            }
        })
}

fn minir_data_dir(interp: &Interpreter) -> String {
    #[cfg(feature = "dirs-support")]
    {
        if let Some(data) = dirs::data_dir() {
            return data.join("miniR").to_string_lossy().to_string();
        }
    }

    home_dir_string(interp)
        .map(|h| format!("{}/.miniR", h))
        .unwrap_or_else(|| "/tmp/miniR".to_string())
}

// === File operations ===

/// Copy a file from one path to another.
///
/// @param from character scalar: source file path
/// @param to character scalar: destination file path
/// @return logical scalar: TRUE on success
#[interpreter_builtin(name = "file.copy", min_args = 2)]
fn builtin_file_copy(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let from = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'from' must be a character string".to_string(),
            )
        })?;
    let to = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'to' must be a character string".to_string(),
            )
        })?;

    let from = resolved_path_string(context.interpreter(), &from);
    let to = resolved_path_string(context.interpreter(), &to);

    match fs::copy(&from, &to) {
        Ok(_) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => Err(SystemError::Copy { from, to, source }.into()),
    }
}

/// Create empty files at the given paths.
///
/// @param ... character scalars: file paths to create
/// @return logical vector: TRUE for each file successfully created
#[interpreter_builtin(name = "file.create", min_args = 1)]
fn builtin_file_create(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            let path = resolved_path_string(context.interpreter(), &path);
            match fs::File::create(&path) {
                Ok(_) => Some(true),
                Err(_) => Some(false),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

/// Delete files at the given paths.
///
/// @param ... character scalars: file paths to remove
/// @return logical vector: TRUE for each file successfully removed
#[interpreter_builtin(name = "file.remove", min_args = 1)]
fn builtin_file_remove(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            let path = resolved_path_string(context.interpreter(), &path);
            match fs::remove_file(&path) {
                Ok(()) => Some(true),
                Err(_) => Some(false),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

/// Rename (move) a file.
///
/// @param from character scalar: current file path
/// @param to character scalar: new file path
/// @return logical scalar: TRUE on success
#[interpreter_builtin(name = "file.rename", min_args = 2)]
fn builtin_file_rename(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let from = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'from' must be a character string".to_string(),
            )
        })?;
    let to = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'to' must be a character string".to_string(),
            )
        })?;

    let from = resolved_path_string(context.interpreter(), &from);
    let to = resolved_path_string(context.interpreter(), &to);

    match fs::rename(&from, &to) {
        Ok(()) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => Err(SystemError::Rename { from, to, source }.into()),
    }
}

/// Get the size of files in bytes.
///
/// @param ... character scalars: file paths to query
/// @return double vector of file sizes (NA for non-existent files)
#[interpreter_builtin(name = "file.size", min_args = 1)]
fn builtin_file_size(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let results: Vec<Option<f64>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            let path = resolved_path_string(context.interpreter(), &path);
            fs::metadata(&path).ok().map(|m| u64_to_f64(m.len()))
        })
        .collect();
    Ok(RValue::vec(Vector::Double(results.into())))
}

/// Get file modification times as POSIXct timestamps (seconds since Unix epoch).
///
/// @param ... character scalars: file paths to query
/// @return double vector of modification times (NA for non-existent files)
#[interpreter_builtin(name = "file.mtime", min_args = 1)]
fn builtin_file_mtime(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let results: Vec<Option<f64>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            let path = resolved_path_string(context.interpreter(), &path);
            fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
        })
        .collect();
    let mut rv = RVector::from(Vector::Double(results.into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("POSIXct".to_string()), Some("POSIXt".to_string())].into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

/// Delete files or directories.
///
/// @param x character scalar: path to remove
/// @param recursive logical: if TRUE, remove directories recursively (default FALSE)
/// @return integer scalar: 0 on success, 1 on failure
#[interpreter_builtin(name = "unlink", min_args = 1)]
fn builtin_unlink(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'x' must be a character string".to_string(),
            )
        })?;
    let recursive = named
        .iter()
        .find(|(n, _)| n == "recursive")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let path = resolved_path_string(context.interpreter(), &path);
    let p = Path::new(&path);
    let result = if p.is_dir() {
        if recursive {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_dir(&path)
        }
    } else {
        fs::remove_file(&path)
    };

    match result {
        Ok(()) => Ok(RValue::vec(Vector::Integer(vec![Some(0)].into()))),
        Err(_) => Ok(RValue::vec(Vector::Integer(vec![Some(1)].into()))),
    }
}

// region: file.info

/// Get detailed file metadata (size, type, permissions, timestamps).
///
/// @param ... character scalars: file paths to query
/// @return data.frame with columns: size, isdir, mode, mtime, ctime, atime
#[interpreter_builtin(name = "file.info", min_args = 1)]
fn builtin_file_info(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let paths: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .map(|path| resolved_path_string(context.interpreter(), &path))
        .collect();

    if paths.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "'...' must contain at least one file path".to_string(),
        ));
    }

    let mut sizes: Vec<Option<f64>> = Vec::new();
    let mut isdirs: Vec<Option<bool>> = Vec::new();
    let mut modes: Vec<Option<i64>> = Vec::new();
    let mut mtimes: Vec<Option<f64>> = Vec::new();
    let mut ctimes: Vec<Option<f64>> = Vec::new();
    let mut atimes: Vec<Option<f64>> = Vec::new();

    for path in &paths {
        match fs::metadata(path) {
            Ok(meta) => {
                sizes.push(Some(u64_to_f64(meta.len())));
                isdirs.push(Some(meta.is_dir()));

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode_u32 = meta.permissions().mode() & 0o777;
                    modes.push(Some(i64::from(mode_u32)));
                }
                #[cfg(not(unix))]
                {
                    modes.push(Some(0o644));
                }

                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64());
                mtimes.push(mtime);

                let ctime = meta
                    .created()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64());
                ctimes.push(ctime);

                let atime = meta
                    .accessed()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64());
                atimes.push(atime);
            }
            Err(_) => {
                sizes.push(None);
                isdirs.push(None);
                modes.push(None);
                mtimes.push(None);
                ctimes.push(None);
                atimes.push(None);
            }
        }
    }

    let row_names: Vec<Option<String>> = paths.into_iter().map(Some).collect();
    let mut list = RList::new(vec![
        (
            Some("size".to_string()),
            RValue::vec(Vector::Double(sizes.into())),
        ),
        (
            Some("isdir".to_string()),
            RValue::vec(Vector::Logical(isdirs.into())),
        ),
        (
            Some("mode".to_string()),
            RValue::vec(Vector::Integer(modes.into())),
        ),
        (
            Some("mtime".to_string()),
            RValue::vec(Vector::Double(mtimes.into())),
        ),
        (
            Some("ctime".to_string()),
            RValue::vec(Vector::Double(ctimes.into())),
        ),
        (
            Some("atime".to_string()),
            RValue::vec(Vector::Double(atimes.into())),
        ),
    ]);

    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Character(row_names.into())),
    );

    Ok(RValue::List(list))
}
// endregion

// === Directory operations ===

/// Create a directory, optionally with parent directories.
///
/// @param path character scalar: directory path to create
/// @param showWarnings logical: if TRUE (default), warn on failure instead of erroring
/// @param recursive logical: if TRUE, create parent directories as needed (default TRUE,
///   diverging from R's default of FALSE for better ergonomics)
/// @return logical scalar: TRUE on success, FALSE on failure (when showWarnings is TRUE)
#[interpreter_builtin(name = "dir.create", min_args = 1)]
fn builtin_dir_create(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'path' must be a character string".to_string(),
            )
        })?;

    let show_warnings = named
        .iter()
        .find(|(n, _)| n == "showWarnings")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    // miniR diverges from R: recursive = TRUE by default
    let recursive = named
        .iter()
        .find(|(n, _)| n == "recursive")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let path = resolved_path_string(context.interpreter(), &path);

    let result = if recursive {
        fs::create_dir_all(&path)
    } else {
        fs::create_dir(&path)
    };

    match result {
        Ok(()) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => {
            if show_warnings {
                // Return FALSE with a warning (matching R behavior for showWarnings=TRUE)
                // In R, this issues a warning rather than an error. We return FALSE to
                // signal failure without aborting.
                Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
            } else {
                Err(SystemError::CreateDir { path, source }.into())
            }
        }
    }
}

/// Test whether directories exist at the given paths.
///
/// @param ... character scalars: directory paths to check
/// @return logical vector indicating existence of each directory
#[interpreter_builtin(name = "dir.exists", min_args = 1)]
fn builtin_dir_exists(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            let path = resolved_path_string(context.interpreter(), &path);
            Some(Path::new(&path).is_dir())
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

/// List files in a directory, optionally filtering by pattern.
///
/// @param path character scalar: directory path (default ".")
/// @param pattern character scalar: regex pattern to filter file names
/// @param all.files logical: if TRUE, include hidden files starting with "." (default FALSE)
/// @param full.names logical: if TRUE, return full paths (default FALSE)
/// @param recursive logical: if TRUE, recurse into subdirectories (default FALSE).
///   When the `walkdir-support` feature is enabled, uses walkdir for efficient
///   recursive traversal.
/// @return character vector of matching file names (sorted)
#[interpreter_builtin(name = "list.files", names = ["dir"])]
fn builtin_list_files(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| ".".to_string());
    let path = resolved_path_string(context.interpreter(), &path);

    let pattern = named
        .iter()
        .find(|(n, _)| n == "pattern")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());

    let all_files = named
        .iter()
        .find(|(n, _)| n == "all.files")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let recursive = named
        .iter()
        .find(|(n, _)| n == "recursive")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let full_names = named
        .iter()
        .find(|(n, _)| n == "full.names")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let regex = match &pattern {
        Some(pat) => Some(regex::Regex::new(pat).map_err(|source| -> RError {
            super::strings::StringError::InvalidRegex { source }.into()
        })?),
        None => None,
    };

    let result = if recursive {
        list_files_recursive(&path, &regex, all_files, full_names)?
    } else {
        list_files_flat(&path, &regex, all_files, full_names)?
    };

    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Non-recursive directory listing.
fn list_files_flat(
    path: &str,
    regex: &Option<regex::Regex>,
    all_files: bool,
    full_names: bool,
) -> Result<Vec<Option<String>>, RError> {
    let entries = fs::read_dir(path).map_err(|source| SystemError::ReadDir {
        path: path.to_string(),
        source,
    })?;

    let result: Vec<Option<String>> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            // Skip hidden files (starting with '.') unless all.files is TRUE
            if !all_files && name.starts_with('.') {
                return None;
            }
            if let Some(ref re) = regex {
                if !re.is_match(&name) {
                    return None;
                }
            }
            if full_names {
                Some(entry.path().to_string_lossy().to_string())
            } else {
                Some(name)
            }
        })
        .sorted()
        .map(Some)
        .collect();
    Ok(result)
}

/// Recursive directory listing using walkdir (when available) or std::fs fallback.
#[cfg(feature = "walkdir-support")]
fn list_files_recursive(
    path: &str,
    regex: &Option<regex::Regex>,
    all_files: bool,
    full_names: bool,
) -> Result<Vec<Option<String>>, RError> {
    let base = Path::new(path);
    let result: Vec<Option<String>> = walkdir::WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files unless all.files is TRUE
            if !all_files && name.starts_with('.') {
                return None;
            }
            if let Some(ref re) = regex {
                if !re.is_match(&name) {
                    return None;
                }
            }
            if full_names {
                Some(entry.path().to_string_lossy().to_string())
            } else {
                // Return path relative to the base directory
                entry
                    .path()
                    .strip_prefix(base)
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            }
        })
        .sorted()
        .map(Some)
        .collect();
    Ok(result)
}

/// Recursive directory listing fallback without walkdir.
#[cfg(not(feature = "walkdir-support"))]
fn list_files_recursive(
    path: &str,
    regex: &Option<regex::Regex>,
    all_files: bool,
    full_names: bool,
) -> Result<Vec<Option<String>>, RError> {
    let mut result: Vec<String> = Vec::new();
    list_files_recursive_fallback(
        Path::new(path),
        Path::new(path),
        regex,
        all_files,
        full_names,
        &mut result,
    )?;
    result.sort();
    Ok(result.into_iter().map(Some).collect())
}

#[cfg(not(feature = "walkdir-support"))]
fn list_files_recursive_fallback(
    base: &Path,
    dir: &Path,
    regex: &Option<regex::Regex>,
    all_files: bool,
    full_names: bool,
    out: &mut Vec<String>,
) -> Result<(), RError> {
    let entries = fs::read_dir(dir).map_err(|source| SystemError::ReadDir {
        path: dir.to_string_lossy().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();

        // Skip hidden files unless all.files is TRUE
        if !all_files && name.starts_with('.') {
            continue;
        }

        if entry_path.is_dir() {
            list_files_recursive_fallback(base, &entry_path, regex, all_files, full_names, out)?;
        } else {
            if let Some(ref re) = regex {
                if !re.is_match(&name) {
                    continue;
                }
            }
            if full_names {
                out.push(entry_path.to_string_lossy().to_string());
            } else if let Ok(rel) = entry_path.strip_prefix(base) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}

// region: list.dirs

/// List subdirectories of a directory.
///
/// @param path character scalar: directory path (default ".")
/// @param full.names logical: if TRUE, return full paths (default TRUE, matching R)
/// @param recursive logical: if TRUE, recurse into subdirectories (default TRUE).
///   When the `walkdir-support` feature is enabled, uses walkdir for efficient
///   recursive traversal.
/// @return character vector of directory paths (sorted)
#[interpreter_builtin(name = "list.dirs")]
fn builtin_list_dirs(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| ".".to_string());
    let path = resolved_path_string(context.interpreter(), &path);

    // R defaults: full.names = TRUE, recursive = TRUE
    let full_names = named
        .iter()
        .find(|(n, _)| n == "full.names")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let recursive = named
        .iter()
        .find(|(n, _)| n == "recursive")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let result = if recursive {
        list_dirs_recursive(&path, full_names)?
    } else {
        list_dirs_flat(&path, full_names)?
    };

    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Non-recursive directory listing (immediate subdirectories only).
fn list_dirs_flat(path: &str, full_names: bool) -> Result<Vec<Option<String>>, RError> {
    let base = Path::new(path);
    let entries = fs::read_dir(path).map_err(|source| SystemError::ReadDir {
        path: path.to_string(),
        source,
    })?;

    // Include the base directory itself, matching R behavior
    let mut result: Vec<String> = vec![if full_names {
        base.to_string_lossy().to_string()
    } else {
        ".".to_string()
    }];

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let name = entry.file_name().into_string().unwrap_or_default();
        if full_names {
            result.push(entry_path.to_string_lossy().to_string());
        } else {
            result.push(name);
        }
    }

    result.sort();
    Ok(result.into_iter().map(Some).collect())
}

/// Recursive directory listing using walkdir.
#[cfg(feature = "walkdir-support")]
fn list_dirs_recursive(path: &str, full_names: bool) -> Result<Vec<Option<String>>, RError> {
    let base = Path::new(path);
    let mut result: Vec<String> = walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if !entry.file_type().is_dir() {
                return None;
            }
            if full_names {
                Some(entry.path().to_string_lossy().to_string())
            } else {
                let rel = entry.path().strip_prefix(base).ok()?;
                let s = rel.to_string_lossy().to_string();
                Some(if s.is_empty() { ".".to_string() } else { s })
            }
        })
        .collect();
    result.sort();
    Ok(result.into_iter().map(Some).collect())
}

/// Recursive directory listing fallback without walkdir.
#[cfg(not(feature = "walkdir-support"))]
fn list_dirs_recursive(path: &str, full_names: bool) -> Result<Vec<Option<String>>, RError> {
    let base = Path::new(path);
    let mut result: Vec<String> = Vec::new();
    list_dirs_recursive_fallback(base, base, full_names, &mut result)?;
    result.sort();
    Ok(result.into_iter().map(Some).collect())
}

#[cfg(not(feature = "walkdir-support"))]
fn list_dirs_recursive_fallback(
    base: &Path,
    dir: &Path,
    full_names: bool,
    out: &mut Vec<String>,
) -> Result<(), RError> {
    // Add the current directory
    if full_names {
        out.push(dir.to_string_lossy().to_string());
    } else {
        let rel = dir
            .strip_prefix(base)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(if rel.is_empty() { ".".to_string() } else { rel });
    }

    let entries = fs::read_dir(dir).map_err(|source| SystemError::ReadDir {
        path: dir.to_string_lossy().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_path = entry.path();
        if entry_path.is_dir() {
            list_dirs_recursive_fallback(base, &entry_path, full_names, out)?;
        }
    }
    Ok(())
}

// endregion

// === Temp paths ===

/// Get the path to the interpreter's per-session temporary directory.
///
/// @return character scalar: path to the temp directory
#[interpreter_builtin]
fn interp_tempdir(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path =
        context.with_interpreter(|interp| interp.temp_dir.path().to_string_lossy().to_string());
    Ok(RValue::vec(Vector::Character(vec![Some(path)].into())))
}

/// Generate a unique temporary file path.
///
/// @param pattern character scalar: filename prefix (default "file")
/// @param tmpdir character scalar: directory for the temp file (default: session temp dir)
/// @param fileext character scalar: file extension (default "")
/// @return character scalar: the generated temporary file path
#[interpreter_builtin]
fn interp_tempfile(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .or_else(|| named.iter().find(|(n, _)| n == "pattern").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "file".to_string());

    let fileext = args
        .get(2)
        .or_else(|| named.iter().find(|(n, _)| n == "fileext").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let path = context.with_interpreter(|interp| {
        let tmpdir = args
            .get(1)
            .or_else(|| named.iter().find(|(n, _)| n == "tmpdir").map(|(_, v)| v))
            .and_then(|v| v.as_vector()?.as_character_scalar())
            .unwrap_or_else(|| interp.temp_dir.path().to_string_lossy().to_string());

        let n = interp.temp_counter.get();
        interp.temp_counter.set(n + 1);

        Path::new(&tmpdir)
            .join(format!("{}{}{}", pattern, n, fileext))
            .to_string_lossy()
            .to_string()
    });
    Ok(RValue::vec(Vector::Character(vec![Some(path)].into())))
}

// === Glob ===

/// Expand file system glob patterns to matching paths.
///
/// Uses the `glob` crate for file-system expansion. When `globset-support` is
/// enabled, pattern validation is done via `globset` for better error messages,
/// though actual path enumeration still goes through `glob::glob()` since
/// globset is a pattern matcher, not a directory walker.
///
/// @param ... character scalars: glob patterns (e.g. "*.R", "src/**/*.rs")
/// @return character vector of matching file paths
#[interpreter_builtin(name = "Sys.glob", min_args = 1)]
fn builtin_sys_glob(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let patterns: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();

    let mut results: Vec<Option<String>> = Vec::new();
    for pattern in &patterns {
        let resolved_pattern = resolved_path_string(context.interpreter(), pattern);
        // Validate the pattern with globset when available, for better errors
        #[cfg(feature = "globset-support")]
        {
            if let Err(e) = globset::Glob::new(&resolved_pattern) {
                return Err(RError::other(format!(
                    "invalid glob pattern '{}': {}",
                    pattern, e
                )));
            }
        }

        match glob::glob(&resolved_pattern) {
            Ok(paths) => {
                for path in paths.flatten() {
                    results.push(Some(path.to_string_lossy().to_string()));
                }
            }
            Err(e) => {
                return Err(RError::other(format!(
                    "invalid glob pattern '{}': {}",
                    pattern, e
                )));
            }
        }
    }

    Ok(RValue::vec(Vector::Character(results.into())))
}

// === Path operations ===

/// Normalize a file path to its canonical absolute form.
///
/// @param path character scalar: path to normalize
/// @param mustWork if TRUE, error when the path cannot be resolved; if FALSE/NA, return
///   the original path on failure
/// @return character scalar: the canonical path (or the original if resolution fails)
#[interpreter_builtin(name = "normalizePath", min_args = 1)]
fn builtin_normalize_path(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'path' must be a character string".to_string(),
            )
        })?;

    let must_work = named
        .iter()
        .find(|(n, _)| n == "mustWork")
        .or_else(|| named.iter().find(|(n, _)| n == "mustwork"))
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| args.get(2).and_then(|v| v.as_vector()?.as_logical_scalar()))
        .unwrap_or(false);

    let resolved = context.interpreter().resolve_path(&path);

    match fs::canonicalize(&resolved) {
        Ok(p) => Ok(RValue::vec(Vector::Character(
            vec![Some(p.to_string_lossy().to_string())].into(),
        ))),
        Err(e) => {
            if must_work {
                Err(RError::new(
                    RErrorKind::Other,
                    format!("path '{}' does not exist: {}", path, e),
                ))
            } else {
                Ok(RValue::vec(Vector::Character(
                    vec![Some(path.clone())].into(),
                )))
            }
        }
    }
}

/// Expand a tilde (~) prefix in a file path to the user's home directory.
///
/// Uses `dirs::home_dir()` (when the `dirs-support` feature is enabled) for
/// robust, cross-platform home directory detection, falling back to $HOME /
/// %USERPROFILE% environment variables.
///
/// @param path character scalar: path possibly starting with ~
/// @return character scalar: the expanded path
#[interpreter_builtin(name = "path.expand", min_args = 1)]
fn builtin_path_expand(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'path' must be a character string".to_string(),
            )
        })?;

    let expanded = if path.starts_with('~') {
        let home = home_dir_string(context.interpreter());
        match home {
            Some(h) => path.replacen('~', &h, 1),
            None => path,
        }
    } else {
        path
    };

    Ok(RValue::vec(Vector::Character(vec![Some(expanded)].into())))
}

// region: R.home / .libPaths

/// Return the miniR "home" directory (data directory for miniR resources).
///
/// Uses `dirs::data_dir()` to find a platform-appropriate location, e.g.
/// `~/Library/Application Support/miniR` on macOS, `~/.local/share/miniR`
/// on Linux, `%APPDATA%/miniR` on Windows.
///
/// @param component character scalar: optional sub-path within R home (default "")
/// @return character scalar: the miniR home directory path
#[interpreter_builtin(name = "R.home")]
fn builtin_r_home(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let component = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let base = minir_data_dir(context.interpreter());
    let result = if component.is_empty() {
        base
    } else {
        Path::new(&base)
            .join(&component)
            .to_string_lossy()
            .to_string()
    };

    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Return the library search paths for package installation.
///
/// Builds the search path from (in order):
/// 1. `R_LIBS` environment variable (colon-separated on Unix, semicolon on Windows)
/// 2. `R_LIBS_USER` environment variable
/// 3. The default miniR library directory (`<data_dir>/miniR/library`)
///
/// Only directories that actually exist on disk are included, matching R's
/// behavior of filtering `.libPaths()` to existing directories.
///
/// @param new character vector: if provided, sets new library paths (currently ignored)
/// @return character vector of library search paths
#[interpreter_builtin(name = ".libPaths")]
fn builtin_lib_paths(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let mut paths: Vec<String> = Vec::new();

    // Platform-appropriate path separator
    let sep = if cfg!(windows) { ';' } else { ':' };

    // R_LIBS takes priority
    if let Some(r_libs) = interp.get_env_var("R_LIBS") {
        for p in r_libs.split(sep) {
            let p = p.trim();
            if !p.is_empty() {
                let resolved = interp.resolve_path(p);
                if resolved.is_dir() {
                    paths.push(resolved.to_string_lossy().to_string());
                }
            }
        }
    }

    // Then R_LIBS_USER
    if let Some(r_libs_user) = interp.get_env_var("R_LIBS_USER") {
        for p in r_libs_user.split(sep) {
            let p = p.trim();
            if !p.is_empty() {
                let resolved = interp.resolve_path(p);
                if resolved.is_dir() {
                    paths.push(resolved.to_string_lossy().to_string());
                }
            }
        }
    }

    // Default miniR library directory (always included even if it doesn't exist yet,
    // as it's the canonical install location)
    let default_lib = format!("{}/library", minir_data_dir(interp));
    if !paths.contains(&default_lib) {
        paths.push(default_lib);
    }

    let values: Vec<Option<String>> = paths.into_iter().map(Some).collect();
    Ok(RValue::vec(Vector::Character(values.into())))
}

// endregion

// === System operations ===

/// Execute a shell command.
///
/// When `intern = FALSE` (default), the command runs and its exit code is
/// returned as an integer. When `intern = TRUE`, the command's stdout is
/// captured and returned as a character vector (one element per line).
///
/// @param command character scalar: the shell command to run
/// @param intern logical: if TRUE, capture stdout as character vector (default FALSE)
/// @return integer scalar (exit code) when intern=FALSE, or character vector when intern=TRUE
#[interpreter_builtin(name = "system", min_args = 1)]
fn builtin_system(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let command = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'command' must be a character string".to_string(),
            )
        })?;

    let intern = named
        .iter()
        .find(|(n, _)| n == "intern")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| args.get(1).and_then(|v| v.as_vector()?.as_logical_scalar()))
        .unwrap_or(false);

    if intern {
        // Capture stdout and return as character vector
        let output = context.with_interpreter(|interp| {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c")
                .arg(&command)
                .current_dir(interp.get_working_dir())
                .env_clear()
                .envs(interp.env_vars_snapshot());
            cmd.output().map_err(|source| SystemError::Command {
                command: command.clone(),
                source,
            })
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<Option<String>> =
            stdout.lines().map(|line| Some(line.to_string())).collect();
        Ok(RValue::vec(Vector::Character(lines.into())))
    } else {
        // Run command and return exit code
        let output = context.with_interpreter(|interp| {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c")
                .arg(&command)
                .current_dir(interp.get_working_dir())
                .env_clear()
                .envs(interp.env_vars_snapshot());
            cmd.status().map_err(|source| SystemError::Command {
                command: command.clone(),
                source,
            })
        })?;

        let code = i64::from(output.code().unwrap_or(-1));
        Ok(RValue::vec(Vector::Integer(vec![Some(code)].into())))
    }
}

/// Execute a command with arguments, optionally capturing stdout/stderr.
///
/// @param command character scalar: the program to run
/// @param args character vector: command-line arguments (default: none)
/// @param stdout logical or character: TRUE = capture to character vector,
///   FALSE = discard, "" = inherit (default), or a file path to redirect to
/// @param stderr logical or character: TRUE = capture, FALSE = discard,
///   "" = inherit (default), or a file path to redirect to
/// @return integer scalar (exit code) when stdout is not captured, or
///   character vector of captured output with "status" attribute set to exit code
#[interpreter_builtin(name = "system2", min_args = 1)]
fn builtin_system2(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let command = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'command' must be a character string".to_string(),
            )
        })?;

    let cmd_args: Vec<String> = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "args").map(|(_, v)| v))
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters().into_iter().flatten().collect())
        .unwrap_or_default();

    // Parse stdout parameter: TRUE = capture, FALSE = discard, "" = inherit
    let stdout_val = named.iter().find(|(n, _)| n == "stdout").map(|(_, v)| v);
    let capture_stdout = stdout_val
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    // Parse stderr parameter: TRUE = capture, FALSE = discard, "" = inherit
    let stderr_val = named.iter().find(|(n, _)| n == "stderr").map(|(_, v)| v);
    let capture_stderr = stderr_val
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    if capture_stdout || capture_stderr {
        // Capture output
        let output = context.with_interpreter(|interp| {
            let mut cmd = std::process::Command::new(&command);
            cmd.args(&cmd_args)
                .current_dir(interp.get_working_dir())
                .env_clear()
                .envs(interp.env_vars_snapshot());

            if capture_stdout {
                cmd.stdout(std::process::Stdio::piped());
            }
            if capture_stderr {
                cmd.stderr(std::process::Stdio::piped());
            }

            cmd.output().map_err(|source| SystemError::Command {
                command: command.clone(),
                source,
            })
        })?;

        let code = i64::from(output.status.code().unwrap_or(-1));

        // Build the captured output lines
        let mut lines: Vec<Option<String>> = Vec::new();

        if capture_stdout {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                lines.push(Some(line.to_string()));
            }
        }

        if capture_stderr {
            let stderr = String::from_utf8_lossy(&output.stderr);
            for line in stderr.lines() {
                lines.push(Some(line.to_string()));
            }
        }

        let mut rv = RVector::from(Vector::Character(lines.into()));
        rv.set_attr(
            "status".to_string(),
            RValue::vec(Vector::Integer(vec![Some(code)].into())),
        );
        Ok(RValue::Vector(rv))
    } else {
        // No capture — just run and return exit code
        let output = context.with_interpreter(|interp| {
            let mut cmd = std::process::Command::new(&command);
            cmd.args(&cmd_args)
                .current_dir(interp.get_working_dir())
                .env_clear()
                .envs(interp.env_vars_snapshot());
            cmd.status().map_err(|source| SystemError::Command {
                command: command.clone(),
                source,
            })
        })?;

        let code = i64::from(output.code().unwrap_or(-1));
        Ok(RValue::vec(Vector::Integer(vec![Some(code)].into())))
    }
}

/// Set environment variables in the interpreter's private environment.
///
/// @param ... named character scalars: name=value pairs to set
/// @return logical scalar: TRUE
#[interpreter_builtin(name = "Sys.setenv")]
fn interp_sys_setenv(
    _args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        for (name, val) in named {
            let val_str = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            interp.set_env_var(name.clone(), val_str);
        }
    });
    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
}

/// Unset environment variables in the interpreter's private environment.
///
/// @param x character vector of variable names to unset
/// @return logical vector (TRUE for each successfully unset)
#[interpreter_builtin(name = "Sys.unsetenv", min_args = 1)]
fn interp_sys_unsetenv(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let names: Vec<Option<String>> = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();

    let results: Vec<Option<bool>> = names
        .iter()
        .map(|n| {
            if let Some(name) = n {
                context.with_interpreter(|interp| interp.remove_env_var(name));
                Some(true)
            } else {
                Some(false)
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

/// Look up the full paths of programs on the system PATH.
///
/// @param names character vector: program names to search for
/// @return named character vector: full paths (empty string if not found),
///   with names set to the input program names (matching R behavior)
#[interpreter_builtin(name = "Sys.which", min_args = 1)]
fn interp_sys_which(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Accept a character vector as the first argument (R's Sys.which(c("ls", "cat")))
    let names: Vec<Option<String>> = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();

    let path_var = context
        .with_interpreter(|interp| interp.get_env_var("PATH"))
        .unwrap_or_default();
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path_dirs: Vec<&str> = path_var.split(sep).collect();

    let results: Vec<Option<String>> = names
        .iter()
        .map(|name_opt| {
            let name = match name_opt {
                Some(n) => n,
                None => return Some(String::new()),
            };
            // If the name contains a path separator, check it directly
            if name.contains('/') || (cfg!(windows) && name.contains('\\')) {
                let p = Path::new(name);
                if p.is_file() {
                    return Some(p.to_string_lossy().to_string());
                }
                return Some(String::new());
            }
            for dir in &path_dirs {
                let candidate = Path::new(dir).join(name);
                if candidate.is_file() {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = candidate.metadata() {
                            if meta.permissions().mode() & 0o111 != 0 {
                                return Some(candidate.to_string_lossy().to_string());
                            }
                        }
                        continue;
                    }
                    #[cfg(not(unix))]
                    {
                        return Some(candidate.to_string_lossy().to_string());
                    }
                }
            }
            Some(String::new())
        })
        .collect();

    // Return named character vector
    let mut rv = RVector::from(Vector::Character(results.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// Set the interpreter's working directory.
///
/// @param dir character scalar: path to the new working directory
/// @return character scalar: the previous working directory
#[interpreter_builtin(min_args = 1)]
fn interp_setwd(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let dir = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'dir' must be a character string".to_string(),
            )
        })?;

    let path = Path::new(&dir);
    if !path.is_dir() {
        return Err(SystemError::SetCwd {
            path: dir.clone(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such directory"),
        }
        .into());
    }

    context.with_interpreter(|interp| {
        let old_wd = interp.get_working_dir().to_string_lossy().to_string();
        interp.set_working_dir(path.to_path_buf());
        Ok(RValue::vec(Vector::Character(vec![Some(old_wd)].into())))
    })
}

// === Sleep ===

/// Pause execution for a specified number of seconds.
///
/// @param time numeric scalar: seconds to sleep (values <= 0 are ignored)
/// @return NULL (invisibly)
#[builtin(name = "Sys.sleep", min_args = 1)]
fn builtin_sys_sleep(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let time = args
        .first()
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'time' must be a numeric value".to_string(),
            )
        })?;

    if time > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(time));
    }

    Ok(RValue::Null)
}

// === System info ===

/// Return system information as a named character vector.
///
/// Returns all 7 fields that R's Sys.info() provides: sysname, nodename,
/// release, version, machine, login, user.
///
/// @return named character vector with 7 elements
#[interpreter_builtin(name = "Sys.info")]
fn builtin_sys_info(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let sysname = if cfg!(target_os = "macos") {
        "Darwin"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Unknown"
    };

    let machine = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    let nodename = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get OS release and version via uname -r / uname -v on Unix
    let release = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let version = std::process::Command::new("uname")
        .arg("-v")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let user = context
        .with_interpreter(|interp| {
            interp
                .get_env_var("USER")
                .or_else(|| interp.get_env_var("USERNAME"))
        })
        .unwrap_or_else(|| "unknown".to_string());

    // R returns a named character vector, not a list
    let field_names = vec![
        Some("sysname".to_string()),
        Some("nodename".to_string()),
        Some("release".to_string()),
        Some("version".to_string()),
        Some("machine".to_string()),
        Some("login".to_string()),
        Some("user".to_string()),
    ];
    let field_values = vec![
        Some(sysname.to_string()),
        Some(nodename),
        Some(release),
        Some(version),
        Some(machine.to_string()),
        Some(user.clone()),
        Some(user),
    ];

    let mut rv = RVector::from(Vector::Character(field_values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(field_names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// Get the current timezone from the TZ environment variable.
///
/// @return character scalar: timezone string (defaults to "UTC")
#[interpreter_builtin(name = "Sys.timezone")]
fn builtin_sys_timezone(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let tz = context
        .with_interpreter(|interp| interp.get_env_var("TZ"))
        .unwrap_or_else(|| "UTC".to_string());
    Ok(RValue::vec(Vector::Character(vec![Some(tz)].into())))
}

/// Report which optional features are available in this interpreter.
///
/// @return named logical vector of capability flags (jpeg, png, etc.)
#[builtin]
fn builtin_capabilities(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let caps = vec![
        ("jpeg", false),
        ("png", false),
        ("tiff", false),
        ("tcltk", false),
        ("X11", false),
        ("aqua", false),
        ("http/ftp", false),
        ("sockets", false),
        ("libxml", false),
        ("fifo", cfg!(unix)),
        ("cledit", true),
        ("iconv", true),
        ("NLS", false),
        ("profmem", false),
        ("cairo", false),
        ("ICU", false),
        ("long.double", true),
        ("libcurl", false),
    ];

    let names: Vec<Option<String>> = caps.iter().map(|(n, _)| Some(n.to_string())).collect();
    let values: Vec<Option<bool>> = caps.iter().map(|(_, v)| Some(*v)).collect();

    let mut rv = RVector::from(Vector::Logical(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// Report localization information (encoding support).
///
/// @return named list with MBCS, UTF-8, and Latin-1 flags
#[builtin]
fn builtin_l10n_info(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![
        (
            Some("MBCS".to_string()),
            RValue::vec(Vector::Logical(vec![Some(true)].into())),
        ),
        (
            Some("UTF-8".to_string()),
            RValue::vec(Vector::Logical(vec![Some(true)].into())),
        ),
        (
            Some("Latin-1".to_string()),
            RValue::vec(Vector::Logical(vec![Some(false)].into())),
        ),
    ])))
}

// region: proc.time

/// Get elapsed (wall-clock) time since the interpreter started.
///
/// @return named double vector: c(user.self, sys.self, elapsed)
#[interpreter_builtin(name = "proc.time")]
fn interp_proc_time(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let elapsed = context.with_interpreter(|interp| interp.start_instant.elapsed().as_secs_f64());
    // R returns a named vector c(user.self=..., sys.self=..., elapsed=...)
    // We don't track CPU time, so user and sys are 0.0
    let mut rv = RVector::from(Vector::Double(
        vec![Some(0.0), Some(0.0), Some(elapsed)].into(),
    ));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            vec![
                Some("user.self".to_string()),
                Some("sys.self".to_string()),
                Some("elapsed".to_string()),
            ]
            .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

// endregion

/// Return session information (miniR version, platform, locale).
///
/// @return named list with R.version, platform, and locale
#[interpreter_builtin(name = "sessionInfo")]
fn builtin_session_info(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let locale = context
        .with_interpreter(|interp| interp.get_env_var("LANG"))
        .unwrap_or_else(|| "C".to_string());
    Ok(RValue::List(RList::new(vec![
        (
            Some("R.version".to_string()),
            RValue::List(RList::new(vec![
                (
                    Some("major".to_string()),
                    RValue::vec(Vector::Character(vec![Some("0".to_string())].into())),
                ),
                (
                    Some("minor".to_string()),
                    RValue::vec(Vector::Character(vec![Some("1.0".to_string())].into())),
                ),
                (
                    Some("engine".to_string()),
                    RValue::vec(Vector::Character(
                        vec![Some("miniR (Rust)".to_string())].into(),
                    )),
                ),
            ])),
        ),
        (
            Some("platform".to_string()),
            RValue::vec(Vector::Character(
                vec![Some(format!(
                    "{}-{}",
                    std::env::consts::ARCH,
                    std::env::consts::OS
                ))]
                .into(),
            )),
        ),
        (
            Some("locale".to_string()),
            RValue::vec(Vector::Character(vec![Some(locale)].into())),
        ),
    ])))
}

/// Find files in installed packages.
///
/// Searches the package's installation directory for the specified file
/// path components. Returns the full path if found, or "" if not found.
///
/// @param ... character: path components to join (e.g. "DESCRIPTION", or "data", "mtcars.rda")
/// @param package character: the package name to search in
/// @param lib.loc character vector: library search paths (defaults to .libPaths())
/// @return character scalar: the full path to the file, or "" if not found
#[interpreter_builtin(name = "system.file")]
fn interp_system_file(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Extract 'package' named argument
    let package = named
        .iter()
        .find(|(n, _)| n == "package")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());

    // Extract 'lib.loc' named argument (optional)
    let lib_loc: Option<Vec<String>> =
        named
            .iter()
            .find(|(n, _)| n == "lib.loc")
            .and_then(|(_, v)| {
                let vec = v.as_vector()?;
                Some(
                    vec.to_characters()
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>(),
                )
            });

    // Collect positional args as path components (skip any that are named-arg leaks)
    let path_parts: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();

    let package_name = match package {
        Some(p) if !p.is_empty() => p,
        _ => {
            // No package specified — return ""
            return Ok(RValue::vec(Vector::Character(
                vec![Some(String::new())].into(),
            )));
        }
    };

    // Build the subpath from the path components
    let subpath = if path_parts.is_empty() {
        String::new()
    } else {
        path_parts.join("/")
    };

    // Search for the package directory
    let result = context.with_interpreter(|interp| {
        // Use lib.loc if provided, otherwise .libPaths()
        let lib_paths = lib_loc.unwrap_or_else(|| interp.get_lib_paths());

        for lib_path in &lib_paths {
            let pkg_dir = std::path::Path::new(lib_path).join(&package_name);
            if !pkg_dir.join("DESCRIPTION").is_file() {
                continue;
            }
            if subpath.is_empty() {
                // No subpath: return the package directory itself
                return pkg_dir.to_string_lossy().to_string();
            }
            let target = pkg_dir.join(&subpath);
            if target.exists() {
                return target.to_string_lossy().to_string();
            }
        }

        // Also check loaded_namespaces for the package's lib_path
        if let Some(ns) = interp.loaded_namespaces.borrow().get(&package_name) {
            let pkg_dir = &ns.lib_path;
            if subpath.is_empty() {
                return pkg_dir.to_string_lossy().to_string();
            }
            let target = pkg_dir.join(&subpath);
            if target.exists() {
                return target.to_string_lossy().to_string();
            }
        }

        // Not found
        String::new()
    });

    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Return the process ID of the current R process.
///
/// @return integer scalar: the PID
#[builtin(name = "Sys.getpid")]
fn builtin_sys_getpid(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pid = i64::from(std::process::id());
    Ok(RValue::vec(Vector::Integer(vec![Some(pid)].into())))
}

/// Open a file or URL with the system's default application.
///
/// Uses `open` on macOS, `xdg-open` on Linux, and `cmd /c start` on Windows.
/// This is an interactive utility — it launches an external process and returns
/// immediately without waiting for it to finish.
///
/// @param file character scalar: the file path or URL to open
/// @return NULL (invisibly)
#[builtin(name = "shell.exec", min_args = 1)]
fn builtin_shell_exec(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'file' must be a character string".to_string(),
            )
        })?;

    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&file).spawn()
    } else if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open").arg(&file).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &file])
            .spawn()
    } else {
        return Err(RError::other(
            "shell.exec is not supported on this platform".to_string(),
        ));
    };

    match result {
        Ok(_) => Ok(RValue::Null),
        Err(e) => Err(RError::other(format!("cannot open '{}': {}", file, e))),
    }
}
