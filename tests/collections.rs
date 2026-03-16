//! Integration tests for the `collections` feature — HashMap, BTreeMap,
//! HashSet, BinaryHeap, and VecDeque exposed as R data structures.

use r::Session;

// region: HashMap

#[test]
fn hashmap_create_set_get() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()
stopifnot(is.integer(h))
stopifnot(inherits(h, "hashmap"))

hashmap_set(h, "x", 42)
stopifnot(hashmap_get(h, "x") == 42)

# Update existing key
hashmap_set(h, "x", 99)
stopifnot(hashmap_get(h, "x") == 99)
"#,
    )
    .expect("hashmap create/set/get");
}

#[test]
fn hashmap_get_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()
stopifnot(is.null(hashmap_get(h, "missing")))
stopifnot(hashmap_get(h, "missing", default = -1) == -1)
"#,
    )
    .expect("hashmap get with default");
}

#[test]
fn hashmap_has_and_remove() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()
hashmap_set(h, "a", 1)
stopifnot(hashmap_has(h, "a"))
stopifnot(!hashmap_has(h, "b"))

old <- hashmap_remove(h, "a")
stopifnot(old == 1)
stopifnot(!hashmap_has(h, "a"))
stopifnot(is.null(hashmap_remove(h, "a")))
"#,
    )
    .expect("hashmap has/remove");
}

#[test]
fn hashmap_keys_values_size() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()
hashmap_set(h, "a", 10)
hashmap_set(h, "b", 20)
hashmap_set(h, "c", 30)

stopifnot(hashmap_size(h) == 3L)

keys <- hashmap_keys(h)
stopifnot(length(keys) == 3L)
stopifnot(all(c("a", "b", "c") %in% keys))

vals <- hashmap_values(h)
stopifnot(length(vals) == 3L)
"#,
    )
    .expect("hashmap keys/values/size");
}

#[test]
fn hashmap_to_list() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()
hashmap_set(h, "x", 1)
hashmap_set(h, "y", "hello")

lst <- hashmap_to_list(h)
stopifnot(is.list(lst))
stopifnot(length(lst) == 2L)
stopifnot(!is.null(names(lst)))
stopifnot(lst[["x"]] == 1)
stopifnot(lst[["y"]] == "hello")
"#,
    )
    .expect("hashmap to list");
}

// endregion

// region: BTreeMap

#[test]
fn btreemap_ordered_keys() {
    let mut s = Session::new();
    s.eval_source(
        r#"
bt <- btreemap()
stopifnot(inherits(bt, "btreemap"))

btreemap_set(bt, "cherry", 3)
btreemap_set(bt, "apple", 1)
btreemap_set(bt, "banana", 2)

# BTreeMap keys are always sorted
keys <- btreemap_keys(bt)
stopifnot(identical(keys, c("apple", "banana", "cherry")))

stopifnot(btreemap_size(bt) == 3L)
"#,
    )
    .expect("btreemap ordered keys");
}

#[test]
fn btreemap_get_has_remove() {
    let mut s = Session::new();
    s.eval_source(
        r#"
bt <- btreemap()
btreemap_set(bt, "k", 42)
stopifnot(btreemap_get(bt, "k") == 42)
stopifnot(btreemap_has(bt, "k"))

btreemap_remove(bt, "k")
stopifnot(!btreemap_has(bt, "k"))
stopifnot(is.null(btreemap_get(bt, "k")))
stopifnot(btreemap_get(bt, "k", default = -1) == -1)
"#,
    )
    .expect("btreemap get/has/remove");
}

#[test]
fn btreemap_to_list_preserves_order() {
    let mut s = Session::new();
    s.eval_source(
        r#"
bt <- btreemap()
btreemap_set(bt, "z", 26)
btreemap_set(bt, "a", 1)
btreemap_set(bt, "m", 13)

lst <- btreemap_to_list(bt)
stopifnot(identical(names(lst), c("a", "m", "z")))
stopifnot(lst[["a"]] == 1)
stopifnot(lst[["m"]] == 13)
stopifnot(lst[["z"]] == 26)
"#,
    )
    .expect("btreemap to list preserves order");
}

#[test]
fn btreemap_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
bt <- btreemap()
btreemap_set(bt, "b", 2)
btreemap_set(bt, "a", 1)

vals <- btreemap_values(bt)
stopifnot(is.list(vals))
# values are in key-sorted order (a, b)
stopifnot(vals[[1]] == 1)
stopifnot(vals[[2]] == 2)
"#,
    )
    .expect("btreemap values in sorted order");
}

// endregion

// region: HashSet

#[test]
fn hashset_add_has_remove() {
    let mut s = Session::new();
    s.eval_source(
        r#"
s <- hashset()
stopifnot(inherits(s, "hashset"))

was_new <- hashset_add(s, "hello")
stopifnot(was_new)

was_new2 <- hashset_add(s, "hello")
stopifnot(!was_new2)

stopifnot(hashset_has(s, "hello"))
stopifnot(!hashset_has(s, "world"))
stopifnot(hashset_size(s) == 1L)

removed <- hashset_remove(s, "hello")
stopifnot(removed)
stopifnot(!hashset_has(s, "hello"))
stopifnot(hashset_size(s) == 0L)
"#,
    )
    .expect("hashset add/has/remove");
}

#[test]
fn hashset_to_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
s <- hashset()
hashset_add(s, "a")
hashset_add(s, "b")
hashset_add(s, "c")

v <- hashset_to_vector(s)
stopifnot(is.character(v))
stopifnot(length(v) == 3L)
stopifnot(all(c("a", "b", "c") %in% v))
"#,
    )
    .expect("hashset to vector");
}

