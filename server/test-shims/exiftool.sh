#!/usr/bin/env bash

set -euo pipefail

in_path=${*: -1}

args=("$@")
unset "args((${#args[@]}-1))"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"${SCRIPT_DIR}/common.sh" "$in_path" exiftool "${args[@]}"
