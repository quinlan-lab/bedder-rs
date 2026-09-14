#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4
bedder_bin=${BEDDER_BIN:-bedder}

"$bedder_bin" intersect \
  -a "$query" \
  -b "$target" \
  -g "$genome" \
  -p whole-wide \
  -P none \
  -o "$output"