#[test]
fn hashset_union_intersect_diff() {
    let mut s = Session::new();
    s.eval_source(
        r#"
s1 <- hashset()
hashset_add(s1, "a")
hashset_add(s1, "b")
hashset_add(s1, "c")

s2 <- hashset()
hashset_add(s2, "b")
hashset_add(s2, "c")
hashset_add(s2, "d")

# Union
u <- hashset_union(s1, s2)
stopifnot(hashset_size(u) == 4L)
stopifnot(hashset_has(u, "a"))
stopifnot(hashset_has(u, "d"))

# Intersection
inter <- hashset_intersect(s1, s2)
stopifnot(hashset_size(inter) == 2L)
stopifnot(hashset_has(inter, "b"))
stopifnot(hashset_has(inter, "c"))
stopifnot(!hashset_has(inter, "a"))

# Difference (s1 - s2)
d <- hashset_diff(s1, s2)
stopifnot(hashset_size(d) == 1L)
stopifnot(hashset_has(d, "a"))
stopifnot(!hashset_has(d, "b"))
"#,
    )
    .expect("hashset union/intersect/diff");
}

// endregion

// region: BinaryHeap

#[test]
fn heap_push_pop_peek() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- heap()
stopifnot(inherits(h, "heap"))
stopifnot(heap_size(h) == 0L)

# Empty heap returns NULL
stopifnot(is.null(heap_pop(h)))
stopifnot(is.null(heap_peek(h)))

heap_push(h, 10)
heap_push(h, 30)
heap_push(h, 20)

stopifnot(heap_size(h) == 3L)

# Max-heap: peek and pop should return the largest value
stopifnot(heap_peek(h) == 30)
stopifnot(heap_pop(h) == 30)
stopifnot(heap_pop(h) == 20)
stopifnot(heap_pop(h) == 10)
stopifnot(heap_size(h) == 0L)
"#,
    )
    .expect("heap push/pop/peek");
}

#[test]
fn heap_with_negative_and_fractional_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- heap()
heap_push(h, -5.5)
heap_push(h, 3.14)
heap_push(h, 0)
heap_push(h, -100)

stopifnot(heap_pop(h) == 3.14)
stopifnot(heap_pop(h) == 0)
stopifnot(heap_pop(h) == -5.5)
stopifnot(heap_pop(h) == -100)
"#,
    )
    .expect("heap with negative and fractional values");
}

// endregion

// region: VecDeque

#[test]
fn deque_push_pop_both_ends() {
    let mut s = Session::new();
    s.eval_source(
        r#"
d <- deque()
stopifnot(inherits(d, "deque"))
stopifnot(deque_size(d) == 0L)

# Empty deque returns NULL
stopifnot(is.null(deque_pop_front(d)))
stopifnot(is.null(deque_pop_back(d)))

deque_push_back(d, "a")
deque_push_back(d, "b")
deque_push_front(d, "z")

# Should be: z, a, b
stopifnot(deque_size(d) == 3L)
stopifnot(deque_pop_front(d) == "z")
stopifnot(deque_pop_back(d) == "b")
stopifnot(deque_pop_front(d) == "a")
stopifnot(deque_size(d) == 0L)
"#,
    )
    .expect("deque push/pop both ends");
}

#[test]
fn deque_to_list() {
    let mut s = Session::new();
    s.eval_source(
        r#"
d <- deque()
deque_push_back(d, 1)
deque_push_back(d, "two")
deque_push_back(d, TRUE)

lst <- deque_to_list(d)
stopifnot(is.list(lst))
stopifnot(length(lst) == 3L)
stopifnot(lst[[1]] == 1)
stopifnot(lst[[2]] == "two")
stopifnot(lst[[3]] == TRUE)
"#,
    )
    .expect("deque to list");
}

#[test]
fn deque_mixed_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
d <- deque()
deque_push_back(d, c(1, 2, 3))
deque_push_back(d, list(a = 1))
deque_push_back(d, NULL)

stopifnot(deque_size(d) == 3L)

v <- deque_pop_front(d)
stopifnot(identical(v, c(1, 2, 3)))
"#,
    )
    .expect("deque with mixed values");
}

// endregion

// region: Cross-collection tests

#[test]
fn multiple_collections_coexist() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Create one of each type and make sure they don't interfere
hm <- hashmap()
bt <- btreemap()
hs <- hashset()
hp <- heap()
dq <- deque()

hashmap_set(hm, "k", 1)
btreemap_set(bt, "k", 2)
hashset_add(hs, "k")
heap_push(hp, 42)
deque_push_back(dq, "hello")

stopifnot(hashmap_get(hm, "k") == 1)
stopifnot(btreemap_get(bt, "k") == 2)
stopifnot(hashset_has(hs, "k"))
stopifnot(heap_peek(hp) == 42)
stopifnot(deque_pop_front(dq) == "hello")
"#,
    )
    .expect("multiple collection types coexist");
}

#[test]
fn hashmap_stores_complex_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- hashmap()

# Store a vector
hashmap_set(h, "vec", c(1, 2, 3))
v <- hashmap_get(h, "vec")
stopifnot(identical(v, c(1, 2, 3)))

# Store a list
hashmap_set(h, "lst", list(a = 1, b = "x"))
lst <- hashmap_get(h, "lst")
stopifnot(is.list(lst))
stopifnot(lst$a == 1)
stopifnot(lst$b == "x")

# Store NULL
hashmap_set(h, "null_val", NULL)
stopifnot(is.null(hashmap_get(h, "null_val")))
"#,
    )
    .expect("hashmap stores complex R values");
}

// endregion
