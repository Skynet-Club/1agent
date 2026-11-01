#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "$version" ]]; then
  echo "cannot determine package version" >&2
  exit 1
fi

linux_archive="$project_dir/dist/1agent-$version-linux-x86_64.tar.gz"
windows_archive="$project_dir/dist/1agent-$version-windows-x86_64.zip"
first="$(mktemp -d /tmp/1agent-package-first.XXXXXX)"
second="$(mktemp -d /tmp/1agent-package-second.XXXXXX)"
trap 'rm -rf "$first" "$second"' EXIT

./scripts/package.sh >/dev/null
cp "$linux_archive" "$windows_archive" "$first/"

./scripts/package.sh >/dev/null
cp "$linux_archive" "$windows_archive" "$second/"

for archive in 1agent-"$version"-linux-x86_64.tar.gz 1agent-"$version"-windows-x86_64.zip; do
  if ! cmp "$first/$archive" "$second/$archive"; then
    echo "package reproducibility failed: $archive" >&2
    exit 1
  fi
done

sha256sum "$linux_archive" "$windows_archive"
echo "package reproducibility passed"
