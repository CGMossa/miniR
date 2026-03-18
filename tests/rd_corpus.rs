//! Rd documentation corpus test — parse all .Rd files from the CRAN corpus.
//!
//! Validates the Rd parser against real-world package documentation.
//! Only runs when MINIR_PARSE_CRAN=1 is set. Uses rayon for parallelism.
//! No catch_unwind — panics are bugs, not expected behavior.

#![cfg(feature = "parallel")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use r::interpreter::packages::rd::RdDoc;
use rayon::prelude::*;

fn collect_rd_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rd_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("Rd") {
            out.push(path);
        }
    }
}

#[test]
fn rd_corpus_parses() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cran_dir = repo_root.join("cran");

    if std::env::var("MINIR_PARSE_CRAN").as_deref() != Ok("1") {
        eprintln!("Skipping Rd corpus (set MINIR_PARSE_CRAN=1 to run)");
        return;
    }
    if !cran_dir.is_dir() {
        eprintln!("Skipping Rd corpus (cran/ directory not found)");
        return;
    }

    let mut files = Vec::new();
    collect_rd_files(&cran_dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .Rd files found in cran/");

    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let had_name = AtomicUsize::new(0);
    let had_title = AtomicUsize::new(0);
    let had_usage = AtomicUsize::new(0);
    let had_arguments = AtomicUsize::new(0);
    let had_examples = AtomicUsize::new(0);
    let failures: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

    files.par_iter().for_each(|path| {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => match std::fs::read(path) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(_) => return,
            },
        };

        match RdDoc::parse(&content) {
            Ok(doc) => {
                passed.fetch_add(1, Ordering::Relaxed);
                if doc.name.is_some() {
                    had_name.fetch_add(1, Ordering::Relaxed);
                }
                if doc.title.is_some() {
                    had_title.fetch_add(1, Ordering::Relaxed);
                }
                if doc.usage.is_some() {
                    had_usage.fetch_add(1, Ordering::Relaxed);
                }
                if !doc.arguments.is_empty() {
                    had_arguments.fetch_add(1, Ordering::Relaxed);
                }
                if doc.examples.is_some() {
                    had_examples.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                let rel = path
                    .strip_prefix(repo_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                failures.lock().unwrap().push((rel, format!("{e}")));
            }
        }
    });

    let p = passed.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    let total = p + f;
    let rate = if total > 0 {
        p as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let n = p.max(1) as f64;

    eprintln!();
    eprintln!("=== Rd Corpus: {total} files, {p} passed ({rate:.1}%), {f} failed ===");
    eprintln!(
        "  \\name:      {:>5}/{p} ({:>3.0}%)",
        had_name.load(Ordering::Relaxed),
        had_name.load(Ordering::Relaxed) as f64 / n * 100.0
    );
    eprintln!(
        "  \\title:     {:>5}/{p} ({:>3.0}%)",
        had_title.load(Ordering::Relaxed),
        had_title.load(Ordering::Relaxed) as f64 / n * 100.0
    );
    eprintln!(
        "  \\usage:     {:>5}/{p} ({:>3.0}%)",
        had_usage.load(Ordering::Relaxed),
        had_usage.load(Ordering::Relaxed) as f64 / n * 100.0
    );
    eprintln!(
        "  \\arguments: {:>5}/{p} ({:>3.0}%)",
        had_arguments.load(Ordering::Relaxed),
        had_arguments.load(Ordering::Relaxed) as f64 / n * 100.0
    );
    eprintln!(
        "  \\examples:  {:>5}/{p} ({:>3.0}%)",
        had_examples.load(Ordering::Relaxed),
        had_examples.load(Ordering::Relaxed) as f64 / n * 100.0
    );

    let mut all_failures = failures.into_inner().unwrap();
    all_failures.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    if !all_failures.is_empty() {
        let mut by_error: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for (path, err) in &all_failures {
            let key = if err.len() > 80 {
                format!("{}...", &err[..80])
            } else {
                err.clone()
            };
            by_error.entry(key).or_default().push(path.clone());
        }

        eprintln!();
        eprintln!("Failures by error type ({} total):", all_failures.len());
        for (err, paths) in &by_error {
            eprintln!("  [{:>3} files] {err}", paths.len());
            for path in paths.iter().take(3) {
                eprintln!("           - {path}");
            }
            if paths.len() > 3 {
                eprintln!("           ... and {} more", paths.len() - 3);
            }
        }

        let failure_path = repo_root.join("reviews/rd-corpus-failures.txt");
        let mut report = String::new();
        for (path, err) in &all_failures {
            report.push_str(&format!("{path}\t{err}\n"));
        }
        let _ = std::fs::write(&failure_path, &report);
        eprintln!();
        eprintln!("Full failure list: reviews/rd-corpus-failures.txt");
    }

    assert!(
        rate >= 99.0,
        "Rd parse rate too low: {rate:.1}% ({p}/{total})"
    );
}
