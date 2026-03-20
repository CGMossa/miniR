# vendor-patches

Local patches applied to vendored crates after `cargo vendor`.

## Directory layout

```shell
vendor-patches/
  reedline/
    001-make-sqlite-optional.patch
    002-fix-validator-lifetime.patch
  some-crate/
    001-description.patch
```

## Creating a patch

1. Edit files directly under `vendor/<crate>/`
2. Run `just vendor-diff` to see what changed
3. Generate a patch file:

```bash
mkdir -p vendor-patches/<crate>
cd vendor/<crate>
# Make a fresh copy to diff against:
TMPDIR=$(mktemp -d)
cargo vendor "$TMPDIR" 2>/dev/null
diff -ruN "$TMPDIR/<crate>" . > ../../vendor-patches/<crate>/001-description.patch
rm -rf "$TMPDIR"
```

Or use `git diff` if the vendor directory is tracked:

```bash
cd vendor/<crate>
git diff . > ../../vendor-patches/<crate>/001-description.patch
```

1. Name patches with a numeric prefix for ordering: `001-`, `002-`, etc.
2. Use standard unified diff format (`diff -ruN` or `git diff`)

## Applying patches

Patches are applied automatically by `just vendor` and `just vendor-force` after
re-vendoring. You can also apply them manually:

```bash
just vendor-apply-patches
```

This applies each patch with `patch -p1` and recalculates `.cargo-checksum.json`
for the affected crates.

## Checking for local modifications

```bash
just vendor-diff
```

This vendors to a temp directory and diffs against the current `vendor/`,
showing any local modifications (including unapplied manual edits).
