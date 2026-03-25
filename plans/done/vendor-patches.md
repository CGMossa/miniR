# Vendor Patch System

Create infrastructure for applying local patches to vendored dependencies without losing the ability to update them.

## Motivation

Sometimes we need to modify a vendored crate — e.g., making a transitive dependency optional, fixing a bug, or adapting an API. Currently, editing files under `vendor/` directly means `just vendor` would overwrite changes. We need a patch system that:

1. Stores patches separately in `vendor-patches/`
2. Applies patches after `just vendor` re-vendors
3. Fixes up cargo checksum hashes for patched crates only
4. Leaves unpatched crate hashes untouched

## Design

### Directory layout

```
vendor-patches/
├── reedline/
│   ├── 001-make-sqlite-optional.patch
│   └── 002-fix-validator-lifetime.patch
├── some-crate/
│   └── 001-description.patch
└── README.md
```

### Patch format

Standard `git diff` / unified diff format, created via:
```bash
cd vendor/reedline
# make changes
git diff > ../../vendor-patches/reedline/001-description.patch
```

### Workflow

1. `just vendor` downloads fresh crates (as before)
2. New `just vendor-patch` step runs after vendoring:
   - For each directory in `vendor-patches/`:
     - Apply all `.patch` files in sorted order via `patch -p1`
     - Recalculate the `.cargo-checksum.json` for that crate:
       - Read the existing checksum file
       - For each patched file, update its sha256 hash
       - Write back the checksum file
     - Leave unpatched crates' checksums untouched
3. `just vendor` is updated to call `just vendor-patch` automatically

### Hash recalculation

Each vendored crate has a `.cargo-checksum.json` containing:
```json
{"files": {"src/lib.rs": "sha256hash...", ...}, "package": "packagehash"}
```

For patched files:
- Recompute `sha256sum` of the modified file
- Update the entry in `"files"`
- Set `"package"` to an empty string (or remove it) — cargo only checks file hashes when package hash is missing

For unpatched crates: don't touch anything.

### Justfile recipe

```bash
vendor-patch:
    #!/usr/bin/env bash
    set -euo pipefail
    for patch_dir in vendor-patches/*/; do
        crate=$(basename "$patch_dir")
        if [ ! -d "vendor/$crate" ]; then
            echo "warning: vendor-patches/$crate has no matching vendor/$crate"
            continue
        fi
        for patch in "$patch_dir"*.patch; do
            [ -f "$patch" ] || continue
            echo "applying $patch to vendor/$crate"
            (cd "vendor/$crate" && patch -p1 < "../../$patch")
        done
        # recalculate checksums for patched crate
        scripts/fix-vendor-checksum.sh "vendor/$crate"
    done
```

## Prerequisites

- Write `scripts/fix-vendor-checksum.sh` — reads `.cargo-checksum.json`, recomputes sha256 for all files, writes back
- Create `vendor-patches/README.md` documenting the process
- Update `just vendor` to call `just vendor-patch` after vendoring
