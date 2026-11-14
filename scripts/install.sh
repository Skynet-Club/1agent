#!/usr/bin/env bash
# Install a packaged 1agent binary into a prefix. This does not initialize,
# migrate or delete a runtime home; use `1agent init` / `1agent uninstall`.
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"
binary=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      prefix="${2:?--prefix requires a directory}"
      shift 2
      ;;
    --binary)
      binary="${2:?--binary requires a path}"
      shift 2
      ;;
    *)
      echo "usage: $0 [--prefix DIR] [--binary PATH]" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$binary" ]]; then
  packaged_binary="$project_dir/linux-x86_64/1agent"
  repo_binary=""
  if [[ -f "$project_dir/Cargo.toml" ]]; then
    version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
    repo_binary="$project_dir/dist/1agent-$version/linux-x86_64/1agent"
  fi
  if [[ -x "$packaged_binary" ]]; then
    binary="$packaged_binary"
  elif [[ -n "$repo_binary" ]]; then
    binary="$repo_binary"
  else
    binary="$packaged_binary"
  fi
fi

if [[ ! -x "$binary" ]]; then
  echo "packaged binary not found: $binary" >&2
  echo "run scripts/package.sh first, or pass --binary" >&2
  exit 1
fi

package_dir="$(cd "$(dirname "$binary")/.." && pwd)"
if [[ -f "$package_dir/SHA256SUMS" ]]; then
  if ! (cd "$package_dir" && grep -F 'linux-x86_64/1agent' SHA256SUMS | sha256sum -c - >/dev/null); then
    echo "packaged binary failed SHA256SUMS verification: $binary" >&2
    exit 1
  fi
fi

bindir="$prefix/bin"
mkdir -p "$bindir"
staged="$bindir/.1agent.new.$$"
if ! install -m 0755 "$binary" "$staged"; then
  rm -f "$staged"
  exit 1
fi
if ! staged_version="$("$staged" --version)"; then
  rm -f "$staged"
  echo "staged binary failed its version check; nothing was installed" >&2
  exit 1
fi
mv -f "$staged" "$bindir/1agent"
printf 'installed %s\n' "$bindir/1agent"
printf '%s\n' "$staged_version"
printf 'next: 1agent init\n'
