#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

target="${1:-x86_64-unknown-linux-gnu}"
output="${2:-}"
case "$target" in
  x86_64-unknown-linux-gnu)
    binary_name="first-principles-agent"
    ;;
  x86_64-pc-windows-gnu)
    binary_name="first-principles-agent.exe"
    ;;
  *)
    echo "unsupported reproducibility target: $target" >&2
    exit 1
    ;;
esac

dir_a="$(mktemp -d /tmp/1agent-repro-a.XXXXXX)"
dir_b="$(mktemp -d /tmp/1agent-repro-b.XXXXXX)"
trap 'rm -rf "$dir_a" "$dir_b"' EXIT

common_rustflags="--remap-path-prefix=$project_dir=/src"
common_rustflags+=" --remap-path-prefix=$dir_a=/target"
common_rustflags+=" --remap-path-prefix=$dir_b=/target"

for target_dir in "$dir_a" "$dir_b"; do
  CARGO_TARGET_DIR="$target_dir" \
  CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_RELEASE_DEBUG=0 \
  CARGO_PROFILE_RELEASE_STRIP=symbols \
  SOURCE_DATE_EPOCH=0 \
  RUSTFLAGS="$common_rustflags" \
    cargo build --offline --locked --release --target "$target" >/dev/null
done

artifact_a="$dir_a/$target/release/$binary_name"
artifact_b="$dir_b/$target/release/$binary_name"
hash_a="$(sha256sum "$artifact_a" | cut -d' ' -f1)"
hash_b="$(sha256sum "$artifact_b" | cut -d' ' -f1)"
size_a="$(stat -c '%s' "$artifact_a")"
size_b="$(stat -c '%s' "$artifact_b")"

if [[ "$hash_a" != "$hash_b" || "$size_a" != "$size_b" ]]; then
  printf 'reproducibility failed\na=%s %s\nb=%s %s\n' \
    "$hash_a" "$size_a" "$hash_b" "$size_b" >&2
  exit 1
fi

if [[ -n "$output" ]]; then
  mkdir -p "$(dirname "$output")"
  install -m 0755 "$artifact_a" "$output"
  installed_hash="$(sha256sum "$output" | cut -d' ' -f1)"
  if [[ "$installed_hash" != "$hash_a" ]]; then
    echo "installed artifact hash mismatch" >&2
    exit 1
  fi
fi

printf 'reproducible=true\ntarget=%s\nsha256=%s\nsize=%s\n' \
  "$target" "$hash_a" "$size_a"
