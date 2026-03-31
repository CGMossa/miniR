//! Native symbol resolution for stack traces.
//!
//! Two layers of resolution:
//! 1. **dladdr** — lightweight, zero deps: function name + library path
//! 2. **addr2line/gimli** — DWARF debug info: file:line for C code
//!
//! The DWARF layer is optional and gracefully falls back to dladdr-only
//! output when debug info is unavailable.
//!
//! Platform quirks handled:
//! - **macOS**: dSYM bundles (`<lib>.dSYM/Contents/Resources/DWARF/<filename>`)
//! - **Linux**: `.gnu_debuglink` section, build-id paths, `/usr/lib/debug/` mirror
//! - **musl**: no `dladdr` or `backtrace()` — graceful no-op

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A resolved native stack frame.
#[derive(Debug, Clone)]
pub struct ResolvedFrame {
    /// Raw instruction pointer address.
    pub address: usize,
    /// Demangled function name (if available).
    pub function: Option<String>,
    /// Path to the shared library containing this frame.
    pub library: Option<String>,
    /// Offset from the start of the function.
    pub offset: usize,
    /// Source file (from DWARF debug info, if available).
    pub file: Option<String>,
    /// Source line number (from DWARF debug info, if available).
    pub line: Option<u32>,
}

// region: dladdr FFI
//
// dladdr is available on macOS (always) and Linux with glibc.
// On musl Linux it does not exist — the entire dladdr layer is a no-op.

#[cfg(any(target_os = "macos", target_env = "gnu"))]
mod dladdr_ffi {
    use std::ffi::{c_void, CStr};
    use std::os::raw::c_char;
    use std::path::Path;

    use super::ResolvedFrame;

    #[repr(C)]
    struct DlInfo {
        dli_fname: *const c_char,
        dli_fbase: *mut c_void,
        dli_sname: *const c_char,
        dli_saddr: *mut c_void,
    }

    extern "C" {
        fn dladdr(addr: *const c_void, info: *mut DlInfo) -> i32;
    }

    /// Result of dladdr lookup — raw info needed for DWARF resolution.
    pub struct DladdrResult {
        pub frame: ResolvedFrame,
        /// Full path to the shared library (for DWARF lookup).
        pub library_path: Option<String>,
        /// Base address where the library is loaded (for address rebasing).
        pub library_base: usize,
    }

    /// Resolve a single address via dladdr.
    pub fn resolve(addr: usize) -> DladdrResult {
        let mut info: DlInfo = unsafe { std::mem::zeroed() };
        let ret = unsafe { dladdr(addr as *const c_void, &mut info) };
        if ret == 0 {
            return DladdrResult {
                frame: ResolvedFrame {
                    address: addr,
                    function: None,
                    library: None,
                    offset: 0,
                    file: None,
                    line: None,
                },
                library_path: None,
                library_base: 0,
            };
        }

        let function = if info.dli_sname.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(info.dli_sname) }
                .to_str()
                .ok()
                .map(String::from)
        };

        let (library, library_path) = if info.dli_fname.is_null() {
            (None, None)
        } else {
            let full_path = unsafe { CStr::from_ptr(info.dli_fname) }
                .to_str()
                .ok()
                .map(String::from);
            let short_name = full_path.as_deref().map(|s| {
                Path::new(s)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(s)
                    .to_string()
            });
            (short_name, full_path)
        };

        let offset = if info.dli_saddr.is_null() {
            0
        } else {
            addr.wrapping_sub(info.dli_saddr as usize)
        };

        let library_base = info.dli_fbase as usize;

        DladdrResult {
            frame: ResolvedFrame {
                address: addr,
                function,
                library,
                offset,
                file: None,
                line: None,
            },
            library_path,
            library_base,
        }
    }
}

/// Fallback for platforms without dladdr (e.g., musl Linux).
#[cfg(not(any(target_os = "macos", target_env = "gnu")))]
mod dladdr_ffi {
    use super::ResolvedFrame;

