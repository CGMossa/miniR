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

// region: list.files all.files parameter

#[test]
fn list_files_all_files_controls_hidden_files() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
base <- paste0(td, "/test_allfiles")
dir.create(base, recursive = TRUE)

# Create visible and hidden files
file.create(paste0(base, "/visible.txt"))
file.create(paste0(base, "/.hidden.txt"))

# Default (all.files = FALSE) should skip hidden files
visible <- list.files(base)
stopifnot(!any(grepl("^\\.hidden", visible)))
stopifnot("visible.txt" %in% visible)

# all.files = TRUE should include hidden files
all <- list.files(base, all.files = TRUE)
stopifnot(".hidden.txt" %in% all)
stopifnot("visible.txt" %in% all)

# Recursive with all.files
dir.create(paste0(base, "/sub"), recursive = TRUE)
file.create(paste0(base, "/sub/.secret"))
deep_hidden <- list.files(base, recursive = TRUE, all.files = TRUE)
stopifnot(any(grepl("\\.secret", deep_hidden)))
deep_no_hidden <- list.files(base, recursive = TRUE, all.files = FALSE)
stopifnot(!any(grepl("\\.secret", deep_no_hidden)))

unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

// endregion

// region: list.dirs

#[test]
fn list_dirs_finds_subdirectories() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
base <- paste0(td, "/test_listdirs")
dir.create(paste0(base, "/a"), recursive = TRUE)
dir.create(paste0(base, "/b"), recursive = TRUE)
dir.create(paste0(base, "/b/c"), recursive = TRUE)
file.create(paste0(base, "/file.txt"))

# Recursive (default) should find all dirs including base
dirs_r <- list.dirs(base, full.names = FALSE)
stopifnot("." %in% dirs_r)
stopifnot("a" %in% dirs_r)
stopifnot("b" %in% dirs_r)

# Non-recursive should only find immediate children
dirs_nr <- list.dirs(base, recursive = FALSE, full.names = FALSE)
stopifnot("." %in% dirs_nr)
stopifnot("a" %in% dirs_nr)
stopifnot("b" %in% dirs_nr)
# b/c should NOT appear in non-recursive mode
stopifnot(!any(grepl("b/c", dirs_nr)))

# full.names = TRUE should return full paths
dirs_full <- list.dirs(base, full.names = TRUE, recursive = FALSE)
stopifnot(all(nchar(dirs_full) > 1))

unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

// endregion

// region: file.mtime

#[test]
fn file_mtime_returns_posixct_timestamps() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
f <- paste0(td, "/test_mtime.txt")
file.create(f)

mt <- file.mtime(f)

# Should be a numeric value (seconds since epoch)
stopifnot(is.numeric(mt))

# Should be a reasonable timestamp (after 2020-01-01)
stopifnot(mt > 1577836800)

# Should have POSIXct class
stopifnot(inherits(mt, "POSIXct"))

# Non-existent file should return NA
mt_bad <- file.mtime(paste0(td, "/no_such_file_xyz"))
stopifnot(is.na(mt_bad))

unlink(f)
"#,
    )
    .unwrap();
}

// endregion

// region: dir.create parameters

#[test]
fn dir_create_supports_recursive_and_show_warnings() {
    let mut s = Session::new();
    s.eval_source(
        r#"
td <- tempdir()
base <- paste0(td, "/test_dircreate")

# recursive = TRUE (default in miniR) creates nested dirs
deep <- paste0(base, "/a/b/c")
result <- dir.create(deep)
stopifnot(result == TRUE)
stopifnot(dir.exists(deep))

# showWarnings = TRUE (default) returns FALSE on failure instead of error
result2 <- dir.create(deep, showWarnings = TRUE)
# dir already exists, so this is a no-op that may return FALSE
# (for create_dir_all it succeeds; for create_dir it fails)

# recursive = FALSE should fail for nested non-existent paths
deep2 <- paste0(base, "/x/y/z")
result3 <- dir.create(deep2, recursive = FALSE, showWarnings = TRUE)
stopifnot(result3 == FALSE)

unlink(base, recursive = TRUE)
"#,
    )
    .unwrap();
}

// endregion
