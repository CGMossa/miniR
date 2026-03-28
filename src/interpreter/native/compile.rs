//! Package C/C++ code compilation — Makevars parser and compiler invocation.
//!
//! Compiles package `src/*.{c,cpp,cc,cxx}` files into a shared library
//! (.so/.dylib). Uses the `cc` crate for compiler detection and flag
//! management (respects CC, CXX, CFLAGS, CXXFLAGS env vars, handles
//! cross-compilation). Only the final linking step uses `std::process::Command`.
//!
//! Reads `src/Makevars` for package-specific flags.

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
    ///
    /// Handles Make variable references `$(VAR)` by expanding known R variables
    /// and stripping unknown ones. Skips Make conditionals and build targets.
    pub fn parse_str(content: &str) -> Self {
        let mut vars = HashMap::new();
        let mut continued_key: Option<String> = None;
        let mut continued_val = String::new();
        let mut in_conditional = 0i32; // nesting depth of ifeq/ifdef

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                if continued_key.is_some() {
                    if let Some(key) = continued_key.take() {
                        vars.insert(key, continued_val.trim().to_string());
                        continued_val.clear();
                    }
                }
                continue;
            }

            // Handle Make conditionals — skip content inside them
            if line.starts_with("ifeq")
                || line.starts_with("ifdef")
                || line.starts_with("ifneq")
                || line.starts_with("ifndef")
            {
                in_conditional += 1;
                continue;
            }
            if line.starts_with("endif") {
                in_conditional = (in_conditional - 1).max(0);
                continue;
            }
            if line == "else" || line.starts_with("else ") {
                continue;
            }
            if in_conditional > 0 {
                continue;
            }

            // Skip Make targets (lines with `:` before any `=`)
            if let Some(colon_pos) = line.find(':') {
                if let Some(eq_pos) = line.find('=') {
                    // `:=` is an assignment, not a target
                    if colon_pos + 1 != eq_pos
                        && colon_pos < eq_pos
                        && !line[..colon_pos].contains('$')
                    {
                        continue; // target: dependency
                    }
                } else {
                    continue; // target with no assignment
                }
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

        // Expand $(VAR) references in all values
        let expanded: HashMap<String, String> = vars
            .into_iter()
            .map(|(k, v)| (k, expand_make_vars(&v)))
            .collect();

        Makevars { vars: expanded }
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

    /// Get PKG_CXXFLAGS (C++ compiler flags).
    pub fn pkg_cxxflags(&self) -> &str {
        self.vars.get("PKG_CXXFLAGS").map_or("", |s| s.as_str())
    }

    /// Get OBJECTS (explicit list of .o files to link).
    /// If not set, the default is all .c/.cpp files in src/.
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

/// Expand Make variable references `$(VAR)` with known R values.
/// Unknown variables are stripped (removed from the string).
fn expand_make_vars(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'(') {
            chars.next(); // consume '('
                          // Collect the variable name/expression, respecting nested parens
            let mut var_name = String::new();
            let mut depth = 1;
            for c in chars.by_ref() {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                var_name.push(c);
            }
            // Expand known variables, strip unknown
            match var_name.as_str() {
                "C_VISIBILITY" | "CXX_VISIBILITY" => {
                    result.push_str("-fvisibility=hidden");
                }
                "F_VISIBILITY" | "FPICFLAGS" | "CPICFLAGS" => {
                    // Fortran/PIC flags — skip (handled by cc crate)
                }
                "SHLIB_OPENMP_CFLAGS" | "SHLIB_OPENMP_CXXFLAGS" => {
                    // OpenMP — skip for now
                }
                "BLAS_LIBS" | "LAPACK_LIBS" | "FLIBS" => {
                    // System math libraries — skip
                }
                // Build system variables — skip entirely
                "CC" | "CXX" | "AR" | "RANLIB" | "MAKE" | "RM" | "SHLIB" | "STATLIB"
                | "OBJECTS" | "LIBR" | "SHLIB_EXT" | "SHLIB_LINK" | "SHLIB_LIBADD"
                | "SHLIB_CXXLD" | "SHLIB_CXXLDFLAGS" | "SHLIB_FFLAGS" | "CFLAGS" | "CPPFLAGS"
                | "LDFLAGS" | "SAFE_FFLAGS" | "R_ARCH" | "R_ARCH_BIN" | "R_HOME" | "R_CC"
                | "R_CXX" | "R_CONFIGURE_FLAGS" | "CONFIGURE_ARGS" | "ALL_CFLAGS"
                | "ALL_CPPFLAGS" | "UNAME" | "OS" | "WIN" | "CYGWIN" | "CC_TARGET"
                | "CLANG_CHECK" => {}
                _ => {
                    // Unknown variable — strip it
                    tracing::debug!("Makevars: stripping unknown variable $({})", var_name);
                }
            }
        } else {
            result.push(ch);
        }
    }

    // Clean up double spaces from stripped variables
    let mut clean = result.replace("  ", " ");
    while clean.contains("  ") {
        clean = clean.replace("  ", " ");
    }
    clean.trim().to_string()
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

