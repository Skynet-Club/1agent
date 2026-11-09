#!/usr/bin/env bash
# Check that every ✅ in docs/claims.md has evidence that can actually run.
#
#   ./scripts/verify-claims.sh          structural check only (fast, offline)
#   ./scripts/verify-claims.sh --run    also execute each ✅ evidence
#
# Deliberately does not auto-match claims to tests: a keyword match would be
# the next silent gate. The mapping is written by hand; this script only
# enforces that the mapping is well-formed and that it still executes.
set -uo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

# Same reason as scripts/verify.sh: /tmp is quota-limited on this host, and the
# evidence runner spawns cargo tests that write temporary files.
export TMPDIR="$HOME/.cache/1agent-tmp"
mkdir -p "$TMPDIR"

doc="docs/claims.md"
run=0
[[ "${1-}" == "--run" ]] && run=1

if [[ ! -f "$doc" ]]; then
  printf 'missing %s\n' "$doc" >&2
  exit 1
fi

rows="$(
  LC_ALL=C.UTF-8 awk -F'|' '
    /^\|/ {
      if ($0 ~ /^[|][[:space:]]*:?-/) next
      claim = $2; ev = $3; st = $4
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", claim)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", ev)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", st)
      if (claim == "说法" || claim == "") next
      print st "\t" ev "\t" claim
    }
  ' "$doc"
)"

if [[ -z "$rows" ]]; then
  printf 'no claim rows parsed from %s\n' "$doc" >&2
  exit 1
fi

verified=0
partial=0
missing=0
problems=0

problem() {
  printf '  FAIL  %s\n' "$1"
  problems=$((problems + 1))
}

while IFS=$'\t' read -r status evidence claim; do
  case "$status" in
    "✅")
      verified=$((verified + 1))
      case "$evidence" in
        test:*|cmd:*|file:*) ;;
        *) problem "✅ without executable evidence: $claim" ;;
      esac
      ;;
    "⚠️")
      partial=$((partial + 1))
      [[ -z "$evidence" || "$evidence" == "无" ]] \
        && problem "⚠️ must say what is missing: $claim"
      ;;
    "❌")
      missing=$((missing + 1))
      [[ "$evidence" != "无" ]] \
        && problem "❌ must use 无, not '$evidence': $claim"
      ;;
    *)
      problem "unknown status '$status': $claim"
      continue
      ;;
  esac

  if [[ $run -eq 1 && "$status" == "✅" ]]; then
    case "$evidence" in
      test:*)
        name="${evidence#test: }"
        test_output="$(cargo test --offline "$name" -- --quiet 2>&1)"
        if [[ $? -ne 0 ]]; then
          problem "test does not pass: $name"
        elif ! grep -qE '[1-9][0-9]* passed' <<<"$test_output"; then
          problem "no test matched (silent pass risk): $name"
        fi
        ;;
      cmd:*)
        command="${evidence#cmd: }"
        if ! bash -c "$command" >/dev/null 2>&1; then
          problem "command failed: $command"
        fi
        ;;
      file:*)
        path="${evidence#file: }"
        [[ -f "$path" ]] || problem "missing file: $path"
        ;;
    esac
  fi
done <<< "$rows"

readme_claims="$(
  sed -n '/^## Status/,/^## Commands/p' README.md 2>/dev/null | grep -c '^- ' || true
)"
mapped=$((verified + partial + missing))

printf 'claims checked    : %d\n' "$mapped"
printf '  verified  ✅    : %d\n' "$verified"
printf '  partial   ⚠️    : %d\n' "$partial"
printf '  no evidence ❌  : %d\n' "$missing"
printf 'README claims     : %s (not yet mapped one by one)\n' "${readme_claims:-unknown}"
printf 'mode              : %s\n' "$([[ $run -eq 1 ]] && echo 'structure + execution' || echo 'structure only')"

if [[ $problems -gt 0 ]]; then
  printf '\n%d problem(s)\n' "$problems"
  exit 1
fi

printf '\nclaims table is well-formed'
[[ $run -eq 1 ]] && printf ' and every ✅ evidence executed'
printf '\n'
