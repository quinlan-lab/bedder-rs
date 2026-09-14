#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4

bedtools intersect \
  -sorted \
  -g "$genome" \
  -u \
  -a "$query" \
  -b "$target" \
  > "$output"
