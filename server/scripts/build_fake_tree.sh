#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pushd "$SCRIPT_DIR/../test-data/library" > /dev/null
find . -type f -exec bash -c '
		mkdir -p "../fake-tree/$(dirname "$1")" && cksum --raw --algorithm sha256 "$1" > "../fake-tree/$1"
		' bash {} \;
popd > /dev/null