    pub struct DladdrResult {
        pub frame: ResolvedFrame,
        pub library_path: Option<String>,
        pub library_base: usize,
    }

    pub fn resolve(addr: usize) -> DladdrResult {
        DladdrResult {
            frame: ResolvedFrame {
                address: addr,
                function: None,
                library: None,
                offset: 0,
                file: None,
                line: None,
            },
            library_path: None,
            library_base: 0,
        }
    }
}

// endregion

// region: DWARF debug info discovery
//
// Finding DWARF debug info is platform-specific:
// - macOS: .dSYM bundles next to the library
// - Linux: .gnu_debuglink section, build-id paths, /usr/lib/debug/ mirror
// - Fallback: embedded in the binary itself

/// Find the file containing DWARF debug info for a given library.
/// Returns the library_path itself if debug info is embedded, or a
/// separate debug file path if found via platform-specific mechanisms.
fn find_debug_file(library_path: &str) -> PathBuf {
    let lib = Path::new(library_path);

    // macOS: check for .dSYM bundle
    #[cfg(target_os = "macos")]
    if let Some(dsym) = find_dsym(lib) {
        return dsym;
    }

    // Linux: check .gnu_debuglink, build-id, and /usr/lib/debug/ mirror
    #[cfg(target_os = "linux")]
    if let Some(debug) = find_linux_debug_file(lib) {
        return debug;
    }

    // Fallback: debug info embedded in the binary itself
    lib.to_path_buf()
}

/// macOS: look for `<path>.dSYM/Contents/Resources/DWARF/<filename>`.
#[cfg(target_os = "macos")]
fn find_dsym(lib: &Path) -> Option<PathBuf> {
    let filename = lib.file_name()?.to_str()?;
    let dsym = lib
        .parent()?
        .join(format!("{}.dSYM", filename))
        .join("Contents")
        .join("Resources")
        .join("DWARF")
        .join(filename);
    if dsym.exists() {
        Some(dsym)
    } else {
        None
    }
}

/// Linux: find separate debug info via multiple strategies.
#[cfg(target_os = "linux")]
fn find_linux_debug_file(lib: &Path) -> Option<PathBuf> {
    // Strategy 1: .gnu_debuglink section in the binary
    if let Some(path) = find_gnu_debuglink(lib) {
        return Some(path);
    }

    // Strategy 2: build-id path (/usr/lib/debug/.build-id/<xx>/<rest>.debug)
    if let Some(path) = find_build_id_debug(lib) {
        return Some(path);
    }

    // Strategy 3: /usr/lib/debug mirror of the original path
    let debug_mirror = Path::new("/usr/lib/debug").join(lib.strip_prefix("/").unwrap_or(lib));
    if debug_mirror.exists() {
        return Some(debug_mirror);
    }

    None
}

