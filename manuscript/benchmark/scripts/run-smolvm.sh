#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
benchmark_dir=$(cd "$script_dir/.." && pwd)
repo_root=$(cd "$benchmark_dir/../.." && pwd)
runtime_root=${BEDDER_CMP_ROOT:-/media/brentp/elements/bedder-cmp}
smolvm_bin=${SMOLVM_BIN:-/home/brentp/.smolvm/smolvm}
case_id=${1:-${BENCH_CASE:-smoke}}
tools=${BENCH_TOOLS:-bedder,bedtools,bedtk,bedops,ailist,coitrees}
runs=${BENCH_RUNS:-3}
warmups=${BENCH_WARMUPS:-1}
result_dir=${BENCH_RESULT_DIR:-$case_id}
bedder_bin=${BENCH_BEDDER_BIN:-bedder}

if [[ ! -x "$smolvm_bin" ]]; then
  echo "smolvm not found at $smolvm_bin" >&2
  exit 2
fi
if [[ ! -f "$runtime_root/bedder-benchmark-image.tar" ]]; then
  echo "benchmark image is missing; run scripts/build-image.sh first" >&2
  exit 2
fi

mkdir -p "$runtime_root/results/$result_dir"

"$smolvm_bin" machine run \
  --smolfile "$benchmark_dir/Smolfile" \
  --volume "$repo_root:/workspace:ro" \
  --volume "$runtime_root:/results:rw" \
  -- \
  env BEDDER_BIN="$bedder_bin" python3 bench.py \
    --case "$case_id" \
    --tools "$tools" \
    --runs "$runs" \
    --warmups "$warmups" \
    --output-root "/results/results/$result_dir"
