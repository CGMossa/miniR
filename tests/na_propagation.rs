use r::Session;

/// cumsum/cumprod/cummax/cummin must propagate NA forward:
/// once an NA appears, all subsequent values are NA.
#[test]
fn test_cumulative_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# cumsum: NA propagates forward
x <- cumsum(c(1, NA, 3))
stopifnot(identical(x[1], 1))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

# cumprod: NA propagates forward
x <- cumprod(c(2, NA, 3))
stopifnot(identical(x[1], 2))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

# cummax: NA propagates forward
x <- cummax(c(1, NA, 3))
stopifnot(identical(x[1], 1))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

# cummin: NA propagates forward
x <- cummin(c(3, NA, 1))
stopifnot(identical(x[1], 3))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))
"#,
    )
    .unwrap();
}

/// NA at the first position means ALL values are NA.
#[test]
fn test_cumulative_na_at_start() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- cumsum(c(NA, 1, 2))
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

x <- cumprod(c(NA, 2, 3))
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

x <- cummax(c(NA, 1, 2))
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))

x <- cummin(c(NA, 3, 1))
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))
"#,
    )
    .unwrap();
}

/// No NAs means normal cumulative behavior.
#[test]
fn test_cumulative_no_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(cumsum(c(1, 2, 3)), c(1, 3, 6)))
stopifnot(identical(cumprod(c(1, 2, 3)), c(1, 2, 6)))
stopifnot(identical(cummax(c(1, 3, 2)), c(1, 3, 3)))
stopifnot(identical(cummin(c(3, 1, 2)), c(3, 1, 1)))
"#,
    )
    .unwrap();
}

/// var/sd/median/range return NA by default (na.rm=FALSE) when input has NAs.
#[test]
fn test_stats_na_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# var returns NA
stopifnot(is.na(var(c(1, NA, 3))))

# sd returns NA
stopifnot(is.na(sd(c(1, NA, 3))))

# median returns NA
stopifnot(is.na(median(c(1, NA, 3))))

# range returns c(NA, NA)
r <- range(c(1, NA, 3))
stopifnot(is.na(r[1]))
stopifnot(is.na(r[2]))
stopifnot(length(r) == 2)
"#,
    )
    .unwrap();
}

/// With na.rm=TRUE, NAs are removed before computation.
#[test]
fn test_stats_na_rm_true() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# var(c(1,NA,3), na.rm=TRUE) == var(c(1,3))
stopifnot(identical(var(c(1, NA, 3), na.rm=TRUE), var(c(1, 3))))

# sd(c(1,NA,3), na.rm=TRUE) == sd(c(1,3))
stopifnot(identical(sd(c(1, NA, 3), na.rm=TRUE), sd(c(1, 3))))

# median(c(1,NA,3), na.rm=TRUE) = median(c(1,3)) = 2
stopifnot(identical(median(c(1, NA, 3), na.rm=TRUE), 2))

# range(c(1,NA,3), na.rm=TRUE) = c(1,3)
r <- range(c(1, NA, 3), na.rm=TRUE)
stopifnot(identical(r[1], 1))
stopifnot(identical(r[2], 3))
"#,
    )
    .unwrap();
}

/// All-NA input for stats functions.
#[test]
fn test_stats_all_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# All NA, na.rm=FALSE -> NA
stopifnot(is.na(var(c(NA, NA, NA))))
stopifnot(is.na(sd(c(NA, NA, NA))))
stopifnot(is.na(median(c(NA, NA, NA))))
r <- range(c(NA, NA, NA))
stopifnot(is.na(r[1]))
stopifnot(is.na(r[2]))
"#,
    )
    .unwrap();
}
