# HG002 SNVs that change uninterrupted tandem-repeat runs

This example uses bedder's Python interface to ask whether an HG002 SNV
lengthens or shortens the longest pure run within an overlapping short tandem
repeat. BEDtools can identify the overlap; this calculation additionally needs
the REF/ALT alleles, repeat motif, repeat sequence, and sequence-specific logic.
An equivalent BEDtools workflow would require custom postprocessing.

The Python example is [repeat_run_effect.py](repeat_run_effect.py);
`bedder_repeat_run` is the callback invoked by `-c py:repeat_run`.

## Reproduce offline

From the repository root, with a working release build of bedder and Python
3.11 or later:

```bash
python3 manuscript/repeat-interruptions/run.py \
  --bedder target/release/bedder \
  --output manuscript/repeat-interruptions/results

python3 -m unittest discover -s manuscript/repeat-interruptions -v
```

The runner requires a new output directory, verifies SHA-256 hashes before
execution, and rejects any result that disagrees with an independent oracle.
No downloads, indexes, third-party Python packages, or environment-specific
absolute input paths are needed. The bundled inputs are approximately 580 KiB.
The Python callback uses only the standard library. A release binary must
support embedded Python; an alternative binary can be supplied with `--bedder`.
The integration test uses `target/release/bedder` and reports a skip if absent.

Outputs:

- `annotated.vcf`: original VCF records with one `repeat_run` INFO annotation
  per overlapping repeat. A variant overlapping two repeats appears twice.
- `effects.tsv`: validated results, including reference and substituted repeat
  sequences for inspecting individual examples.
- `summary.json`: counts at the variant-repeat-pair level.
- `provenance.json`: exact command, input/script/output hashes, binary hash and
  version, embedded and driver Python versions, platform, current Git revision,
  tracked source changes, and Cargo.lock hash when available. The binary hash
  identifies the executable actually used; a Git revision alone does not prove
  a binary was built from that revision.

The underlying command is:

```bash
target/release/bedder intersect \
  -a manuscript/repeat-interruptions/data/variants.vcf \
  -b manuscript/repeat-interruptions/data/repeats.bed \
  -g manuscript/repeat-interruptions/data/genome.tsv \
  --a-piece whole --b-piece whole \
  --python manuscript/repeat-interruptions/repeat_run_effect.py \
  -c py:repeat_run -o annotated.vcf
```

The current binary may print index-lookup messages for the plain VCF before
falling back to streaming. No index is required for this example.

## Frozen dataset and selection rules

`data/inputs.lock.json` records hashes of both original files and bundled
derived inputs. The data use the manuscript's existing HG002
`HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz` and UCSC hg38 simpleRepeat
BED track. Chromosome 19 matches an existing manuscript benchmark scope.

Selection is deterministic and independent of the annotation result:

1. Select chromosome 19 records with FILTER exactly `PASS`, one A/C/G/T REF
   base, one A/C/G/T ALT base, and ALT allele 1 present in the HG002 genotype.
2. Select chr19 simpleRepeat intervals with A/C/G/T motifs of length 2–6.
   Exclude motifs expressible as repetitions of a shorter motif and repeat
   sequences containing ambiguous reference bases.
3. Retain selected SNVs overlapping at least one selected repeat and retain
   the repeat intervals they overlap. Validate REF against the chr19 reference.

Of 89,648 chr19 source records, 74,987 pass the SNV/genotype filters.
There are 6,298 eligible repeat intervals; the bundled subset contains
551 SNVs and 420 overlapping repeat intervals. Original VCF records and
headers are preserved. Repeat IDs encode the line number in the frozen source
BED; they are identifiers for this dataset, not stable UCSC accessions.

The BED contains chromosome, zero-based start, exclusive end, repeat ID,
forward-strand motif, and uppercase reference sequence. Its final two columns
are custom fields consumed by the callback. Reference sequences are included
so the callback does not require a FASTA reader or repeated reference fetches.

## Definition and limits

