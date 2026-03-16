#!/usr/bin/env bash
# Recalculate .cargo-checksum.json for a vendored crate after patching.
#
# Usage: fix-vendor-checksum.sh vendor/<crate>
#
# Reads the existing .cargo-checksum.json, recomputes sha256 for every file
# listed in it, clears the package hash (so cargo uses per-file checks), and
# writes the file back.

set -euo pipefail

CRATE_DIR="$1"
CHECKSUM_FILE="$CRATE_DIR/.cargo-checksum.json"

if [ ! -f "$CHECKSUM_FILE" ]; then
    echo "error: $CHECKSUM_FILE not found" >&2
    exit 1
fi

# Build a new "files" object by hashing every file listed in the original
NEW_FILES="{"
FIRST=true
for file in $(python3 -c "
import json, sys
with open('$CHECKSUM_FILE') as f:
    data = json.load(f)
for name in sorted(data.get('files', {}).keys()):
    print(name)
"); do
    filepath="$CRATE_DIR/$file"
    if [ -f "$filepath" ]; then
        hash=$(shasum -a 256 "$filepath" | cut -d' ' -f1)
        if [ "$FIRST" = true ]; then
            FIRST=false
        else
            NEW_FILES+=","
        fi
        NEW_FILES+="\"$file\":\"$hash\""
    fi
done
NEW_FILES+="}"

# Write back with package hash cleared
python3 -c "
import json, sys

files = json.loads(sys.argv[1])
out = {'files': files, 'package': ''}
with open('$CHECKSUM_FILE', 'w') as f:
    json.dump(out, f, sort_keys=True)
    f.write('\n')
" "$NEW_FILES"

echo "  updated $CHECKSUM_FILE"
