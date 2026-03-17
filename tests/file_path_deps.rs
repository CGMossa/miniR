use r::Session;

// region: path.expand with dirs

#[test]
fn path_expand_tilde_resolves_to_home() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# path.expand("~") should return a non-empty, non-tilde string
home <- path.expand("~")
stopifnot(nchar(home) > 0)
stopifnot(!grepl("^~", home))

# path.expand("~/foo") should produce home/foo
expanded <- path.expand("~/foo")
stopifnot(endsWith(expanded, "/foo"))
stopifnot(startsWith(expanded, home))

# path.expand on an absolute path should be a no-op
stopifnot(path.expand("/tmp") == "/tmp")
"#,
    )
    .unwrap();
}

// endregion

// region: R.home

#[test]
fn r_home_returns_non_empty_path() {
    let mut s = Session::new();
    s.eval_source(
        r#"
home <- R.home()
stopifnot(is.character(home))
stopifnot(nchar(home) > 0)

# R.home with a component appends to the base
sub <- R.home("library")
stopifnot(endsWith(sub, "/library"))
stopifnot(startsWith(sub, home))
"#,
    )
    .unwrap();
}

// endregion

// region: .libPaths

#[test]
fn lib_paths_returns_character_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
paths <- .libPaths()
stopifnot(is.character(paths))
stopifnot(length(paths) >= 1)
stopifnot(nchar(paths[1]) > 0)
# Should end with /library
stopifnot(endsWith(paths[1], "/library"))
"#,
    )
    .unwrap();
}

// endregion

// region: list.files recursive

#[test]
fn list_files_recursive_finds_nested_files() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Create a temp directory structure
td <- tempdir()
base <- paste0(td, "/test_recursive")
dir.create(paste0(base, "/sub1"), recursive = TRUE)
dir.create(paste0(base, "/sub2"), recursive = TRUE)

# Create some files
file.create(paste0(base, "/a.txt"))
file.create(paste0(base, "/sub1/b.txt"))
file.create(paste0(base, "/sub2/c.txt"))

# Non-recursive should only find a.txt (not b.txt or c.txt)
flat <- list.files(base, pattern = "\\.txt$")
stopifnot(length(flat) == 1)
stopifnot("a.txt" %in% flat)

# Recursive should find all three files
deep <- list.files(base, pattern = "\\.txt$", recursive = TRUE)
stopifnot(length(deep) == 3)

# full.names should return absolute-ish paths
full <- list.files(base, pattern = "\\.txt$", recursive = TRUE, full.names = TRUE)
stopifnot(all(grepl(base, full)))
stopifnot(length(full) == 3)

# Clean up
unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

#[test]
fn list_files_full_names_flat() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
base <- paste0(td, "/test_fullnames")
dir.create(base, recursive = TRUE)
file.create(paste0(base, "/x.R"))
file.create(paste0(base, "/y.R"))

# full.names = FALSE (default) returns just filenames
short <- list.files(base, pattern = "\\.R$")
stopifnot(all(short %in% c("x.R", "y.R")))
stopifnot(!any(grepl("/", short)))

# full.names = TRUE returns full paths
long <- list.files(base, pattern = "\\.R$", full.names = TRUE)
stopifnot(all(grepl(base, long)))

unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.glob

#[test]
fn sys_glob_matches_files() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
base <- paste0(td, "/test_glob")
dir.create(base, recursive = TRUE)

file.create(paste0(base, "/foo.R"))
file.create(paste0(base, "/bar.R"))
file.create(paste0(base, "/baz.txt"))

# Glob for .R files
matches <- Sys.glob(paste0(base, "/*.R"))
stopifnot(length(matches) == 2)

# Glob for all files
all <- Sys.glob(paste0(base, "/*"))
stopifnot(length(all) == 3)

# Non-matching glob returns empty
empty <- Sys.glob(paste0(base, "/*.csv"))
stopifnot(length(empty) == 0)

unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

// endregion