// region: C/C++ compilation

/// Get the current platform's target triple (e.g. "aarch64-apple-darwin").
fn current_target_triple() -> String {
    // Check if TARGET is set (e.g. in a Cargo build environment)
    if let Ok(target) = std::env::var("TARGET") {
        return target;
    }
    // Construct from compile-time cfg values
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    // Map to standard triple format
    let vendor_os = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        "windows" => "pc-windows-msvc",
        "freebsd" => "unknown-freebsd",
        other => other,
    };
    format!("{arch}-{vendor_os}")
}

/// Shared library extension for the current platform.
fn dylib_ext() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

/// Compile C/C++ source files in a package's `src/` directory into a shared library.
///
/// Uses the `cc` crate for compiler detection (respects CC, CXX, CFLAGS, CXXFLAGS
/// env vars, handles cross-compilation). The `cc` crate compiles sources to `.o`
/// files; we then link them into a `.so`/`.dylib` ourselves.
///
/// # Arguments
/// * `pkg_src_dir` — the package's `src/` directory
/// * `pkg_name` — package name (used for the output library name)
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

    // Find C and C++ source files
    let src_files = find_sources(pkg_src_dir, &makevars)?;
    if src_files.is_empty() {
        return Err(format!(
            "no C/C++ source files found in {}",
            pkg_src_dir.display()
        ));
    }

    // Runtime is now in the binary (Rust extern "C" + C trampoline via build.rs).
    // Package .so files resolve API symbols at load time.
    // No minir_runtime.c needed.

    // Use cc::Build for compilation — it handles compiler detection,
    // platform flags, cross-compilation, ccache/sccache, etc.
    //
    // cc::Build normally runs inside Cargo build scripts where TARGET/HOST
    // env vars are set. At runtime we set them to the current platform.
    let target = current_target_triple();

    // Split files into C and C++ groups — must compile separately because
    // cc::Build with .cpp(true) compiles ALL files as C++, which breaks
    // C files that rely on C linkage (extern declarations without extern "C").
    let (c_files, cpp_files): (Vec<_>, Vec<_>) = src_files.iter().partition(|f| {
        !matches!(
            f.extension().and_then(|e| e.to_str()),
            Some("cpp" | "cc" | "cxx" | "C")
        )
    });
    let has_cpp = !cpp_files.is_empty();

    // Helper: configure common build settings
    let configure_build = |build: &mut cc::Build| {
        build
            .pic(true)
            .warnings(false)
            .cargo_metadata(false) // suppress cargo:rerun-if-env-changed output
            // Suppress function pointer type errors (common in R packages with Fortran)
            .flag_if_supported("-Wno-incompatible-function-pointer-types")
            .flag_if_supported("-Wno-int-conversion")
            .flag_if_supported("-Wno-error")
            .target(&target)
            .host(&target)
            .opt_level(2)
            .out_dir(output_dir)
            .include(include_dir)
            .include(include_dir.join("miniR"))
            .include(pkg_src_dir);

        // Add PKG_CPPFLAGS (preprocessor flags, applies to both C and C++)
        let cppflags = makevars.pkg_cppflags();
        if !cppflags.is_empty() {
            for flag in shell_split(cppflags) {
                if let Some(rel_path) = flag.strip_prefix("-I") {
                    let path = Path::new(rel_path);
                    if path.is_relative() {
                        build.include(pkg_src_dir.join(path));
                    } else {
                        build.include(path);
                    }
                } else {
                    build.flag(&flag);
                }
            }
        }
    };

    let mut object_files = Vec::new();

    // Compile C files
    if !c_files.is_empty() {
        let mut c_build = cc::Build::new();
        configure_build(&mut c_build);
        let cflags = makevars.pkg_cflags();
        if !cflags.is_empty() {
            for flag in shell_split(cflags) {
                c_build.flag(&flag);
            }
        }
        for src in &c_files {
            c_build.file(src);
        }
        let c_objs = c_build
            .try_compile_intermediates()
            .map_err(|e| format!("C compilation failed: {e}"))?;
        object_files.extend(c_objs);
    }

    // Compile C++ files
    if has_cpp {
        let mut cxx_build = cc::Build::new();
        configure_build(&mut cxx_build);
        cxx_build.cpp(true).std("c++17");
        let cxxflags = makevars.pkg_cxxflags();
        if !cxxflags.is_empty() {
            for flag in shell_split(cxxflags) {
                cxx_build.flag(&flag);
            }
        }
        for src in &cpp_files {
            cxx_build.file(src);
        }
        let cxx_objs = cxx_build
            .try_compile_intermediates()
            .map_err(|e| format!("C++ compilation failed: {e}"))?;
        object_files.extend(cxx_objs);
    }

    // Link .o files into a shared library (.so/.dylib)
    // Use the C++ compiler as linker if any C++ files were compiled.
    let linker_build = if has_cpp {
        let mut b = cc::Build::new();
        b.cpp(true)
            .cargo_metadata(false)
            .target(&target)
            .host(&target)
            .opt_level(2);
        b
    } else {
        let mut b = cc::Build::new();
        b.cargo_metadata(false)
            .target(&target)
            .host(&target)
            .opt_level(2);
        b
    };
    let linker = linker_build
        .try_get_compiler()
        .map_err(|e| format!("cannot find compiler: {e}"))?;

    let lib_name = format!("{pkg_name}.{}", dylib_ext());
    let lib_path = output_dir.join(&lib_name);

    let mut cmd = Command::new(linker.path());
    cmd.arg("-shared").arg("-o").arg(&lib_path);

    for obj in &object_files {
        cmd.arg(obj);
    }

    // Platform-specific flags
    if cfg!(target_os = "macos") {
        cmd.arg("-undefined").arg("dynamic_lookup");
    }

    // C++ runtime linking
    if has_cpp {
        if cfg!(target_os = "macos") {
            cmd.arg("-lc++");
        } else {
            cmd.arg("-lstdc++");
        }
    }

    // Add PKG_LIBS (linker flags) — skip local -L/-l for bundled static libs
    // since we compile all sources directly into the .so.
    // If PKG_LIBS references a local -L path, ALL -l flags from PKG_LIBS are
    // likely bundled and should be skipped.
    let libs = makevars.pkg_libs();
    if !libs.is_empty() {
        let flags = shell_split(libs);
        let has_local_lib = flags
            .iter()
            .any(|f| f.starts_with("-L") && !f.starts_with("-L/"));
        for flag in &flags {
            if flag.starts_with("-L") && !flag.starts_with("-L/") {
                continue; // skip local -L paths
            }
            if has_local_lib && flag.starts_with("-l") {
                continue; // skip all -l when using local lib paths
            }
            cmd.arg(flag);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run linker: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("linking {lib_name} failed:\n{stderr}"));
    }

    Ok(lib_path)
}

