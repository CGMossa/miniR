#!/usr/bin/env python3
"""
Filter the crates.io database dump into a curated overview of popular,
actively maintained crates with source repositories.

Usage:
    # Download the dump first:
    curl -L -o /tmp/db-dump.tar.gz https://static.crates.io/db-dump.tar.gz

    # Extract needed tables:
    cd /tmp && tar xzf db-dump.tar.gz --include='*/data/crates.csv' \
        --include='*/data/crate_downloads.csv' \
        --include='*/data/versions.csv' \
        --include='*/data/default_versions.csv' \
        --include='*/metadata.json'

    # Run the filter (auto-detects the dump directory by date prefix):
    python3 scripts/filter-crates-io.py /tmp/2026-03-27-020024

    # Or override defaults:
    python3 scripts/filter-crates-io.py /tmp/2026-03-27-020024 \
        --min-downloads 500000 --max-age-days 365

Output: analysis/crates-io-overview.csv
"""

import argparse
import csv
import re
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path


def is_prerelease(version: str) -> bool:
    """Return True if the version string looks like a pre-release or dev version.

    Filters out versions containing: alpha, beta, rc, dev, pre, nightly,
    or semver pre-release tags like 1.0.0-rc.1, 0.5.0-alpha.2, etc.
    """
    v = version.lower()
    # Semver pre-release: anything with a hyphen followed by a label
    if re.search(r'-\s*(alpha|beta|rc|dev|pre|nightly|canary|snapshot|preview|exp|unstable)', v):
        return True
    # Explicit non-semver patterns
    if re.search(r'\b(alpha|beta|nightly|canary|snapshot|preview)\b', v):
        return True
    # Versions like 0.0.0-dev or with +build metadata containing dev
    if re.search(r'-(dev|pre)\b', v):
        return True
    return False


def parse_semver_loose(version: str):
    """Parse a version string into a sortable tuple.

    Returns (major, minor, patch, pre_release_penalty) where
    pre_release_penalty is 0 for stable and 1 for pre-release.
    Falls back to string sorting for unparseable versions.
    """
    # Strip build metadata (+...)
    base = version.split('+')[0]
    # Split on hyphen to separate pre-release
    parts = base.split('-', 1)
    ver_str = parts[0]
    is_pre = len(parts) > 1

    # Parse major.minor.patch
    nums = ver_str.split('.')
    try:
        major = int(nums[0]) if len(nums) > 0 else 0
        minor = int(nums[1]) if len(nums) > 1 else 0
        patch = int(nums[2]) if len(nums) > 2 else 0
        return (major, minor, patch, 1 if is_pre else 0)
    except (ValueError, IndexError):
        return (0, 0, 0, 1)  # Unparseable → lowest priority


