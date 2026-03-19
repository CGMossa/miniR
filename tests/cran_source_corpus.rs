//! CRAN package sourcing corpus test.
//!
//! Sources all R files from each CRAN package and reports success rates.
//! Only runs when MINIR_PARSE_CRAN=1 is set. Uses per-package Session
//! instances with catch_unwind for crash isolation.

use std::path::{Path, PathBuf};

use r::Session;

fn collect_packages(cran_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut packages = Vec::new();
    let entries = match std::fs::read_dir(cran_dir) {
        Ok(e) => e,
        Err(_) => return packages,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let r_dir = path.join("R");
            if r_dir.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                packages.push((name, r_dir));
            }
        }
    }
    packages.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    packages
}

#[test]
fn cran_source_corpus() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cran_dir = repo_root.join("cran");

    if std::env::var("MINIR_PARSE_CRAN").as_deref() != Ok("1") {
        eprintln!("Skipping CRAN source corpus (set MINIR_PARSE_CRAN=1)");
        return;
    }
    if !cran_dir.is_dir() {
        eprintln!("Skipping (cran/ not found)");
        return;
    }

    let packages = collect_packages(&cran_dir);
    assert!(!packages.is_empty());

    let mut total_ok = 0usize;
    let mut total_fail = 0usize;
    let mut total_crash = 0usize;
    let mut pkg_100 = Vec::new();
    let mut pkg_partial = Vec::new();
    let mut pkg_crash = Vec::new();

    for (name, r_dir) in &packages {
        let mut files: Vec<PathBuf> = std::fs::read_dir(r_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("R"))
            .collect();
        files.sort();
        if files.is_empty() {
            continue;
        }

        let result = std::panic::catch_unwind(|| {
            let mut s = Session::new();
            let mut ok = 0usize;
            let mut fail = 0usize;
            let mut first_err = String::new();
            for f in &files {
                let code = format!(
                    "source(\"{}\")",
                    f.to_string_lossy()
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                );
                match s.eval_source(&code) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        fail += 1;
                        if first_err.is_empty() {
                            first_err = format!("{e:?}");
                            if first_err.len() > 50 {
                                first_err.truncate(50);
                            }
                        }
                    }
                }
            }
            (ok, fail, first_err)
        });

        match result {
            Ok((ok, fail, err)) => {
                let total = ok + fail;
                let pct = ok * 100 / total.max(1);
                if fail == 0 {
                    eprintln!("{:<25} {:>4} / {:>4}  ({:>3}%)", name, ok, total, pct);
                    pkg_100.push(name.clone());
                } else {
                    eprintln!(
                        "{:<25} {:>4} / {:>4}  ({:>3}%)  | {}",
                        name, ok, total, pct, err
                    );
                    pkg_partial.push((name.clone(), pct));
                }
                total_ok += ok;
                total_fail += fail;
            }
            Err(_) => {
                eprintln!("{:<25}  CRASH", name);
                pkg_crash.push(name.clone());
                total_crash += 1;
            }
        }
    }

    let grand_total = total_ok + total_fail;
    let grand_pct = if grand_total > 0 {
        total_ok * 100 / grand_total
    } else {
        0
    };

    eprintln!();
    eprintln!("=== CRAN Source Corpus: {} packages ===", packages.len());
    eprintln!("  Files: {total_ok} / {grand_total} sourced OK ({grand_pct}%)");
    eprintln!("  100% packages: {}", pkg_100.len());
    eprintln!("  Partial packages: {}", pkg_partial.len());
    eprintln!("  Crashed packages: {total_crash}");
}
