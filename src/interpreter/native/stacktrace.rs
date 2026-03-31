//! Native symbol resolution for stack traces.
//!
//! Resolves raw instruction pointer addresses (captured by `backtrace()` in
//! the C trampoline) into human-readable function names and library paths
//! using `dladdr()`.

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
}

// dladdr FFI — available on macOS and Linux without additional linking.
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

/// Resolve a single instruction pointer address to a function name and library.
fn resolve_address(addr: usize) -> ResolvedFrame {
    let mut info: DlInfo = unsafe { std::mem::zeroed() };
    let ret = unsafe { dladdr(addr as *const c_void, &mut info) };
    if ret == 0 {
        return ResolvedFrame {
            address: addr,
            function: None,
            library: None,
            offset: 0,
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
    let library = if info.dli_fname.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(info.dli_fname) }
            .to_str()
            .ok()
            .map(|s| {
                // Show just the filename, not the full path
                Path::new(s)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(s)
                    .to_string()
            })
    };
    let offset = if info.dli_saddr.is_null() {
        0
    } else {
        addr.wrapping_sub(info.dli_saddr as usize)
    };
    ResolvedFrame {
        address: addr,
        function,
        library,
        offset,
    }
}

/// Resolve a native backtrace into human-readable frames.
///
/// Filters out internal frames (trampoline, backtrace machinery) to show
/// only the interesting package code between the .Call entry and the error.
pub fn resolve_native_backtrace(frames: &[usize]) -> Vec<ResolvedFrame> {
    let resolved: Vec<ResolvedFrame> = frames.iter().map(|&addr| resolve_address(addr)).collect();

    // Filter: skip frames from the error machinery (Rf_error, backtrace, longjmp)
    // and stop at the trampoline (_minir_call_protected / _minir_dotC_call_protected).
    let mut result = Vec::new();
    let mut started = false;
    for frame in &resolved {
        let name = frame.function.as_deref().unwrap_or("");

        // Skip the error/backtrace setup frames at the top
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

        // Stop at the trampoline — everything below is miniR internals
        if name.contains("_minir_call_protected") || name.contains("_minir_dotC_call_protected") {
            break;
        }

        result.push(frame.clone());
    }

    // If filtering left nothing (e.g., the error was directly in the trampoline),
    // return all resolved frames as a fallback.
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
        if frame.offset > 0 {
            lines.push(format!("   [C] {}+0x{:x} ({})", func, frame.offset, lib));
        } else {
            lines.push(format!("   [C] {} ({})", func, lib));
        }
    }
    lines.join("\n")
}
