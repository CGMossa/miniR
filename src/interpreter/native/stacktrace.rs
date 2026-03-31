//! Native symbol resolution for stack traces.
//!
//! Two layers of resolution:
//! 1. **dladdr** — lightweight, zero deps: function name + library path
//! 2. **addr2line/gimli** — DWARF debug info: file:line for C code
//!
//! The DWARF layer is optional and gracefully falls back to dladdr-only
//! output when debug info is unavailable.

use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::path::Path;

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
struct DladdrResult {
    frame: ResolvedFrame,
    /// Full path to the shared library (for DWARF lookup).
    library_path: Option<String>,
    /// Base address where the library is loaded (for address rebasing).
    library_base: usize,
}

/// Resolve a single address via dladdr (lightweight, no DWARF).
fn dladdr_resolve(addr: usize) -> DladdrResult {
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

    /// Find the path containing DWARF debug info.
    /// On macOS, this is often `<path>.dSYM/Contents/Resources/DWARF/<filename>`.
    fn find_dwarf_path(library_path: &str) -> Option<String> {
        let lib = Path::new(library_path);
        let filename = lib.file_name()?.to_str()?;
        let dsym = lib
            .parent()?
            .join(format!("{}.dSYM", filename))
            .join("Contents")
            .join("Resources")
            .join("DWARF")
            .join(filename);
        if dsym.exists() {
            Some(dsym.to_string_lossy().into_owned())
        } else {
            None
        }
    }

    /// Try to load DWARF debug info from a shared library.
    /// On macOS, also checks for a `.dSYM` bundle next to the library.
    fn load_context(
        library_path: &str,
    ) -> Option<addr2line::Context<gimli::EndianSlice<'static, gimli::RunTimeEndian>>> {
        // On macOS, debug info is often in a .dSYM bundle rather than the binary.
        // Check for <path>.dSYM/Contents/Resources/DWARF/<filename> first.
        let dwarf_path = Self::find_dwarf_path(library_path);
        let bytes = std::fs::read(dwarf_path.as_deref().unwrap_or(library_path)).ok()?;
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
            // For borrowed data, the lifetime is 'static (from leaked bytes).
            // For owned data (decompressed), leak it too.
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
    let mut dladdr_results: Vec<DladdrResult> =
        frames.iter().map(|&addr| dladdr_resolve(addr)).collect();

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
