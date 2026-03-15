use r::Session;

#[test]
fn lm_fits_simple_linear_regression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Simple linear regression: y = 2*x + 1, with some noise
df <- data.frame(x = c(1, 2, 3, 4, 5), y = c(3.0, 5.0, 7.0, 9.0, 11.0))
fit <- lm(y ~ x, data = df)

# Check it returns a list with class "lm"
stopifnot(inherits(fit, "lm"))
stopifnot(is.list(fit))

# Check coefficients — exact data: y = 2x + 1
coefs <- coef(fit)
stopifnot(length(coefs) == 2)
stopifnot(abs(coefs[1] - 1.0) < 1e-10)   # intercept
stopifnot(abs(coefs[2] - 2.0) < 1e-10)   # slope

# Check coefficient names
coef_names <- names(coefs)
stopifnot(identical(coef_names, c("(Intercept)", "x")))

# Check fitted values
fv <- fit$fitted.values
stopifnot(length(fv) == 5)
stopifnot(abs(fv[1] - 3.0) < 1e-10)
stopifnot(abs(fv[5] - 11.0) < 1e-10)

# Check residuals (should be ~0 for exact data)
res <- fit$residuals
stopifnot(length(res) == 5)
stopifnot(all(abs(res) < 1e-10))

# Summary should work without error
summary(fit)
"#,
    )
    .expect("lm() simple regression failed");
}

#[test]
fn lm_fits_multiple_regression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Multiple regression: y = 1 + 2*x1 + 3*x2 (exact)
df <- data.frame(
    x1 = c(1, 2, 3, 4, 5),
    x2 = c(2, 1, 3, 2, 4),
    y  = c(1 + 2*1 + 3*2, 1 + 2*2 + 3*1, 1 + 2*3 + 3*3, 1 + 2*4 + 3*2, 1 + 2*5 + 3*4)
)
fit <- lm(y ~ x1 + x2, data = df)

coefs <- coef(fit)
stopifnot(length(coefs) == 3)
stopifnot(abs(coefs[1] - 1.0) < 1e-8)   # intercept
stopifnot(abs(coefs[2] - 2.0) < 1e-8)   # x1
stopifnot(abs(coefs[3] - 3.0) < 1e-8)   # x2

# Coefficient names
coef_names <- names(coefs)
stopifnot(identical(coef_names, c("(Intercept)", "x1", "x2")))
"#,
    )
    .expect("lm() multiple regression failed");
}

#[test]
fn lm_rejects_missing_data_argument() {
    let mut s = Session::new();
    let err = s
        .eval_source("lm(y ~ x)")
        .expect_err("lm() without data should fail");

    assert!(
        err.to_string().contains("data"),
        "error should mention 'data': {err}"
    );
}

#[test]
fn coef_extracts_coefficients_from_list() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# coef() works on any list with $coefficients
obj <- list(coefficients = c(a = 1.5, b = 2.5))
result <- coef(obj)
stopifnot(identical(result, c(a = 1.5, b = 2.5)))
"#,
    )
    .expect("coef() extraction failed");
}

#[test]
fn summary_dispatches_to_summary_lm() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), y = c(2, 4, 6))
fit <- lm(y ~ x, data = df)
# summary() on an lm object should return the object (invisibly)
result <- summary(fit)
stopifnot(inherits(result, "lm"))
"#,
    )
    .expect("summary.lm dispatch failed");
}
