//! Integration tests for graphics device management builtins.

use r::Session;

#[test]
fn dev_cur_returns_1_when_no_devices_open() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(dev.cur() == 1L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_new_opens_device_and_returns_index() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        stopifnot(dev.cur() == 2L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_new_multiple_devices_increment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        dev.new()
        stopifnot(dev.cur() == 3L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_off_closes_current_and_reverts() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        dev.new()
        # Current is 3 (second device opened)
        stopifnot(dev.cur() == 3L)
        dev.off()
        # After closing 3, should revert to 2
        stopifnot(dev.cur() == 2L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_off_closes_last_device_reverts_to_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        stopifnot(dev.cur() == 2L)
        dev.off()
        stopifnot(dev.cur() == 1L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_off_with_explicit_which() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()  # device 2
        dev.new()  # device 3
        # Close device 2, current should remain 3
        dev.off(2L)
        stopifnot(dev.cur() == 3L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_set_switches_device() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()  # device 2
        dev.new()  # device 3
        stopifnot(dev.cur() == 3L)
        prev <- dev.set(2L)
        stopifnot(prev == 3L)
        stopifnot(dev.cur() == 2L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_set_to_null_device() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()  # device 2
        stopifnot(dev.cur() == 2L)
        prev <- dev.set(1L)
        stopifnot(prev == 2L)
        stopifnot(dev.cur() == 1L)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_list_returns_null_when_no_devices() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(is.null(dev.list()))
        "#,
    )
    .unwrap();
}

#[test]
fn dev_list_returns_named_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        dev.new()
        lst <- dev.list()
        stopifnot(length(lst) == 2L)
        stopifnot(lst[1] == 2L)
        stopifnot(lst[2] == 3L)
        stopifnot(names(lst)[1] == "null device")
        stopifnot(names(lst)[2] == "null device")
        "#,
    )
    .unwrap();
}

#[test]
fn graphics_off_closes_all_devices() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()
        dev.new()
        dev.new()
        stopifnot(dev.cur() == 4L)
        graphics.off()
        stopifnot(dev.cur() == 1L)
        stopifnot(is.null(dev.list()))
        "#,
    )
    .unwrap();
}

#[test]
fn dev_off_null_device_is_error() {
    let mut s = Session::new();
    let result = s.eval_source("dev.off(1L)");
    assert!(
        result.is_err(),
        "closing the null device should be an error"
    );
}

#[test]
fn dev_set_invalid_device_is_error() {
    let mut s = Session::new();
    let result = s.eval_source("dev.set(99L)");
    assert!(
        result.is_err(),
        "setting to a nonexistent device should be an error"
    );
}

#[test]
fn dev_off_nonexistent_device_is_error() {
    let mut s = Session::new();
    let result = s.eval_source("dev.off(99L)");
    assert!(
        result.is_err(),
        "closing a nonexistent device should be an error"
    );
}

#[test]
fn slot_reuse_after_close() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        dev.new()  # device 2
        dev.new()  # device 3
        dev.off(2L)  # close device 2
        dev.new()    # should reuse slot 2
        lst <- dev.list()
        # Should have devices at 2 and 3
        stopifnot(length(lst) == 2L)
        stopifnot(2L %in% lst)
        stopifnot(3L %in% lst)
        "#,
    )
    .unwrap();
}

#[test]
fn dev_new_returns_invisible_integer() {
    let mut s = Session::new();
    let output = s.eval_source("dev.new()").unwrap();
    // dev.new() should return invisibly
    assert!(!output.visible, "dev.new() should return invisibly");
}

#[test]
fn dev_off_returns_invisible_integer() {
    let mut s = Session::new();
    s.eval_source("dev.new()").unwrap();
    let output = s.eval_source("dev.off()").unwrap();
    // dev.off() should return invisibly
    assert!(!output.visible, "dev.off() should return invisibly");
}

#[test]
fn graphics_off_returns_invisible_null() {
    let mut s = Session::new();
    s.eval_source("dev.new()").unwrap();
    let output = s.eval_source("graphics.off()").unwrap();
    assert!(!output.visible, "graphics.off() should return invisibly");
}

#[test]
fn device_manager_isolation_between_sessions() {
    // Each session should have its own device manager
    let mut s1 = Session::new();
    let mut s2 = Session::new();

    s1.eval_source("dev.new()").unwrap();
    s1.eval_source("stopifnot(dev.cur() == 2L)").unwrap();

    // s2 should still be at null device
    s2.eval_source("stopifnot(dev.cur() == 1L)").unwrap();
    s2.eval_source("stopifnot(is.null(dev.list()))").unwrap();
}
