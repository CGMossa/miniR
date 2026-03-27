//! Package C code compilation — Makevars parser and compiler invocation.
//!
//! Compiles package `src/*.c` files into a shared library (.so/.dylib)
//! using the system C compiler. Reads `src/Makevars` for custom flags.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// region: Makevars parser

/// Parsed Makevars key-value pairs.
#[derive(Debug, Default)]
pub struct Makevars {
    /// All key=value pairs from the Makevars file.
    pub vars: HashMap<String, String>,
}

impl Makevars {
    /// Parse a Makevars file. Returns empty Makevars if the file doesn't exist.
    pub fn parse(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Makevars::default(),
        };
        Self::parse_str(&content)
    }

    /// Parse Makevars content from a string.
    pub fn parse_str(content: &str) -> Self {
        let mut vars = HashMap::new();
        let mut continued_key: Option<String> = None;
        let mut continued_val = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                // But check if we're in a continuation
                if continued_key.is_some() {
                    // Comment breaks continuation
                    if let Some(key) = continued_key.take() {
                        vars.insert(key, continued_val.trim().to_string());
                        continued_val.clear();
                    }
                }
                continue;
            }

            // Handle line continuation
            if let Some(ref key) = continued_key {
                let (val, has_cont) = strip_continuation(line);
                continued_val.push(' ');
                continued_val.push_str(val.trim());
                if !has_cont {
                    vars.insert(key.clone(), continued_val.trim().to_string());
                    continued_key = None;
                    continued_val.clear();
                }
                continue;
            }

            // Try to parse as KEY = VALUE or KEY += VALUE
            if let Some((key, op, val_part)) = parse_assignment(line) {
                let (val, has_continuation) = strip_continuation(val_part);
                let val = val.trim();

                match op {
                    AssignOp::Set => {
                        if has_continuation {
                            continued_key = Some(key.to_string());
                            continued_val = val.to_string();
                        } else {
                            vars.insert(key.to_string(), val.to_string());
                        }
                    }
                    AssignOp::Append => {
                        let existing = vars.get(key).cloned().unwrap_or_default();
                        let new_val = if existing.is_empty() {
                            val.to_string()
                        } else {
                            format!("{existing} {val}")
                        };
                        if has_continuation {
                            continued_key = Some(key.to_string());
                            continued_val = new_val;
                        } else {
                            vars.insert(key.to_string(), new_val);
                        }
                    }
                }
            }
        }

        // Handle unterminated continuation
        if let Some(key) = continued_key {
            vars.insert(key, continued_val.trim().to_string());
        }

        Makevars { vars }
    }

    /// Get PKG_CFLAGS (additional C compiler flags).
    pub fn pkg_cflags(&self) -> &str {
        self.vars.get("PKG_CFLAGS").map_or("", |s| s.as_str())
    }

    /// Get PKG_CPPFLAGS (preprocessor flags like -I, -D).
    pub fn pkg_cppflags(&self) -> &str {
        self.vars.get("PKG_CPPFLAGS").map_or("", |s| s.as_str())
    }

    /// Get PKG_LIBS (linker flags).
    pub fn pkg_libs(&self) -> &str {
        self.vars.get("PKG_LIBS").map_or("", |s| s.as_str())
    }

    /// Get OBJECTS (explicit list of .o files to link).
    /// If not set, the default is all .c files in src/.
    pub fn objects(&self) -> Option<&str> {
        self.vars.get("OBJECTS").map(|s| s.as_str())
    }
}

#[derive(Debug, PartialEq)]
enum AssignOp {
    Set,
    Append,
}

/// Parse a line as `KEY = VALUE` or `KEY += VALUE`.
/// Returns (key, op, value_part) where value_part may have trailing `\`.
fn parse_assignment(line: &str) -> Option<(&str, AssignOp, &str)> {
    // Try += first (must come before = check)
    if let Some(pos) = line.find("+=") {
        let key = line[..pos].trim();
        let val = &line[pos + 2..];
        if !key.is_empty() && is_valid_makevars_key(key) {
            return Some((key, AssignOp::Append, val));
        }
    }

    // Try = (but not :=, which we treat the same as =)
    if let Some(pos) = line.find('=') {
        // Check for := (GNU Make simple expansion)
        let (key, val) = if pos > 0 && line.as_bytes()[pos - 1] == b':' {
            (line[..pos - 1].trim(), &line[pos + 1..])
        } else {
            (line[..pos].trim(), &line[pos + 1..])
        };
        if !key.is_empty() && is_valid_makevars_key(key) {
            return Some((key, AssignOp::Set, val));
        }
    }

    None
}

