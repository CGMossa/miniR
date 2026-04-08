#!/usr/bin/env bash
# Convert docs/*.md files into Zola site content under site/content/manual/.
set -euo pipefail

DOCS_DIR="${1:-docs}"
CONTENT_DIR="${2:-site/content/manual}"

# Remove old generated content (but keep _index.md)
find "$CONTENT_DIR" -maxdepth 1 -name '*.md' ! -name '_index.md' -delete

for doc in "$DOCS_DIR"/*.md; do
  basename=$(basename "$doc" .md)

  # Skip README
  [ "$basename" = "README" ] && continue

  # Extract title from first # heading
  title=$(head -1 "$doc" | sed 's/^# *//')

  # Convert filename to kebab-case slug
  slug=$(echo "$basename" | tr '[:upper:]' '[:lower:]' | tr '_' '-')

  # Write frontmatter + content (skip the # Title line)
  {
    echo "+++"
    echo "title = \"${title}\""
    echo "weight = 50"
    echo "+++"
    echo ""
    tail -n +2 "$doc" | sed '1{
/^$/d
}'
  } > "$CONTENT_DIR/${slug}.md"
done

echo "Generated $(find "$CONTENT_DIR" -maxdepth 1 -name '*.md' ! -name '_index.md' | wc -l | tr -d ' ') pages from docs/"
