#!/usr/bin/env bash
set -euo pipefail

query=$1
target=$2
genome=$3
output=$4

# bedtk flt reports records from its second input that overlap its first input.
bedtk flt "$target" "$query" > "$output"