/// Read the `.gnu_debuglink` section to find a separate debug file.
/// The section contains a filename (no directory) and a CRC32 checksum.
/// Search order: same directory as the library, then `/usr/lib/debug/`.
#[cfg(target_os = "linux")]
fn find_gnu_debuglink(lib: &Path) -> Option<PathBuf> {
    use object::{Object as _, ObjectSection as _};

    let bytes = std::fs::read(lib).ok()?;
    let object = object::File::parse(&*bytes).ok()?;
    let section = object.section_by_name(".gnu_debuglink")?;
    let data = section.data().ok()?;

    // The section contains: null-terminated filename, padding, 4-byte CRC32.
    // We only need the filename.
    let nul_pos = data.iter().position(|&b| b == 0)?;
    let debug_filename = std::str::from_utf8(&data[..nul_pos]).ok()?;

    // Check same directory as the library
    if let Some(dir) = lib.parent() {
        let candidate = dir.join(debug_filename);
        if candidate.exists() {
            return Some(candidate);
        }
        // Also check a .debug/ subdirectory
        let candidate = dir.join(".debug").join(debug_filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // Check /usr/lib/debug/ + original directory
    if let Some(dir) = lib.parent() {
        let candidate = Path::new("/usr/lib/debug")
            .join(dir.strip_prefix("/").unwrap_or(dir))
            .join(debug_filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Read the ELF `.note.gnu.build-id` section and check
/// `/usr/lib/debug/.build-id/<xx>/<rest>.debug`.
#[cfg(target_os = "linux")]
fn find_build_id_debug(lib: &Path) -> Option<PathBuf> {
    use object::{Object as _, ObjectSection as _};

    let bytes = std::fs::read(lib).ok()?;
    let object = object::File::parse(&*bytes).ok()?;

    // The build-id is in the .note.gnu.build-id section.
    // Format: 4-byte namesz, 4-byte descsz, 4-byte type, name, desc(build-id)
    let section = object.section_by_name(".note.gnu.build-id")?;
    let data = section.data().ok()?;
    if data.len() < 16 {
        return None;
    }

    let namesz = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let descsz = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    // Skip: type (4 bytes), name (namesz aligned to 4), then desc is the build-id
    let name_end = 12 + ((namesz + 3) & !3);
    if data.len() < name_end + descsz || descsz < 2 {
        return None;
    }
    let build_id = &data[name_end..name_end + descsz];

    // Convert to hex: first byte is directory, rest is filename
    let hex: String = build_id.iter().map(|b| format!("{:02x}", b)).collect();
    let (dir_part, file_part) = hex.split_at(2);
    let candidate = Path::new("/usr/lib/debug/.build-id")
        .join(dir_part)
        .join(format!("{}.debug", file_part));
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

// endregion

// region: DWARF resolution via addr2line

/// Cache of addr2line contexts, keyed by library path.
/// Library bytes are leaked to get a `'static` lifetime for gimli's
/// `EndianSlice`. This is bounded by the number of unique .so files
/// loaded per session (typically < 20).
struct DwarfCache {
    /// Map from library path → addr2line Context (or None if DWARF unavailable).
    contexts: HashMap<
        String,
        Option<addr2line::Context<gimli::EndianSlice<'static, gimli::RunTimeEndian>>>,
    >,
}

impl DwarfCache {
    fn new() -> Self {
        Self {
            contexts: HashMap::new(),
        }
    }

    /// Get or create an addr2line Context for the given library path.
    fn get_context(
        &mut self,
        library_path: &str,
    ) -> Option<&addr2line::Context<gimli::EndianSlice<'static, gimli::RunTimeEndian>>> {
        if !self.contexts.contains_key(library_path) {
            let ctx = Self::load_context(library_path);
            self.contexts.insert(library_path.to_string(), ctx);
        }
        self.contexts.get(library_path).and_then(|opt| opt.as_ref())
    }

    /// Try to load DWARF debug info from a shared library.
    /// Uses platform-specific discovery to find debug info in dSYM bundles,
    /// .gnu_debuglink targets, build-id paths, or the binary itself.
    fn load_context(
        library_path: &str,
    ) -> Option<addr2line::Context<gimli::EndianSlice<'static, gimli::RunTimeEndian>>> {
        let debug_path = find_debug_file(library_path);
        let bytes = std::fs::read(&debug_path).ok()?;
        // Leak the bytes to get 'static lifetime for gimli slices.
        // Bounded by number of unique libraries per session.
        let bytes: &'static [u8] = Vec::leak(bytes);

        use object::Object as _;
        let object = object::File::parse(bytes).ok()?;
        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };

        let dwarf = gimli::Dwarf::load(|section_id| -> Result<_, gimli::Error> {
            use object::ObjectSection as _;
            let data = object
                .section_by_name(section_id.name())
                .and_then(|s: object::Section<'_, '_>| s.uncompressed_data().ok())
                .unwrap_or(std::borrow::Cow::Borrowed(&[]));
            let slice: &'static [u8] = match data {
                std::borrow::Cow::Borrowed(b) => b,
                std::borrow::Cow::Owned(v) => Vec::leak(v),
            };
            Ok(gimli::EndianSlice::new(slice, endian))
        })
        .ok()?;

        addr2line::Context::from_dwarf(dwarf).ok()
    }
}

thread_local! {
    static DWARF_CACHE: std::cell::RefCell<DwarfCache> = std::cell::RefCell::new(DwarfCache::new());
}

/// Try to resolve file:line for an address using DWARF debug info.
fn dwarf_resolve(
    addr: usize,
    library_path: &str,
    library_base: usize,
) -> (Option<String>, Option<u32>) {
    // The address in the backtrace is absolute; DWARF uses relative offsets.
    let relative_addr = (addr as u64).wrapping_sub(library_base as u64);

    DWARF_CACHE
        .with(|cache| {
            let mut cache = cache.borrow_mut();
            let ctx = cache.get_context(library_path)?;

            if let Ok(Some(loc)) = ctx.find_location(relative_addr) {
                let file = loc.file.map(|f| {
                    // Show just the filename for short paths
                    Path::new(f)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(f)
                        .to_string()
                });
                let line = loc.line;
                Some((file, line))
            } else {
                None
            }
        })
        .unwrap_or((None, None))
}

// endregion

// region: Public API

/// Resolve a native backtrace into human-readable frames.
///
/// Filters out internal frames (trampoline, backtrace machinery) to show
/// only the interesting package code between the .Call entry and the error.
pub fn resolve_native_backtrace(frames: &[usize]) -> Vec<ResolvedFrame> {
    // First pass: dladdr resolution for all frames
    let mut dladdr_results: Vec<dladdr_ffi::DladdrResult> = frames
        .iter()
        .map(|&addr| dladdr_ffi::resolve(addr))
        .collect();

    // Second pass: DWARF resolution where we have library paths
    for result in &mut dladdr_results {
        if let Some(ref lib_path) = result.library_path {
            let (file, line) = dwarf_resolve(result.frame.address, lib_path, result.library_base);
            result.frame.file = file;
            result.frame.line = line;
        }
    }

    // Filter: skip error machinery frames, stop at trampoline
    let resolved: Vec<ResolvedFrame> = dladdr_results.into_iter().map(|r| r.frame).collect();
    let mut result = Vec::new();
    let mut started = false;
    for frame in &resolved {
        let name = frame.function.as_deref().unwrap_or("");

        if !started {
            if name.contains("Rf_error")
                || name.contains("Rf_errorcall")
                || name == "backtrace"
                || name.contains("longjmp")
                || name.contains("_sigtramp")
            {
                continue;
            }
            started = true;
        }

        if name.contains("_minir_call_protected") || name.contains("_minir_dotC_call_protected") {
            break;
        }

        result.push(frame.clone());
    }

    if result.is_empty() && !resolved.is_empty() {
        return resolved;
    }

    result
}

/// Format resolved native frames as indented lines for display under an R call frame.
pub fn format_native_frames(frames: &[ResolvedFrame]) -> String {
    let mut lines = Vec::with_capacity(frames.len());
    for frame in frames {
        let addr_fallback = format!("0x{:x}", frame.address);
        let func = frame.function.as_deref().unwrap_or(&addr_fallback);
        let lib = frame.library.as_deref().unwrap_or("???");

        // Build the location suffix: " at file.c:42" if DWARF info available
        let location = match (&frame.file, frame.line) {
            (Some(file), Some(line)) => format!(" at {}:{}", file, line),
            (Some(file), None) => format!(" at {}", file),
            _ => String::new(),
        };

        if frame.offset > 0 && location.is_empty() {
            lines.push(format!("   [C] {}+0x{:x} ({})", func, frame.offset, lib));
        } else {
            lines.push(format!("   [C] {}{} ({})", func, location, lib));
        }
    }
    lines.join("\n")
}

// endregion
