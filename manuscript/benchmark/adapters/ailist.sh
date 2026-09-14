#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4

tmp_output=$(mktemp)
trap 'rm -f "$tmp_output"' EXIT

# The upstream benchmark CLI emits "ordinal chrom: start end count" with -P 1.
ailist "$target" "$query" -P 1 > "$tmp_output"

python3 "$(dirname "$0")/../scripts/normalize-ailist.py" \
  "$query" "$tmp_output" "$output"
