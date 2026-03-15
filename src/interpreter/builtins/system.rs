//! System, file, directory, path, and temp builtins.
//!
//! Each function is auto-registered via `#[builtin]` + linkme.

use crate::interpreter::coerce::*;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use derive_more::{Display, Error};
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

// === File operations ===

#[builtin(name = "file.copy", min_args = 2)]
fn builtin_file_copy(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
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

    match fs::copy(&from, &to) {
        Ok(_) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => Err(SystemError::Copy { from, to, source }.into()),
    }
}

#[builtin(name = "file.create", min_args = 1)]
fn builtin_file_create(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            match fs::File::create(&path) {
                Ok(_) => Some(true),
                Err(_) => Some(false),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

#[builtin(name = "file.remove", min_args = 1)]
fn builtin_file_remove(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            match fs::remove_file(&path) {
                Ok(()) => Some(true),
                Err(_) => Some(false),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

#[builtin(name = "file.rename", min_args = 2)]
fn builtin_file_rename(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
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

    match fs::rename(&from, &to) {
        Ok(()) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => Err(SystemError::Rename { from, to, source }.into()),
    }
}

#[builtin(name = "file.size", min_args = 1)]
fn builtin_file_size(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<f64>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            fs::metadata(&path).ok().map(|m| u64_to_f64(m.len()))
        })
        .collect();
    Ok(RValue::vec(Vector::Double(results.into())))
}

#[builtin(min_args = 1)]
fn builtin_unlink(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
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

#[builtin(name = "file.info", min_args = 1)]
fn builtin_file_info(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let paths: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
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

#[builtin(name = "dir.create", min_args = 1)]
fn builtin_dir_create(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'path' must be a character string".to_string(),
            )
        })?;

    // miniR diverges from R: recursive by default
    match fs::create_dir_all(&path) {
        Ok(()) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(source) => Err(SystemError::CreateDir { path, source }.into()),
    }
}

#[builtin(name = "dir.exists", min_args = 1)]
fn builtin_dir_exists(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Some(Path::new(&path).is_dir())
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

#[builtin(name = "list.files", names = ["dir"])]
fn builtin_list_files(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| ".".to_string());

    let pattern = named
        .iter()
        .find(|(n, _)| n == "pattern")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());

    let regex = match &pattern {
        Some(pat) => Some(regex::Regex::new(pat).map_err(|source| -> RError {
            super::strings::StringError::InvalidRegex { source }.into()
        })?),
        None => None,
    };

    let entries = fs::read_dir(&path).map_err(|source| SystemError::ReadDir {
        path: path.clone(),
        source,
    })?;

    let mut files: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if let Some(ref re) = regex {
                if !re.is_match(&name) {
                    return None;
                }
            }
            Some(name)
        })
        .collect();

    files.sort();

    let result: Vec<Option<String>> = files.into_iter().map(Some).collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

// === Temp paths ===

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

#[builtin(name = "Sys.glob", min_args = 1)]
fn builtin_sys_glob(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let patterns: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();

    let mut results: Vec<Option<String>> = Vec::new();
    for pattern in &patterns {
        match glob::glob(pattern) {
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

#[builtin(name = "normalizePath", min_args = 1)]
fn builtin_normalize_path(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'path' must be a character string".to_string(),
            )
        })?;

    let normalized = fs::canonicalize(&path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.clone());

    Ok(RValue::vec(Vector::Character(
        vec![Some(normalized)].into(),
    )))
}

#[builtin(name = "path.expand", min_args = 1)]
fn builtin_path_expand(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
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
        match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            Ok(home) => path.replacen('~', &home, 1),
            Err(_) => path,
        }
    } else {
        path
    };

    Ok(RValue::vec(Vector::Character(vec![Some(expanded)].into())))
}

// === System operations ===

#[builtin(min_args = 1)]
fn builtin_system(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let command = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'command' must be a character string".to_string(),
            )
        })?;

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .status()
        .map_err(|source| SystemError::Command {
            command: command.clone(),
            source,
        })?;

    let code = i64::from(output.code().unwrap_or(-1));
    Ok(RValue::vec(Vector::Integer(vec![Some(code)].into())))
}

#[builtin(min_args = 1)]
fn builtin_system2(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
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

    let output = std::process::Command::new(&command)
        .args(&cmd_args)
        .status()
        .map_err(|source| SystemError::Command {
            command: command.clone(),
            source,
        })?;

    let code = i64::from(output.code().unwrap_or(-1));
    Ok(RValue::vec(Vector::Integer(vec![Some(code)].into())))
}

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

#[interpreter_builtin(name = "Sys.which", min_args = 1)]
fn interp_sys_which(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let names: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();

    let path_var = context
        .with_interpreter(|interp| interp.get_env_var("PATH"))
        .unwrap_or_default();
    let path_dirs: Vec<&str> = path_var.split(':').collect();

    let results: Vec<Option<String>> = names
        .iter()
        .map(|name| {
            for dir in &path_dirs {
                let candidate = Path::new(dir).join(name);
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
            Some(String::new())
        })
        .collect();

    Ok(RValue::vec(Vector::Character(results.into())))
}

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

// === System info ===

#[builtin(name = "Sys.info")]
fn builtin_sys_info(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
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

    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let mut list = RList::new(vec![
        (
            Some("sysname".to_string()),
            RValue::vec(Vector::Character(vec![Some(sysname.to_string())].into())),
        ),
        (
            Some("nodename".to_string()),
            RValue::vec(Vector::Character(vec![Some(nodename)].into())),
        ),
        (
            Some("machine".to_string()),
            RValue::vec(Vector::Character(vec![Some(machine.to_string())].into())),
        ),
        (
            Some("login".to_string()),
            RValue::vec(Vector::Character(vec![Some(user.clone())].into())),
        ),
        (
            Some("user".to_string()),
            RValue::vec(Vector::Character(vec![Some(user)].into())),
        ),
    ]);

    // Set names attribute for named character vector behavior
    let names: Vec<Option<String>> = list.values.iter().map(|(n, _)| n.clone()).collect();
    list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );

    Ok(RValue::List(list))
}

#[builtin(name = "Sys.timezone")]
fn builtin_sys_timezone(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let tz = std::env::var("TZ").unwrap_or_else(|_| "UTC".to_string());
    Ok(RValue::vec(Vector::Character(vec![Some(tz)].into())))
}

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
        ("iconv", false),
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

#[builtin(name = "sessionInfo")]
fn builtin_session_info(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
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
            RValue::vec(Vector::Character(
                vec![Some(
                    std::env::var("LANG").unwrap_or_else(|_| "C".to_string()),
                )]
                .into(),
            )),
        ),
    ])))
}
