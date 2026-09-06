#!/usr/bin/env bash
set -euo pipefail

# Wrapper around exiftool and ffprobe that writes their output to files,
# so the commands can be replayed later without running the real tool on real files.

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <in_path> <cmd> [args...]" >&2
    exit 1
fi

if [ ! -z ${MYRTI_TEST_REAL_DATA+x} ]; then
    library_dir='../test-data/library'
else
    library_dir='../test-data/fake-tree'
fi
recorded_dir='../test-data/metadata-output'
mkdir -p $recorded_dir

in_path="$1";     shift
cmd="$1";         shift
cmd_args=("$@")

args=("$@")
for i in "${!args[@]}"; do
    if [[ "${args[$i]}" == "$in_path" ]]; then
        args[i]="<INPUT>"
    fi
done

args_hash=$(printf '%s\0' "$cmd" "${args[@]}" | sha256sum | cut -d' ' -f1 | head -c 12)

stripped="${in_path#"$(realpath "$library_dir")/"}"
recording="${recorded_dir}/${stripped}.${cmd}-${args_hash}"

if [ ! -z ${MYRTI_TEST_READ_DATA+x} ]; then
    mkdir -p "$(dirname "$recording")"
    "$cmd" "${cmd_args[@]}" > "${recording}.stdout" 2> "${recording}.stderr"
fi

cat "${recording}.stderr" >&2
cat "${recording}.stdout"
