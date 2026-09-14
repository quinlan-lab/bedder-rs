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
writing are included; preparation and validation are excluded. Earlier native
VCF runs without `-sorted` and with Python-buffered BEDTools output are not
comparable to this protocol.

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

## Verified smoke run

The setup was exercised successfully in SmolVM on 2026-09-08 with bedder
commit `9e66b8f` plus the reviewed A-coordinate cache. All six tools
passed the BED4 membership truth set, including duplicate coordinates,
adjacency, both containment directions, a partial overlap, a multi-hit query,
and chromosome-specific negative cases. Three measured iterations per tool
were written to
`/media/brentp/elements/bedder-cmp/results/smoke-cache-a-20260908/`.

This fixture is an integration and correctness test; its 0–20-ms timings are
too short for reliable performance interpretation. Add full-size
cases to `cases.tsv` before drawing performance conclusions.

## Verified simple-repeat runs

Both cases derived from `manuscript/simple-repeats.sh` were run successfully
in the one-vCPU, 4096-MiB SmolVM on 2026-09-08 using local bedder commit
`9e66b8f` plus the reviewed A-coordinate cache and simplebed v0.1.8 fix
documented in [CACHE-A-EXPERIMENT.md](CACHE-A-EXPERIMENT.md) and
[tabix-bug.md](../../tabix-bug.md). Each tool had
one warm-up and ten measured runs, with rotating tool order:

| Case | Query records | Target records | Matching records | Tools passing | Timed runs per tool |
| --- | ---: | ---: | ---: | ---: | ---: |
| `simple-repeats-chr19` | 89,648 | 967,506 | 6,882 | 6/6 | 10 |
| `simple-repeats-full` | 4,048,427 | 967,506 | 175,403 | 6/6 | 10 |

| Case | Tool | Median wall seconds (IQR) | Median peak RSS (KiB) | Correct |
| --- | --- | ---: | ---: | --- |
| chr19 | bedder | 0.140 (0.1325–0.140) | 13,838 | yes |
| chr19 | BEDTools | 0.110 (0.110–0.110) | 6,900 | yes |
| chr19 | bedtk | 0.100 (0.100–0.100) | 16,934 | yes |
| chr19 | BEDOPS | 0.100 (0.100–0.110) | 4,252 | yes |
| chr19 | AIList | 0.170 (0.160–0.170) | 18,596 | yes |
| chr19 | COITrees | 0.110 (0.110–0.110) | 16,140 | yes |
| full | bedder | 1.060 (1.060–1.160) | 13,726 | yes |
| full | BEDTools | 0.9000 (0.8850–0.9175) | 38,536 | yes |
| full | bedtk | 0.6850 (0.6800–0.6975) | 16,988 | yes |
| full | BEDOPS | 1.2100 (1.2100–1.2175) | 4,252 | yes |
| full | AIList | 2.1500 (2.1350–2.3700) | 18,600 | yes |
| full | COITrees | 1.3450 (1.3225–1.3500) | 16,120 | yes |

The independent oracle's full-genome output was also compared byte-for-byte
with `bedtools intersect -sorted -u`. Results, raw timings, validation reports,
input hashes, and binary hashes are under
`/media/brentp/elements/bedder-cmp/results/simple-repeats-chr19-simplebed-v018-20260908/`
and
`/media/brentp/elements/bedder-cmp/results/simple-repeats-full-simplebed-v018-20260908/`.
The binary was built with `cargo build --release --locked`, default features
(including mimalloc), and `PYO3_PYTHON=/usr/bin/python3.12`. It was copied to
`target/benchmark/simplebed-v018-20260908/bedder` and selected through
`BENCH_BEDDER_BIN`. The exact source patch, including the additional regression
tests, is archived as
`/media/brentp/elements/bedder-cmp/experiments/cache-a-20260908/cached-reviewed.patch`
and the published simplebed v0.1.8 commit;
the baseline commit alone does not include these local changes.
The recorded bedder SHA-256 is
`f9469967b1d8d4e6462579a3467623d9f732936d58a5bc75827e977b69d81b81`.

These cases now use the planned ten-run replication. The supplement still
needs the other workload modules, and a final frozen source revision. BEDOPS
input sorting is performed before timing and recorded in `preparation.json`;
one-shot workflow comparisons should add that cost explicitly. Fractional
milliseconds in medians/quartiles come from interpolation of GNU time's
10-ms-resolution observations, not finer-resolution individual timings.
