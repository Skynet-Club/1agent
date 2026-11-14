#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

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
    cargo build --offline --locked --release --target x86_64-unknown-linux-gnu \
      >/dev/null
done

artifact_a="$dir_a/x86_64-unknown-linux-gnu/release/first-principles-agent"
artifact_b="$dir_b/x86_64-unknown-linux-gnu/release/first-principles-agent"

hash_a="$(sha256sum "$artifact_a" | cut -d' ' -f1)"
hash_b="$(sha256sum "$artifact_b" | cut -d' ' -f1)"
size_a="$(stat -c '%s' "$artifact_a")"
size_b="$(stat -c '%s' "$artifact_b")"

printf 'hash_a=%s\nhash_b=%s\nsize_a=%s\nsize_b=%s\nequal=%s\n' \
  "$hash_a" "$hash_b" "$size_a" "$size_b" \
  "$([[ "$hash_a" == "$hash_b" && "$size_a" == "$size_b" ]] && echo true || echo false)"
