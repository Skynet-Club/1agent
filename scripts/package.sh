#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "$version" ]]; then
  echo "cannot determine package version"
  exit 1
fi

package_root="$project_dir/dist/1agent-$version"
linux_archive="$project_dir/dist/1agent-$version-linux-x86_64.tar.gz"
windows_archive="$project_dir/dist/1agent-$version-windows-x86_64.zip"
release_manifest="$project_dir/dist/1agent-$version-release.json"
case "$package_root" in
  "$project_dir"/dist/1agent-*) ;;
  *)
    echo "unsafe package root: $package_root" >&2
    exit 1
    ;;
esac
rm -rf -- "$package_root"
rm -f -- "$linux_archive" "$windows_archive" "$release_manifest"
linux_dir="$package_root/linux-x86_64"
windows_dir="$package_root/windows-x86_64"
mkdir -p "$linux_dir" "$windows_dir"

./scripts/reproducible-build.sh x86_64-unknown-linux-gnu "$linux_dir/1agent"
./scripts/reproducible-build.sh x86_64-pc-windows-gnu "$windows_dir/1agent.exe"
mkdir -p "$package_root/provenance"
SOURCE_DATE_EPOCH=0 "$linux_dir/1agent" provenance create \
  --artifact "$linux_dir/1agent" \
  --target x86_64-unknown-linux-gnu \
  --output "$package_root/provenance/linux-x86_64.json" \
  >/dev/null
SOURCE_DATE_EPOCH=0 "$linux_dir/1agent" provenance create \
  --artifact "$windows_dir/1agent.exe" \
  --target x86_64-pc-windows-gnu \
  --output "$package_root/provenance/windows-x86_64.json" \
  >/dev/null
install -m 0644 README.md LICENSE CHANGELOG.md SECURITY.md PRIVACY.md \
  THIRD_PARTY_NOTICES.md THIRD_PARTY_LICENSES.md "$package_root/"
cp -R LICENSES "$package_root/"
mkdir -p "$package_root/scripts"
install -m 0755 scripts/install.sh "$package_root/scripts/install.sh"
cp -R docs evals configs "$package_root/"

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

echo "$package_root"
