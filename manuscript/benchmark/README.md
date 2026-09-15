# bedder comparative benchmark

This directory contains two correctness-first benchmark workflows:

* `run-vcf-smolvm.sh` compares bedder and BEDTools on the original VCF input,
  matching the manuscript's native VCF comparison.
* `run-smolvm.sh` compares six tools on a shared BED4 projection, allowing tools
  without VCF support to participate in the same interval-membership panel.

Every tool adapter implements the same interface:

```text
adapter QUERY.bed TARGET.bed GENOME.fai OUTPUT.bed
```

The output must contain each four-column query record once when it overlaps at
least one target interval. The Python runner canonicalizes outputs, rejects a
measurement if correctness differs from the truth file, and writes raw JSONL
plus a summary TSV.

## Quick start

Build the pinned OCI image and save it on the external drive:

```bash
manuscript/benchmark/scripts/build-image.sh
```

Run the smoke evaluation inside SmolVM:

```bash
manuscript/benchmark/scripts/run-smoke-smolvm.sh
```

Results are written under:

```text
/media/brentp/elements/bedder-cmp/results/smoke/
```

Run a subset of tools or change replication without rebuilding the image:

```bash
BENCH_TOOLS=bedder,bedtools,bedtk \
BENCH_RUNS=5 \
manuscript/benchmark/scripts/run-smoke-smolvm.sh
```

To evaluate the current checkout instead of the bedder version pinned in the
image, build a VM-compatible binary on the host and select it through the
repository's read-only VM mount. For example, the development host currently
links against Python 3.13 while the Ubuntu 24.04 image provides Python 3.12,
so select Python 3.12 at build time:

```bash
mkdir -p target/python312-lib
ln -sf /usr/lib/x86_64-linux-gnu/libpython3.12.so.1.0 \
  target/python312-lib/libpython3.12.so
LIBRARY_PATH="$PWD/target/python312-lib" \
PYO3_PYTHON=/usr/bin/python3.12 \
cargo build --release
BENCH_BEDDER_BIN=/workspace/target/release/bedder \
manuscript/benchmark/scripts/run-smoke-smolvm.sh
```

Run any case from `cases.tsv` and optionally keep it in a named result
directory:

```bash
BENCH_RUNS=10 BENCH_RESULT_DIR=hg002-run-01 \
manuscript/benchmark/scripts/run-smolvm.sh simple-repeats-full
```

Prepare and run the two cases derived from `manuscript/simple-repeats.sh`:

```bash
manuscript/benchmark/scripts/prepare-simple-repeats.sh
manuscript/benchmark/scripts/run-smolvm.sh simple-repeats-chr19
BENCH_RUNS=10 manuscript/benchmark/scripts/run-smolvm.sh simple-repeats-full
```

Preparation copies the source VCF, index, repeat track, and genome file to the
external drive; creates the chromosome 19 VCF subset and shared BED4
projections; and generates membership truth with the independent sweep-line
oracle. The six-tool timing therefore measures interval membership on identical
BED4 records. The native VCF timing is separate and includes VCF input parsing
and VCF output for bedder and BEDTools only.

## Native VCF comparison

Run the manuscript's two VCF workloads after preparation:

```bash
BENCH_RUNS=10 manuscript/benchmark/scripts/run-vcf-smolvm.sh chr19
BENCH_RUNS=10 manuscript/benchmark/scripts/run-vcf-smolvm.sh full
```

The runner uses the original `HG002_*.vcf.gz` (or its prepared `chr19`
subset) and `simple-repeats.source.bed.gz`. It checks that bedder and BEDTools
produce the same overlapping VCF records and that their unique query intervals
match the independent BED truth set. Native VCF output can contain multiple
records for a variant overlapping multiple repeat intervals; this is distinct
from the BED4 panel's one-record-per-query membership contract. Results are
written under `$BEDDER_CMP_ROOT/results/$BENCH_RESULT_DIR/`.

BEDTools runs with `-sorted -header -g hg38.fai`; both tools write uncompressed
VCF files during timing. Input decompression, parsing, intersection, and output
writing are included; preparation and validation are excluded.

### Verified full-genome run

The full-genome run used one warm-up and ten measured runs.

| Tool | Median wall seconds (IQR) | Median peak RSS (KiB) | Correct |
| --- | ---: | ---: | --- |
| bedder | 6.715 (6.6625–6.8325) | 43,634 | yes |
| BEDTools `-sorted` | 4.685 (4.615–4.785) | 41,094 | yes |

Both tools emitted 258,629 records covering the 175,403 expected unique query
intervals. Their validation VCFs were byte-identical. Results are under
`/media/brentp/elements/bedder-cmp/results/vcf-full-main-20260915/`.
The tested binary SHA-256 is
`c39f956015ddfa4f89cb0e955a51a64f2d9689ae02fd87d885e373ae1b9b54e5`.

## Adding an evaluation

1. Add sorted BED4 query and target files, a two-column FAI-compatible genome
   file, and an expected BED4 result.
2. Add one row to `cases.tsv`. Paths are relative to this directory.
3. Run `python3 bench.py --list-cases` to confirm discovery.
4. Run the case with `scripts/run-smolvm.sh CASE_ID`.

Large inputs should live under `/media/brentp/elements/bedder-cmp/data`, which
is mounted as `/results` inside the VM. Repository fixtures should remain
small.

## Adding a tool

1. Install a pinned build in `docker/Dockerfile` and record the revision in
   `config.env` and `tools.tsv`.
2. Add `adapters/TOOL.sh` with the standard four-input/one-output interface.
3. Add the tool name to `DEFAULT_TOOLS` in `bench.py`.
4. Add a version command to `scripts/record-versions.sh`.

Preparation required by a tool belongs in `bench.py::prepare_case` so that it
can be measured or excluded consistently. Output conversion needed only for
validation belongs in the adapter or canonicalizer and must be documented.

## Benchmark policy

- Correctness is a gate, not a metric to trade for speed.
- The smoke suite includes only steady-state query timing.
- Sorting and indexing are recorded separately in later modules.
- Count-only and materialized-output tasks must not share a performance panel.
- Tool order rotates between runs.
- The primary VM has one vCPU and 4096 MiB RAM.
