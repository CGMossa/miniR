//! Parallel CRAN package loading test.
//!
//! Uses one Session per package with thread-based parallelism.
//! Only runs when MINIR_PARSE_CRAN=1 is set.

#![cfg(feature = "native")]

use std::path::Path;
use std::sync::Mutex;
use std::thread;

use r::Session;

#[test]
fn cran_load_parallel() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cran_dir = repo_root.join("cran");

    if std::env::var("MINIR_PARSE_CRAN").as_deref() != Ok("1") {
        eprintln!("Skipping (set MINIR_PARSE_CRAN=1)");
        return;
    }
    if !cran_dir.is_dir() {
        eprintln!("Skipping (cran/ not found)");
        return;
    }

    let mut packages: Vec<String> = std::fs::read_dir(&cran_dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir() && e.path().join("DESCRIPTION").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    packages.sort();

    let total = packages.len();
    let ok_list = Mutex::new(Vec::new());
    let fail_list = Mutex::new(Vec::new());
    let hang_list = Mutex::new(Vec::new());

    // 8 threads for parallel loading
    let chunk_size = (total + 7) / 8;
    let chunks: Vec<_> = packages.chunks(chunk_size).map(|c| c.to_vec()).collect();

    thread::scope(|s| {
        for chunk in &chunks {
            let ok_list = &ok_list;
            let fail_list = &fail_list;
            let hang_list = &hang_list;
            let cran_dir = &cran_dir;
            thread::Builder::new()
                .stack_size(16 * 1024 * 1024)  // 16MB stack per thread
                .spawn_scoped(s, move || {
                for pkg in chunk {
                    // Skip packages known to stack-overflow or hang
                    if matches!(pkg.as_str(), "otel" | "opentelemetry") {
                        fail_list.lock().unwrap().push(pkg.clone());
                        continue;
                    }
                    // Skip packages with src/ that need long compilation
                    let has_native = cran_dir.join(pkg).join("src").is_dir();

                    let result = std::panic::catch_unwind(|| {
                        let mut sess = Session::new();
                        sess.eval_source("Sys.setenv(R_LIBS='cran')").ok();
                        if has_native {
                            // For native packages, use loadNamespace with error handling
                            sess.eval_source(&format!(
                                "tryCatch(loadNamespace(\"{pkg}\"), error=function(e) stop(e))"
                            ))
                        } else {
                            sess.eval_source(&format!("loadNamespace(\"{pkg}\")"))
                        }
                    });

                    match result {
                        Ok(Ok(_)) => ok_list.lock().unwrap().push(pkg.clone()),
                        Ok(Err(_)) => fail_list.lock().unwrap().push(pkg.clone()),
                        Err(_) => hang_list.lock().unwrap().push(pkg.clone()),
                    }
                }
            }).unwrap();
        }
    });

    let mut ok = ok_list.into_inner().unwrap();
    let mut fail = fail_list.into_inner().unwrap();
    let mut hang = hang_list.into_inner().unwrap();
    ok.sort();
    fail.sort();
    hang.sort();

    eprintln!("\n=== CRAN Package Loading ===");
    eprintln!(
        "OK: {}/{} ({:.1}%)",
        ok.len(),
        total,
        100.0 * ok.len() as f64 / total as f64
    );
    eprintln!("Fail: {}", fail.len());
    eprintln!("Crash: {}", hang.len());
    eprintln!("\nLoaded ({}):", ok.len());
    for p in &ok {
        eprint!("  {p}");
    }
    eprintln!();
    eprintln!("\nFailed ({}):", fail.len());
    for p in &fail {
        eprint!("  {p}");
    }
    eprintln!();
    if !hang.is_empty() {
        eprintln!("\nCrashed ({}):", hang.len());
        for p in &hang {
            eprint!("  {p}");
        }
        eprintln!();
    }
}
