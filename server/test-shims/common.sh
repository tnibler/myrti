#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <in_path> <cmd> [args...]" >&2
    exit 1
fi

library_dir='/test-library'
fake_dir='./test-metadata'
mkdir -p $fake_dir

in_path="$1";     shift
cmd="$1";         shift
cmd_args=("$@")

args_hash=$(printf '%s\0' "$cmd" "${cmd_args[@]}" | sha256sum | cut -d' ' -f1 | head -c 12)

stripped="${in_path#"${library_dir}/"}"
fake_path="${fake_dir}/${stripped}.${cmd}-${args_hash}"

if [ ! -f "${fake_path}.stdout" ]; then
    mkdir -p "$(dirname "$fake_path")"
    "$cmd" "${cmd_args[@]}" > "${fake_path}.stdout" 2> "${fake_path}.stderr"
fi

cat "${fake_path}.stderr" >&2
cat "${fake_path}.stdout"
