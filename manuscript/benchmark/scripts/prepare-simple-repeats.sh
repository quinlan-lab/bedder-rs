#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
benchmark_dir=$(cd "$script_dir/.." && pwd)
repo_root=$(cd "$benchmark_dir/../.." && pwd)
data_root=${BEDDER_CMP_ROOT:-/media/brentp/elements/bedder-cmp}/data/simple-repeats
vcf_name=HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz
vcf_source=${HG002_VCF:-$repo_root/$vcf_name}
repeat_source=${SIMPLE_REPEATS_BED:-$repo_root/simple-repeats.bed.gz}
genome_source=${HG38_FAI:-$repo_root/hg38.fai}

for required in "$vcf_source" "$vcf_source.tbi" "$repeat_source" "$genome_source"; do
  if [[ ! -f "$required" ]]; then
    echo "required simple-repeats input is missing: $required" >&2
    exit 2
  fi
done

mkdir -p "$data_root"
cp --reflink=auto --preserve=timestamps "$vcf_source" "$data_root/$vcf_name"
cp --reflink=auto --preserve=timestamps "$vcf_source.tbi" "$data_root/$vcf_name.tbi"
cp --reflink=auto --preserve=timestamps "$repeat_source" "$data_root/simple-repeats.source.bed.gz"
cp --reflink=auto --preserve=timestamps "$genome_source" "$data_root/hg38.fai"

bcftools query -f '%CHROM\t%POS0\t%END\t%ID\n' "$vcf_source" \
  | awk 'BEGIN { OFS="\t" } { name = ($4 == "." ? "variant" : $4); print $1, $2, $3, name "-" NR }' \
  > "$data_root/hg002.bed"

gzip -dc "$repeat_source" \
  | awk 'BEGIN { OFS="\t" } { print $1, $2, $3, $4 "-" NR }' \
  > "$data_root/simple-repeats.bed"

awk -F '\t' '$1 == "chr19"' "$data_root/hg002.bed" > "$data_root/hg002.chr19.bed"

python3 "$script_dir/oracle-membership.py" \
  --query "$data_root/hg002.bed" \
  --target "$data_root/simple-repeats.bed" \
  --genome "$data_root/hg38.fai" \
  --output "$data_root/expected.full.bed"

awk -F '\t' '$1 == "chr19"' "$data_root/expected.full.bed" \
  > "$data_root/expected.chr19.bed"

{
  printf 'role\tbytes\tsha256\tpath\n'
  for path in \
    "$data_root/$vcf_name" \
    "$data_root/$vcf_name.tbi" \
    "$data_root/simple-repeats.source.bed.gz" \
    "$data_root/hg38.fai" \
    "$data_root/hg002.bed" \
    "$data_root/hg002.chr19.bed" \
    "$data_root/simple-repeats.bed" \
    "$data_root/expected.full.bed" \
    "$data_root/expected.chr19.bed"; do
    bytes=$(stat --format=%s "$path")
    sha=$(sha256sum "$path" | awk '{ print $1 }')
    printf '%s\t%s\t%s\t%s\n' "$(basename "$path")" "$bytes" "$sha" "$path"
  done
} > "$data_root/source-files.tsv"

printf 'prepared simple-repeats cases in %s\n' "$data_root"