def main():
    parser = argparse.ArgumentParser(description="Filter crates.io dump for popular crates")
    parser.add_argument("dump_dir", help="Path to extracted dump directory (e.g. /tmp/2026-03-27-020024)")
    parser.add_argument("--min-downloads", type=int, default=1_000_000,
                        help="Minimum all-time downloads (default: 1000000)")
    parser.add_argument("--max-age-days", type=int, default=183,
                        help="Maximum days since last update (default: 183, ~6 months)")
    parser.add_argument("--output", default=None,
                        help="Output CSV path (default: analysis/crates-io-overview.csv)")
    args = parser.parse_args()

    dump = Path(args.dump_dir)
    data = dump / "data"

    if not data.exists():
        print(f"Error: {data} not found. Extract the dump first.", file=sys.stderr)
        sys.exit(1)

    # Determine output path
    if args.output:
        output_path = Path(args.output)
    else:
        # Default: analysis/ relative to this script's repo root
        script_dir = Path(__file__).resolve().parent
        output_path = script_dir.parent / "analysis" / "crates-io-overview.csv"

    cutoff = datetime.now(timezone.utc) - timedelta(days=args.max_age_days)
    csv.field_size_limit(sys.maxsize)

    # Step 1: Load download counts (crate_id -> downloads)
    print("Loading download counts...")
    downloads = {}
    with open(data / "crate_downloads.csv", "r", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            downloads[row["crate_id"]] = int(row["downloads"])

    high_dl_ids = {cid for cid, dl in downloads.items() if dl >= args.min_downloads}
    print(f"  {len(downloads):,} crates total, {len(high_dl_ids):,} with >= {args.min_downloads:,} downloads")

    # Step 2: Load default version mapping (crate_id -> version_id)
    print("Loading default versions...")
    default_version = {}  # crate_id -> version_id
    with open(data / "default_versions.csv", "r", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            default_version[row["crate_id"]] = row["version_id"]

    # Step 3: Load version info (version_id -> version details)
    # Only load versions for high-download crates to save memory
    print("Loading version data (filtered)...")
    # First, collect all version_ids we care about
    target_version_ids = {vid for cid, vid in default_version.items() if cid in high_dl_ids}

    # Also collect all versions per high-download crate for finding latest stable
    crate_versions = {}  # crate_id -> list of (version_str, yanked, version_id)
    version_info = {}  # version_id -> {num, yanked, ...}

    with open(data / "versions.csv", "r", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            crate_id = row["crate_id"]
            if crate_id not in high_dl_ids:
                continue

            vid = row["id"]
            num = row.get("num", "")
            yanked = row.get("yanked", "f") == "t"

            version_info[vid] = {"num": num, "yanked": yanked}

            if crate_id not in crate_versions:
                crate_versions[crate_id] = []
            crate_versions[crate_id].append((num, yanked, vid))

    # Step 4: For each crate, find the latest non-prerelease, non-yanked version
    print("Resolving latest stable versions...")
    latest_stable = {}  # crate_id -> version string
    for crate_id, versions in crate_versions.items():
        # Filter to non-yanked, non-prerelease
        stable = [(v, vid) for v, yanked, vid in versions
                   if not yanked and not is_prerelease(v)]
        if not stable:
            # Fall back to any non-yanked version
            stable = [(v, vid) for v, yanked, vid in versions if not yanked]
        if not stable:
            continue

        # Sort by semver descending
        stable.sort(key=lambda x: parse_semver_loose(x[0]), reverse=True)
        latest_stable[crate_id] = stable[0][0]

    # Step 5: Filter crates
    print("Filtering crates...")
    results = []
    skipped = {"downloads": 0, "repo": 0, "date": 0, "version": 0}

    with open(data / "crates.csv", "r", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            crate_id = row.get("id", "")

            if crate_id not in high_dl_ids:
                skipped["downloads"] += 1
                continue

            repo = row.get("repository", "").strip()
            if not repo:
                skipped["repo"] += 1
                continue

            updated_str = row.get("updated_at", "")
            try:
                updated = datetime.fromisoformat(updated_str.replace("+00", "+00:00"))
                if updated < cutoff:
                    skipped["date"] += 1
                    continue
            except (ValueError, TypeError):
                skipped["date"] += 1
                continue

            version = latest_stable.get(crate_id)
            if not version:
                skipped["version"] += 1
                continue

            dl_count = downloads.get(crate_id, 0)
            results.append({
                "name": row.get("name", ""),
                "version": version,
                "downloads": dl_count,
                "updated_at": updated_str[:10],
                "repository": repo,
                "description": row.get("description", "").replace("\n", " ").strip()[:200],
            })

    print(f"\n  Skipped: {skipped['downloads']:,} low-dl, {skipped['repo']:,} no-repo, "
          f"{skipped['date']:,} old, {skipped['version']:,} no-stable-version")
    print(f"  Result: {len(results):,} crates")

    # Sort by downloads descending
    results.sort(key=lambda r: r["downloads"], reverse=True)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=["name", "version", "downloads", "updated_at", "repository", "description"])
        writer.writeheader()
        writer.writerows(results)

    print(f"\nWrote {len(results):,} crates to {output_path}")


if __name__ == "__main__":
    main()
