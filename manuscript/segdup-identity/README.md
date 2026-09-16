# Highest-identity segmental duplication overlapping HG002 variants

[segdup_best_match.py](segdup_best_match.py) selects the overlapping UCSC
segmental-duplication alignment with the largest `fracMatch`, returning its
local and partner coordinates, alignment strand, and source row ID. A second
callback applies the same selection only to interchromosomal SDs. Two optional
callbacks count all and interchromosomal alignment rows.

This is a conditional **record selection** example. BEDtools `map -o max` can
report the maximum identity itself; the Python callback also returns the record
associated with that maximum and applies a partner-chromosome condition.

## Quick start with your existing VCF

From the repository root, with `bedder`, BEDtools, curl, gzip, and awk on your
PATH, use the existing HG002 VCF and `hg38.fai`. No variant subsetting, reference
sequence extraction, or Python preparation script is needed.

Download the UCSC table and verify the version used for this example:

```bash
example=manuscript/segdup-identity
curl -fL --retry 3 https://hgdownload.soe.ucsc.edu/goldenPath/hg38/database/genomicSuperDups.txt.gz \
  -o genomicSuperDups.txt.gz
sha256sum -c "$example/genomicSuperDups.sha256"
```

Prepare the SD intervals with one pipeline (run in Bash):

```bash
set -euo pipefail
gzip -dc genomicSuperDups.txt.gz |
  awk 'BEGIN {OFS="\t"; primary="^chr([1-9]|1[0-9]|2[0-2]|X|Y)$"}
       $2 ~ primary && $8 ~ primary {
         print $2,$3,$4,"sd_"$12"_"NR,0,$7,"fracMatch="$27,$8,$9,$10,$12
       }' |
  bedtools sort -g hg38.fai > segdups.bed
```

This removes UCSC's bin column, extracts the fields used by the callback, and
retains primary chromosomes on both sides of each alignment. Row IDs and
identity values match the frozen workflow. Then annotate the original VCF:

```bash
example=manuscript/segdup-identity
vcf=HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz
bedder intersect -a "$vcf" -b segdups.bed -g hg38.fai \
  --a-piece whole-wide --b-piece whole-wide -r 0 -R 0 \
  --python "$example/segdup_best_match.py" \
  -c py:sd_best -c py:sd_best_interchrom -o segdup-identity.vcf
```

Set `vcf` to another existing VCF, including a chromosome subset, as needed.
This command annotates every input record, so its output counts differ from
the filtered chr19 manuscript subset below. It requires no new VCF index.
For the full record selection logic, see [the Python function](segdup_best_match.py).

## Example result

For **chr19:9722714 G>A**, the best SD has `fracMatch=0.911287` and a chr19
partner. Restricting to interchromosomal SDs selects a chr5 partner with
`fracMatch=0.910397`. Identity describes the entire SD alignment; returned
coordinates are zero-based and half-open.

## Reproduce the manuscript results

The frozen chr19 subset runs offline with independent validation:

```bash
python3 manuscript/segdup-identity/run.py \
  --bedder target/release/bedder --output manuscript/segdup-identity/results
```

[Reproduction details, input fields, tests, and verified results](REPRODUCIBILITY.md).
