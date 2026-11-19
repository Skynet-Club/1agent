#!/bin/sh
# Build a local, reproducible pilot tarball. This does not publish anything.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$root"
version="${1:-0.1.0}"
out="dist/1ash-pilot-$version.tar.gz"
mkdir -p "$(dirname "$out")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/1ash-pilot-$version/harness" \
  "$tmp/1ash-pilot-$version/schemas" \
  "$tmp/1ash-pilot-$version/scripts"
cp -a harness/1ash "$tmp/1ash-pilot-$version/harness/1ash"
cp -a schemas/1ash "$tmp/1ash-pilot-$version/schemas/1ash"
cp scripts/check-1ash-schemas.py "$tmp/1ash-pilot-$version/scripts/"
cp docs/evidence/1ash-m2-m3-2026-11-19/suite-results.json "$tmp/1ash-pilot-$version/" 2>/dev/null || true
cp docs/evidence/1ash-m2-m3-2026-11-19/report-v0.json "$tmp/1ash-pilot-$version/" 2>/dev/null || true
find "$tmp" -name __pycache__ -type d -prune -exec rm -rf {} +
tar --sort=name --mtime='UTC 2026-11-19' --owner=0 --group=0 --numeric-owner \
  -czf "$out" -C "$tmp" "1ash-pilot-$version"
sha256sum "$out"
