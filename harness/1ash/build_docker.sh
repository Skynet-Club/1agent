#!/bin/sh
# Build the local 1ASH subject image. The image is for sandbox mechanics, not
# a published artifact; it uses the already-built debug binary.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$root"
if [ ! -x target/debug/first-principles-agent ]; then
  echo "build target/debug/first-principles-agent first" >&2
  exit 2
fi
docker build -f harness/1ash/Dockerfile -t "${ONEASH_DOCKER_IMAGE:-1ash-1agent:local}" .
docker image inspect "${ONEASH_DOCKER_IMAGE:-1ash-1agent:local}" --format '{{.Id}}'
