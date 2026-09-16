# Reproducing the segdup manuscript results

[Back to the quick-start example](README.md).

From the repository root, with Python 3.11+ and a working release build of bedder:

```bash
python3 manuscript/segdup-identity/run.py \
  --bedder target/release/bedder \
  --output manuscript/segdup-identity/results

python3 -m unittest discover -s manuscript/segdup-identity -v
```

Use a new output directory on each run. No network, reference FASTA, indices,
or third-party Python packages are required. The bundled inputs occupy about
4.3 MiB. Input SHA-256 checks run before annotation. Every original VCF record
and all four annotation fields must match an independent validator using the
original UCSC source rows; it does not import the callback. The validator also
checks the BED projection against those source rows.

Outputs are `annotated.vcf`, `best-matches.tsv`, `summary.json`, and
`provenance.json`. Provenance records commands, input/script/output hashes,
the actual bedder binary hash and version, Git revision, both Python versions,
and platform. The binary hash identifies the tested executable; the checkout's
Git revision alone does not prove how that executable was built.

Underlying command:

```bash
target/release/bedder intersect \
  -a manuscript/segdup-identity/data/variants.vcf \
  -b manuscript/segdup-identity/data/segdups.bed \
  -g manuscript/segdup-identity/data/genome.tsv \
  --a-piece whole-wide --b-piece whole-wide -r 0 -R 0 \
  --python manuscript/segdup-identity/segdup_best_match.py \
  -c py:sd_best -c py:sd_best_interchrom \
  -c py:sd_count -c py:sd_count_interchrom \
  -o annotated.vcf
```

The `whole-wide` options present all overlapping SDs to each callback together,
producing one annotated record per query. Both zero overlap requirements are
needed to retain queries with no SD hit. Missing best matches are `.` and counts
are zero. The frozen biological subset contains only variants with an SD hit;
the CLI fixture separately tests no-hit behavior and half-open boundaries.
Current bedder may print missing-index messages before streaming the plain VCF.

## Input fields and exact identity

The prepared SD file is BED6 plus:

| BED column | Value |
| --- | --- |
| 7 | `fracMatch=<original UCSC value>` |
| 8 | `otherChrom` |
| 9 | `otherStart` |
| 10 | `otherEnd` |
| 11 | `uid` |

Column 4 is a unique snapshot row ID (`sd_<uid>_<source-line>`). Column 5 is
zero; column 6 is the original alignment strand. In the current Python API,
`other_fields()` includes the strand followed by columns 7–11.

The label on `fracMatch` is intentional: the current numeric BED extra-field
formatter rounds to four decimal places. Keeping the value as labeled text
preserves the original precision. The callback uses `Decimal` for ranking;
the independent oracle uses `Fraction`. Ties select the lexicographically
smallest unique row ID, independent of overlap ordering. Counts represent
alignment rows, including distinct rows with the same coordinates, not genome
copy number.

`sd_best` and `sd_best_interchrom` are scalar strings with this layout:

```text
fracMatch|local_chrom:start-end|partner_chrom:start-end|strand|row_id
```

All interval coordinates are zero-based and half-open. VCF POS remains one-based.
Identity is a fraction: `0.994659` means 99.4659% identity.

## Frozen data and interpretation

The source is UCSC hg38 `genomicSuperDups`, downloaded with its SQL schema.
The schema and original selected rows are bundled, and the full downloaded
table's hash is recorded in `data/inputs.lock.json`.

Preparation selects chr19 `PASS`, biallelic A/C/G/T SNVs with the ALT present
in HG002 from the existing manuscript VCF. It retains those overlapping at
least one SD whose partner is a primary chromosome (`chr1`–`chr22`, `chrX`,
or `chrY`). Partner scaffolds and alternate haplotypes are excluded before
annotation, so they cannot be mislabeled as interchromosomal duplications.
The BED retains all eligible SD rows overlapping the selected variants.

The UCSC track contains detected SDs meeting its alignment/identity criteria;
it is not an exhaustive collection of every possible genome self-alignment.
`fracMatch` describes the **whole pairwise SD alignment**, not identity within
the query's overlap or at the individual variant base. The partner interval
does not locate the homologous variant base: that would require the actual
alignment. These annotations provide duplication context, not a genotype,
mapping-error diagnosis, or pathogenicity prediction.

See [UCSC's track description and methods](https://genome.ucsc.edu/cgi-bin/hgTrackUi?db=hg38&g=genomicSuperDups)
and [BEDtools map documentation](https://bedtools.readthedocs.io/en/latest/content/tools/map.html).

## Verified HG002 results

From 89,648 chr19 source records, 74,987 pass the SNV/genotype filters.
There are 1,381 chr19 SD rows, of which 1,326 have primary-chromosome partners.
The frozen overlap subset includes 6,205 variants and 817 SD alignment rows.

| Result | Variants |
| --- | ---: |
| At least one interchromosomal SD | 902 |
| Intrachromosomal SDs only | 5,303 |
| Different best record after restricting to interchromosomal SDs | 99 |

For example, **chr19:9722714 G>A** overlaps two SD rows:

| Selection | fracMatch | Local interval | Partner interval |
| --- | ---: | --- | --- |
| Best overall | 0.911287 | chr19:9722363-9725203 | chr19:9688857-9691392 |
| Best interchromosomal | 0.910397 | chr19:9722368-9725051 | chr5:54857391-54859769 |

All 6,205 records and their four annotations pass independent validation.
Tests cover this complete dataset, conditional selection, exact precision,
deterministic ties, invalid identity, and an actual CLI fixture with adjacent
boundaries, no hits, and identities distinguishable only beyond four decimals.
The quick-start pipeline is also tested for sorting, primary-chromosome
filtering, column mapping, and identity precision.

## Rebuild from the full sources

Download the full table and its schema, then rebuild using the existing HG002 VCF:

```bash
curl -fL --retry 3 https://hgdownload.soe.ucsc.edu/goldenPath/hg38/database/genomicSuperDups.txt.gz \
  -o /tmp/genomicSuperDups.txt.gz
curl -fL --retry 3 https://hgdownload.soe.ucsc.edu/goldenPath/hg38/database/genomicSuperDups.sql \
  -o /tmp/genomicSuperDups.sql
python3 manuscript/segdup-identity/prepare.py \
  --vcf HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz \
  --segdups /tmp/genomicSuperDups.txt.gz \
  --schema /tmp/genomicSuperDups.sql \
  --output /tmp/segdup-rebuilt
```

The script requires the exact frozen source hashes and verifies every rebuilt
input hash and selection count. A changed UCSC download is rejected; the
bundled data remain the authoritative offline reproduction inputs. The
`--freeze` option is only for deliberately creating a different snapshot.
