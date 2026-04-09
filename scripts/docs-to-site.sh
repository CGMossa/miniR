#!/usr/bin/env bash
# Convert docs/*.md files into Zola site content under site/content/manual/.
set -euo pipefail

DOCS_DIR="${1:-docs}"
CONTENT_DIR="${2:-site/content/manual}"

escape_toml_string() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

extract_description() {
  local doc="$1"
  local raw

  raw=$(
    tail -n +2 "$doc" | awk '
      BEGIN { capture = 0 }
      /^[[:space:]]*$/ {
        if (capture) exit
        next
      }
      /^#/ {
        if (capture) exit
        next
      }
      /^```/ {
        if (capture) exit
        next
      }
      /^[-*] / {
        if (capture) exit
        next
      }
      /^[0-9]+\. / {
        if (capture) exit
        next
      }
      {
        capture = 1
        print
      }
    '
  )

  if [ -z "$raw" ]; then
    return 0
  fi

  printf '%s' "$raw" \
    | tr '\n' ' ' \
    | sed -E \
      -e 's/[[:space:]]+/ /g' \
      -e 's/^ //; s/ $//' \
      -e 's/\[([^]]+)\]\([^)]+\)/\1/g' \
      -e 's/`//g' \
      -e 's/\*\*([^*]+)\*\*/\1/g' \
      -e 's/\*([^*]+)\*/\1/g'
}

weight_for_doc() {
  case "$1" in
    divergences) echo 1 ;;
    package_runtime) echo 2 ;;
    native_runtime) echo 3 ;;
    graphics_devices) echo 4 ;;
    backtraces) echo 5 ;;
    hdf5_summary) echo 20 ;;
    *) echo 50 ;;
  esac
}

# Remove old generated content (but keep _index.md)
find "$CONTENT_DIR" -maxdepth 1 -name '*.md' ! -name '_index.md' -delete

for doc in "$DOCS_DIR"/*.md; do
  basename=$(basename "$doc" .md)

  # Skip README
  [ "$basename" = "README" ] && continue

  title=$(head -1 "$doc" | sed 's/^# *//')
  description=$(extract_description "$doc")
  slug=$(printf '%s' "$basename" | tr '[:upper:]' '[:lower:]' | tr '_' '-')
  weight=$(weight_for_doc "$basename")

  {
    echo "+++"
    echo "title = \"$(escape_toml_string "$title")\""
    echo "weight = ${weight}"
    if [ -n "$description" ]; then
      echo "description = \"$(escape_toml_string "$description")\""
    fi
    echo "+++"
    echo ""
    tail -n +2 "$doc" | sed '1{
/^$/d
}'
  } > "$CONTENT_DIR/${slug}.md"
done

echo "Generated $(find "$CONTENT_DIR" -maxdepth 1 -name '*.md' ! -name '_index.md' | wc -l | tr -d ' ') pages from docs/"
