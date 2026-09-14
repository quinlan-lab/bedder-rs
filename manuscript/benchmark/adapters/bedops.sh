#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4

if [[ -z "${BENCH_PREP_DIR:-}" ]]; then
  echo "BENCH_PREP_DIR is required for BEDOPS prepared inputs" >&2
  exit 2
fi

bedops --element-of 1 \
  "$BENCH_PREP_DIR/query.bedops.bed" \
  "$BENCH_PREP_DIR/target.bedops.bed" \
  > "$output"
