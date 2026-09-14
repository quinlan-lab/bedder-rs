#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
benchmark_dir=$(cd "$script_dir/.." && pwd)
repo_root=$(cd "$benchmark_dir/../.." && pwd)
runtime_root=${BEDDER_CMP_ROOT:-/media/brentp/elements/bedder-cmp}
smolvm_bin=${SMOLVM_BIN:-/home/brentp/.smolvm/smolvm}
case_id=${1:-${BENCH_VCF_CASE:-full}}
runs=${BENCH_RUNS:-10}
warmups=${BENCH_WARMUPS:-1}
result_dir=${BENCH_RESULT_DIR:-vcf-$case_id}
bedder_bin=${BENCH_BEDDER_BIN:-bedder}
data_dir=/results/data/simple-repeats

case "$case_id" in
  full)
    vcf="$data_dir/HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz"
    expected="$data_dir/expected.full.bed"
    ;;
  chr19)
    vcf="$data_dir/hg002.chr19.vcf.gz"
    expected="$data_dir/expected.chr19.bed"
    ;;
  *)
    echo "unknown VCF case: $case_id (choose full or chr19)" >&2
    exit 2
    ;;
esac

if [[ ! -x "$smolvm_bin" ]]; then
  echo "smolvm not found at $smolvm_bin" >&2
  exit 2
fi
if [[ ! -f "$runtime_root/bedder-benchmark-image.tar" ]]; then
  echo "benchmark image is missing; run scripts/build-image.sh first" >&2
  exit 2
fi

"$smolvm_bin" machine run \
  --smolfile "$benchmark_dir/Smolfile" \
  --volume "$repo_root:/workspace:ro" \
  --volume "$runtime_root:/results:rw" \
  -- \
  python3 scripts/vcf-bench.py \
    --vcf "$vcf" \
    --target "$data_dir/simple-repeats.source.bed.gz" \
    --genome "$data_dir/hg38.fai" \
    --expected "$expected" \
    --bedder "$bedder_bin" \
    --runs "$runs" \
    --warmups "$warmups" \
    --output-root "/results/results/$result_dir"
