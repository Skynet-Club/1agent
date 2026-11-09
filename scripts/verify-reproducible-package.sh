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
release_manifest="$project_dir/dist/1agent-$version-release.json"
first="$(mktemp -d /tmp/1agent-package-first.XXXXXX)"
second="$(mktemp -d /tmp/1agent-package-second.XXXXXX)"
tar_root="$(mktemp -d /tmp/1agent-package-tar.XXXXXX)"
zip_root="$(mktemp -d /tmp/1agent-package-zip.XXXXXX)"
trap 'rm -rf "$first" "$second" "$tar_root" "$zip_root"' EXIT

./scripts/package.sh >/dev/null
cp "$linux_archive" "$windows_archive" "$release_manifest" "$first/"

./scripts/package.sh >/dev/null
cp "$linux_archive" "$windows_archive" "$release_manifest" "$second/"

for artifact in \
  1agent-"$version"-linux-x86_64.tar.gz \
  1agent-"$version"-windows-x86_64.zip \
  1agent-"$version"-release.json; do
  if ! cmp "$first/$artifact" "$second/$artifact"; then
    echo "package reproducibility failed: $artifact" >&2
    exit 1
  fi
done

dist/1agent-"$version"/linux-x86_64/1agent release verify --manifest "$release_manifest"
tar -xzf "$linux_archive" -C "$tar_root"
unzip -q "$windows_archive" -d "$zip_root"
(
  cd "$tar_root/1agent-$version"
  sha256sum -c SHA256SUMS >/dev/null
  linux-x86_64/1agent provenance verify \
    --provenance provenance/linux-x86_64.json \
    --artifact linux-x86_64/1agent
)
(
  cd "$zip_root/1agent-$version"
  sha256sum -c SHA256SUMS >/dev/null
)
diff -r "$tar_root/1agent-$version" "$zip_root/1agent-$version"

sha256sum "$linux_archive" "$windows_archive" "$release_manifest"
echo "package reproducibility passed"
