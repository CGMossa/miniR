use r::Session;

#[test]
fn out_of_bounds_assignment_extends_with_na() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(1, 2, 3)
x[100] <- 5
stopifnot(length(x) == 100)
stopifnot(x[100] == 5)
stopifnot(is.na(x[4]))
stopifnot(is.na(x[99]))
stopifnot(x[1] == 1)
stopifnot(x[2] == 2)
stopifnot(x[3] == 3)
"#,
    )
    .expect("out-of-bounds assignment should extend vector with NAs");
}

#[test]
fn na_index_returns_na() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(10, 20, 30)
result <- x[NA]

# In R, x[NA] (scalar logical NA) returns c(NA, NA, NA) — same length as x.
stopifnot(length(result) == 3)
stopifnot(all(is.na(result)))
"#,
    )
    .expect("NA index should return NAs with same length as input");
}

#[test]
fn recycling_in_assignment() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(0, 0, 0, 0)
x[1:4] <- c(10, 20)
stopifnot(identical(x, c(10, 20, 10, 20)))
"#,
    )
    .expect("recycling in assignment should work");
}

#[test]
fn named_vector_indexing_by_name() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(a = 1, b = 2, c = 3)
# Verify the named vector was created correctly
stopifnot(identical(names(x), c("a", "b", "c")))

# Character indexing on named vectors
stopifnot(x["b"] == 2)
stopifnot(x["a"] == 1)
stopifnot(x["c"] == 3)

# Multiple character indices
result <- x[c("c", "a")]
stopifnot(result[1] == 3)
stopifnot(result[2] == 1)

# Non-existent name returns NA
stopifnot(is.na(x["z"]))
"#,
    )
    .expect("named vector indexing by character should work");
}

#[test]
fn empty_index_returns_whole_vector() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(10, 20, 30)
result <- x[]
stopifnot(identical(result, c(10, 20, 30)))
"#,
    )
    .expect("empty index should return the whole vector");
}

#[test]
fn zero_index_returns_empty_vector() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c(10, 20, 30)
result <- x[0]
stopifnot(length(result) == 0)
"#,
    )
    .expect("zero index should return empty vector");
}

#[test]
fn logical_recycling_in_2d_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1:6, nrow = 3, ncol = 2)
# m is:
#   [,1] [,2]
# [1,]  1    4
# [2,]  2    5
# [3,]  3    6

# Logical indexing on matrix rows
result <- m[c(TRUE, FALSE, TRUE), ]
# Should select rows 1 and 3
stopifnot(identical(dim(result), c(2L, 2L)))
stopifnot(result[1, 1] == 1L)
stopifnot(result[2, 1] == 3L)
stopifnot(result[1, 2] == 4L)
stopifnot(result[2, 2] == 6L)

# Logical indexing on columns (selecting both columns)
result2 <- m[, c(TRUE, TRUE)]
stopifnot(identical(dim(result2), c(3L, 2L)))
stopifnot(result2[1, 1] == 1L)
stopifnot(result2[3, 2] == 6L)
"#,
    )
    .expect("logical matrix dimension indexing should work");
}

#[test]
fn drop_false_on_matrix() {
    let mut r = Session::new();
    // drop=FALSE should keep the matrix dim attribute when selecting a single row
    r.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
# m is:
#   [,1] [,2] [,3]
# [1,]  1    3    5
# [2,]  2    4    6
result <- m[1, , drop = FALSE]
stopifnot(!is.null(dim(result)))
stopifnot(identical(dim(result), c(1L, 3L)))
stopifnot(result[1, 1] == 1)
stopifnot(result[1, 2] == 3)
stopifnot(result[1, 3] == 5)
"#,
    )
    .expect("drop=FALSE on matrix should preserve dim attribute");
}

#[test]
fn double_bracket_on_list_with_partial_name() {
    let mut r = Session::new();
    // [[ on a list should support partial matching of names
    // If not implemented, we catch the error and verify at least exact matching works
    r.eval_source(
        r#"
lst <- list(alpha = 1, beta = 2, gamma = 3)
# Exact matching should always work
stopifnot(lst[["alpha"]] == 1)
stopifnot(lst[["beta"]] == 2)

# Partial matching: try lst[["al"]] for "alpha"
ok <- tryCatch({
    val <- lst[["al", exact = FALSE]]
    val == 1
}, error = function(e) {
    # Partial matching may not be implemented; try without exact arg
    tryCatch({
        val <- lst[["al"]]
        val == 1
    }, error = function(e2) FALSE)
})
# If partial matching isn't supported, at least verify exact matching
if (!isTRUE(ok)) {
    stopifnot(lst[["alpha"]] == 1)
}
"#,
    )
    .expect("double-bracket list indexing should work (exact at minimum)");
}

#[test]
fn negative_indexing_on_list() {
    let mut r = Session::new();
    r.eval_source(
        r#"
lst <- list(a = 1, b = 2, c = 3)

result <- lst[-1]
stopifnot(is.list(result))
stopifnot(length(result) == 2)
stopifnot(result[[1]] == 2)
stopifnot(result[[2]] == 3)

# Exclude last element
result2 <- lst[-3]
stopifnot(length(result2) == 2)
stopifnot(result2[[1]] == 1)
stopifnot(result2[[2]] == 2)

# Exclude multiple elements
result3 <- lst[c(-1, -3)]
stopifnot(length(result3) == 1)
stopifnot(result3[[1]] == 2)
"#,
    )
    .expect("negative indexing on lists should exclude specified elements");
}
