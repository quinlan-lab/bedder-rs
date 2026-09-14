#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4

# The upstream example parses BED as half-open [start, end) by converting the
# end to an inclusive `end - 1`; it indexes input 1 and appends an overlap
# count to each record from input 2. Keep TARGET first so the adapter returns
# query membership (reversing these arguments would return target membership).
# We intentionally use the upstream default querent rather than `--sorted`:
# both are valid here, but the sorted-state optimization is workload-specific.
coitrees-bed-intersect "$target" "$query" \
  | awk 'BEGIN { OFS="\t" } $NF + 0 > 0 { NF--; print }' \
  > "$output"
