#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

linux_only=false
case "${1:-}" in
  "")
    ;;
  --linux-only)
    linux_only=true
    ;;
  *)
    echo "usage: $0 [--linux-only]" >&2
    exit 2
    ;;
esac
if [[ $# -gt 1 ]]; then
  echo "usage: $0 [--linux-only]" >&2
  exit 2
fi

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "$version" ]]; then
  echo "cannot determine package version"
  exit 1
fi

umask 022
if [[ -n "$(git status --porcelain)" ]]; then
  echo "refusing to package a dirty tree; commit or stash first" >&2
  exit 1
fi
tag="v$version"
if ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  echo "refusing to package without tag $tag" >&2
  exit 1
fi
if [[ "$(git rev-list -n 1 "$tag")" != "$(git rev-parse HEAD)" ]]; then
  echo "refusing to package: HEAD is not $tag" >&2
  exit 1
fi

package_root="$project_dir/dist/1agent-$version"
linux_archive="$project_dir/dist/1agent-$version-linux-x86_64.tar.gz"
windows_archive="$project_dir/dist/1agent-$version-windows-x86_64.zip"
release_manifest="$project_dir/dist/1agent-$version-release.json"
release_provenance="$project_dir/dist/linux-x86_64.json"
release_sums="$project_dir/dist/SHA256SUMS"
case "$package_root" in
  "$project_dir"/dist/1agent-*) ;;
  *)
    echo "unsafe package root: $package_root" >&2
    exit 1
    ;;
esac
rm -rf -- "$package_root"
rm -f -- "$linux_archive" "$windows_archive" "$release_manifest" \
  "$release_provenance" "$release_sums"
linux_dir="$package_root/linux-x86_64"
windows_dir="$package_root/windows-x86_64"
mkdir -p "$linux_dir"
if ! $linux_only; then
  mkdir -p "$windows_dir"
fi

./scripts/reproducible-build.sh x86_64-unknown-linux-gnu "$linux_dir/1agent"
mkdir -p "$package_root/provenance"
(
  cd "$linux_dir"
  SOURCE_DATE_EPOCH=0 ./1agent provenance create \
    --artifact 1agent \
    --target x86_64-unknown-linux-gnu \
    --output "$package_root/provenance/linux-x86_64.json" \
    >/dev/null
)
if ! $linux_only; then
  ./scripts/reproducible-build.sh x86_64-pc-windows-gnu "$windows_dir/1agent.exe"
  (
    cd "$windows_dir"
    SOURCE_DATE_EPOCH=0 "$linux_dir/1agent" provenance create \
      --artifact 1agent.exe \
      --target x86_64-pc-windows-gnu \
      --output "$package_root/provenance/windows-x86_64.json" \
      >/dev/null
  )
fi
install -m 0644 README.md README.en.md LICENSE CHANGELOG.md SECURITY.md PRIVACY.md \
  THIRD_PARTY_NOTICES.md THIRD_PARTY_LICENSES.md "$package_root/"
cp -R LICENSES "$package_root/"
mkdir -p "$package_root/scripts"
install -m 0755 scripts/install.sh "$package_root/scripts/install.sh"
cp -R docs evals configs "$package_root/"
find "$package_root" -type d -exec chmod 0755 {} +
find "$package_root" -type f -exec chmod 0644 {} +
chmod 0755 "$linux_dir/1agent" "$package_root/scripts/install.sh"
if ! $linux_only; then
  chmod 0644 "$windows_dir/1agent.exe"
fi

(
  cd "$package_root"
  find . -type f ! -name SHA256SUMS -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    > SHA256SUMS
)

TZ=UTC find "$package_root" -exec touch -h -t 198001010000 {} +

(
  cd "$project_dir/dist"
  TZ=UTC tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
    --format=gnu -cf - "1agent-$version" \
    | gzip -n -9 > "$linux_archive"
)

if ! $linux_only; then
  (
    cd "$project_dir/dist"
    find "1agent-$version" -type f -print \
      | LC_ALL=C sort \
      | TZ=UTC zip -X -D -q "$windows_archive" -@
  )

  "$linux_dir/1agent" release manifest \
    --package-root "$package_root" \
    --linux-archive "$linux_archive" \
    --windows-archive "$windows_archive" \
    --output "$release_manifest" \
    >/dev/null
else
  cp "$package_root/provenance/linux-x86_64.json" "$release_provenance"
  (
    cd "$project_dir/dist"
    sha256sum "1agent-$version-linux-x86_64.tar.gz" "linux-x86_64.json" \
      > SHA256SUMS
  )
fi

echo "$package_root"
if $linux_only; then
  echo "public assets:"
  echo "  $linux_archive"
  echo "  $release_sums"
  echo "  $release_provenance"
fi
