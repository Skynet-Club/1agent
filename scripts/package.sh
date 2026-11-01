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
linux_dir="$package_root/linux-x86_64"
windows_dir="$package_root/windows-x86_64"
mkdir -p "$linux_dir" "$windows_dir"

./scripts/reproducible-build.sh x86_64-unknown-linux-gnu "$linux_dir/1agent"
./scripts/reproducible-build.sh x86_64-pc-windows-gnu "$windows_dir/1agent.exe"
mkdir -p "$package_root/provenance"
"$linux_dir/1agent" provenance create \
  --artifact "$linux_dir/1agent" \
  --target x86_64-unknown-linux-gnu \
  --output "$package_root/provenance/linux-x86_64.json" \
  >/dev/null
"$linux_dir/1agent" provenance create \
  --artifact "$windows_dir/1agent.exe" \
  --target x86_64-pc-windows-gnu \
  --output "$package_root/provenance/windows-x86_64.json" \
  >/dev/null
install -m 0644 README.md LICENSE CHANGELOG.md "$package_root/"
cp -R docs evals configs "$package_root/"

(
  cd "$package_root"
  find . -type f ! -name SHA256SUMS -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    > SHA256SUMS
)

tar -C "$project_dir/dist" -czf \
  "$project_dir/dist/1agent-$version-linux-x86_64.tar.gz" \
  "1agent-$version"

(
  cd "$project_dir/dist"
  zip -qr "1agent-$version-windows-x86_64.zip" "1agent-$version"
)

echo "$package_root"