/// Find C and C++ source files to compile.
///
/// Collects sources from:
/// 1. `OBJECTS` variable (if set) — explicit list
/// 2. Other variables ending in `.o` — bundled library object files
/// 3. Fallback: all `src/*.{c,cpp,cc,cxx}` files
///
/// For bundled libraries (jsonlite/yajl, commonmark/cmark, etc.), the Makevars
/// defines variables like `LIBYAJL = yajl/yajl.o yajl/yajl_alloc.o ...`.
/// We collect ALL `.o` references, resolve them to source files, and compile
/// everything into one shared library (no static archive intermediate).
fn find_sources(src_dir: &Path, makevars: &Makevars) -> Result<Vec<PathBuf>, String> {
    // Collect all .o file references from ALL Makevars variables
    let mut all_objects: Vec<String> = Vec::new();

    for (key, value) in &makevars.vars {
        // Skip non-object variables
        if matches!(
            key.as_str(),
            "PKG_CFLAGS" | "PKG_CPPFLAGS" | "PKG_CXXFLAGS" | "PKG_LIBS" | "CXX_STD"
        ) {
            continue;
        }
        // Extract .o file references from the value
        for token in shell_split(value) {
            let token = token.trim().to_string();
            if token.ends_with(".o") {
                all_objects.push(token);
            }
        }
    }

    if !all_objects.is_empty() {
        // Resolve .o files to source files
        let mut sources = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for obj in &all_objects {
            let stem = if let Some(s) = obj.strip_suffix(".o") {
                s
            } else {
                continue;
            };
            for ext in &["c", "cpp", "cc", "cxx"] {
                let path = src_dir.join(format!("{stem}.{ext}"));
                if path.is_file() && seen.insert(path.clone()) {
                    sources.push(path);
                    break;
                }
            }
        }

        // Also add any top-level .c/.cpp files not already included
        // (some packages have both bundled libs AND top-level sources)
        if let Ok(entries) = std::fs::read_dir(src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if matches!(ext, "c" | "cpp" | "cc" | "cxx") && seen.insert(path.clone()) {
                            sources.push(path);
                        }
                    }
                }
            }
        }

        sources.sort();
        Ok(sources)
    } else {
        // Default: all C/C++ source files in src/ (non-recursive)
        let mut sources = Vec::new();
        let entries = std::fs::read_dir(src_dir)
            .map_err(|e| format!("cannot read {}: {e}", src_dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("readdir error: {e}"))?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if matches!(ext, "c" | "cpp" | "cc" | "cxx") {
                        sources.push(path);
                    }
                }
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