For every variant-repeat pair, substitute that ALT for the reference base.
Compute the maximum number of consecutive, complete motif copies in each
sequence, allowing every rotation of the forward-strand motif and every start
position within the annotated interval. Subtract the reference count from the
alternate count. Partial copies do not count; reverse complements are not
added to the motif set. The search is bounded by the supplied repeat interval.

INFO syntax is:

```text
repeat_run=repeat_id|motif|reference_copies|alternate_copies|delta_copies
```

The callback rejects reports containing multiple repeats so this single-value
INFO field remains valid. Use the explicit `--a-piece whole --b-piece whole`
options shown above. It also rejects invalid motifs, ambiguous sequence,
non-SNV alleles, REF mismatches, and inconsistent sequence/interval lengths.

This is a **single-SNV substitution in the reference background**, not a phased
HG002 haplotype reconstruction. Other HG002 variants in the same repeat are
not applied. A positive delta means a longer uninterrupted run, not an
insertion, a measured expansion, or a prediction of disease. Zero delta does
not prove the repeat is unaffected: another run can remain the longest.
The method does not infer changes outside the annotated interval, motif
realignment across indels, or pathogenicity. It excludes multiallelic variants,
indels, homopolymers, and long/ambiguous motifs.

Repeat purity has biological relevance: CAA interruptions in CAG tracts can
alter repeat stability without changing the encoded glutamine sequence
([experimental study](https://www.nature.com/articles/s41588-025-02172-8)).
That motivates the calculation; this chr19 demonstration does not analyze
HTT or claim that its HG002 examples have that disease mechanism.

## Verified results

All 561 variant-repeat annotations, their multiplicities, original VCF fields,
and the new INFO header pass validation. Counts are pairs, not independent
variants:

| Effect on longest pure run | Pairs |
| --- | ---: |
| Increased | 48 |
| Decreased | 225 |
| Unchanged | 288 |

Examples from the actual HG002 input (VCF positions are one-based):

| Variant | Motif | Reference copies | Alternate copies | Delta |
| --- | --- | ---: | ---: | ---: |
| chr19:16259432 T>C | AC | 13 | 26 | +13 |
| chr19:2958865 C>G | TG | 14 | 23 | +9 |
| chr19:23692016 T>C | TA | 19 | 9 | -10 |

The independent validator scans candidate start positions and repeated blocks
without importing the regex-based annotation function. Tests include creating
and removing an interruption, boundaries, motif rotation, invalid inputs,
exhaustive short sequences, a real HG002 overlapping-match regression, and the
complete frozen dataset through the actual bedder CLI.
An additional 500 seeded SNV cases compare both forward and reverse
substitutions with the independent oracle; a test checks INFO cardinality.

## Rebuild the frozen inputs from the original files

Original sources:

- [HG002 release VCF](https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/release/AshkenazimTrio/HG002_NA24385_son/mosaic_v1.00/GRCh38/SNV/HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz)
- [UCSC simpleRepeat source table](https://hgdownload.soe.ucsc.edu/goldenPath/hg38/database/simpleRepeat.txt.gz),
  transformed in the existing [manuscript preparation script](../simple-repeats.sh).
- [UCSC chr19 FASTA](https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chr19.fa.gz).
  Its published MD5, verified during preparation, is
  `5668c1a7edca32cc5d68bbeb26061d81`; SHA-256 is in the input lock.

With the original VCF and repeat BED already in the repository root:

```bash
curl -fL --retry 3 \
  https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chr19.fa.gz \
  -o /tmp/chr19.fa.gz

python3 manuscript/repeat-interruptions/prepare.py \
  --vcf HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz \
  --repeats simple-repeats.bed.gz \
  --reference /tmp/chr19.fa.gz \
  --output /tmp/bedder-repeat-rebuilt
```

The preparation script requires exact original-file hashes and verifies that
the rebuilt subset has the same content hashes and selection counts. A fresh
download of a mutable UCSC track, or recompression of a source gzip, may differ
and is deliberately rejected. The bundled frozen subset is the authoritative
offline reproduction input and avoids dependence on those mutable sources.