/// Check if a string is a valid Makevars variable name.
fn is_valid_makevars_key(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// Strip trailing backslash continuation. Returns (line_without_backslash, has_continuation).
fn strip_continuation(s: &str) -> (&str, bool) {
    let trimmed = s.trim_end();
    match trimmed.strip_suffix('\\') {
        Some(without) => (without, true),
        None => (trimmed, false),
    }
}

// endregion

// region: C compilation

/// Find the system C compiler.
fn find_cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

/// Shared library extension for the current platform.
fn dylib_ext() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

/// Compile C source files in a package's `src/` directory into a shared library.
///
/// # Arguments
/// * `pkg_src_dir` — the package's `src/` directory containing `.c` files and optionally `Makevars`
/// * `pkg_name` — package name (used for the output .so/.dylib name)
/// * `output_dir` — directory to write the compiled shared library
/// * `include_dir` — path to miniR's `include/` directory (for Rinternals.h)
///
/// # Returns
/// Path to the compiled shared library.
pub fn compile_package(
    pkg_src_dir: &Path,
    pkg_name: &str,
    output_dir: &Path,
    include_dir: &Path,
) -> Result<PathBuf, String> {
    // Parse Makevars
    let makevars = Makevars::parse(&pkg_src_dir.join("Makevars"));

    // Find C source files
    let c_files = find_c_sources(pkg_src_dir, &makevars)?;
    if c_files.is_empty() {
        return Err(format!(
            "no C source files found in {}",
            pkg_src_dir.display()
        ));
    }

    // Compile each .c file to .o
    let cc = find_cc();
    let mut object_files = Vec::new();

    for c_file in &c_files {
        let obj_file = output_dir.join(
            c_file
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
                + ".o",
        );

        let mut cmd = Command::new(&cc);
        cmd.arg("-c")
            .arg("-fPIC")
            .arg("-o")
            .arg(&obj_file)
            .arg(c_file)
            .arg(format!("-I{}", include_dir.display()))
            .arg(format!("-I{}", include_dir.join("miniR").display()))
            .arg(format!("-I{}", pkg_src_dir.display()));

        // Add PKG_CPPFLAGS (preprocessor flags: -I, -D)
        let cppflags = makevars.pkg_cppflags();
        if !cppflags.is_empty() {
            for flag in shell_split(cppflags) {
                cmd.arg(flag);
            }
        }

        // Add PKG_CFLAGS
        let cflags = makevars.pkg_cflags();
        if !cflags.is_empty() {
            for flag in shell_split(cflags) {
                cmd.arg(flag);
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("failed to run C compiler '{}': {e}", cc))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "compilation of {} failed:\n{}",
                c_file.display(),
                stderr
            ));
        }

        object_files.push(obj_file);
    }

    // Link into shared library
    let lib_name = format!("{pkg_name}.{}", dylib_ext());
    let lib_path = output_dir.join(&lib_name);

    let mut cmd = Command::new(&cc);
    cmd.arg("-shared").arg("-o").arg(&lib_path);

    for obj in &object_files {
        cmd.arg(obj);
    }

    // Platform-specific flags
    if cfg!(target_os = "macos") {
        cmd.arg("-undefined").arg("dynamic_lookup");
    }

    // Add PKG_LIBS (linker flags)
    let libs = makevars.pkg_libs();
    if !libs.is_empty() {
        for flag in shell_split(libs) {
            cmd.arg(flag);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run linker '{}': {e}", cc))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("linking {lib_name} failed:\n{stderr}"));
    }

    Ok(lib_path)
}

/// Find C source files to compile.
///
/// If Makevars specifies `OBJECTS`, derive source files from those .o names.
/// Otherwise, glob `src/*.c`.
fn find_c_sources(src_dir: &Path, makevars: &Makevars) -> Result<Vec<PathBuf>, String> {
    if let Some(objects) = makevars.objects() {
        // OBJECTS lists .o files — convert to .c file paths
        let mut sources = Vec::new();
        for obj in shell_split(objects) {
            let obj = obj.trim();
            if obj.is_empty() {
                continue;
            }
            // Convert foo.o → src/foo.c
            let c_name = if let Some(stem) = obj.strip_suffix(".o") {
                format!("{stem}.c")
            } else {
                continue;
            };
            let c_path = src_dir.join(&c_name);
            if c_path.is_file() {
                sources.push(c_path);
            }
        }
        Ok(sources)
    } else {
        // Default: all .c files in src/
        let mut sources = Vec::new();
        let entries = std::fs::read_dir(src_dir)
            .map_err(|e| format!("cannot read {}: {e}", src_dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("readdir error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("c") {
                sources.push(path);
            }
        }
        sources.sort();
        Ok(sources)
    }
}

/// Simple shell-like splitting of a string into words.
/// Handles basic quoting (double quotes) but not single quotes or escapes.
fn shell_split(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in s.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_makevars() {
        let content = r#"
# Package flags
PKG_CFLAGS = -Wall -O2
PKG_LIBS = -lz
OBJECTS = foo.o bar.o
"#;
        let mv = Makevars::parse_str(content);
        assert_eq!(mv.pkg_cflags(), "-Wall -O2");
        assert_eq!(mv.pkg_libs(), "-lz");
        assert_eq!(mv.objects(), Some("foo.o bar.o"));
    }

    #[test]
    fn parse_continuation_lines() {
        let content = "PKG_CFLAGS = -Wall \\\n  -O2 \\\n  -Wextra\n";
        let mv = Makevars::parse_str(content);
        assert_eq!(mv.pkg_cflags(), "-Wall -O2 -Wextra");
    }

    #[test]
    fn parse_append_operator() {
        let content = "PKG_CFLAGS = -Wall\nPKG_CFLAGS += -O2\n";
        let mv = Makevars::parse_str(content);
        assert_eq!(mv.pkg_cflags(), "-Wall -O2");
    }

    #[test]
    fn parse_colon_equals() {
        let content = "PKG_CFLAGS := -Wall\n";
        let mv = Makevars::parse_str(content);
        assert_eq!(mv.pkg_cflags(), "-Wall");
    }

    #[test]
    fn empty_makevars() {
        let mv = Makevars::parse_str("");
        assert_eq!(mv.pkg_cflags(), "");
        assert_eq!(mv.pkg_libs(), "");
        assert!(mv.objects().is_none());
    }

    #[test]
    fn shell_split_basic() {
        assert_eq!(shell_split("-Wall -O2"), vec!["-Wall", "-O2"]);
        assert_eq!(shell_split("  -I/usr/include  "), vec!["-I/usr/include"]);
        assert_eq!(shell_split(r#"-DFOO="bar baz""#), vec!["-DFOO=bar baz"]);
    }
}
