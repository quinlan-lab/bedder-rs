# Manuscript benchmarks

The [benchmark harness](benchmark/README.md) compares bedder, BEDTools, bedtk,
BEDOPS, AIList, and COITrees on interval membership: emit each BED4 query record
once if it overlaps any target. Outputs must match the truth set before timings
are accepted. Run the commands below from the repository root.

## Setup and smolvm

Docker builds the tool image using revisions in [config.env](benchmark/config.env);
smolvm runs that image in a VM configured by [Smolfile](benchmark/Smolfile).
The VM uses one vCPU, 4096 MiB RAM, disabled networking, and single-thread
environment settings to keep runs comparable. The repository is mounted
read-only at `/workspace`; inputs and results are mounted read-write at `/results`.

Install Docker and smolvm, then build the image once:

```bash
export SMOLVM_BIN=/path/to/smolvm  # default: /home/brentp/.smolvm/smolvm
export BEDDER_CMP_ROOT=/media/brentp/elements/bedder-cmp
manuscript/benchmark/scripts/build-image.sh
```

If you change `BEDDER_CMP_ROOT`, also update the image archive path in
`benchmark/Smolfile`. By default, runs use the bedder revision baked into the
image. To test a local build, set `BENCH_BEDDER_BIN=/workspace/target/release/bedder`;
see the [Python 3.12 build instructions](benchmark/README.md#quick-start) for VM
compatibility.

## Smoke test

Small bundled fixtures cover duplicate coordinates, adjacency, containment,
partial overlaps, multiple hits, and nonmatches. Run this first to check all
six adapters; its near-zero timings are unsuitable for performance claims.

```bash
manuscript/benchmark/scripts/run-smoke-smolvm.sh
```

## Native VCF versus BEDTools

This is the comparison described in manuscript Section 3.2. Bedder and
BEDTools read the original compressed HG002 VCF directly and intersect it with
the compressed UCSC simple-repeat BED track. Only these two tools are included
because they support VCF input. The output is the set of overlapping VCF
records (one record per overlap); the runner validates that their unique query
intervals equal the independent truth set.

Prepare the VCF and BED inputs as described below, then run both manuscript
workloads in SmolVM:

```bash
BENCH_BEDDER_BIN=/workspace/target/release/bedder \
  manuscript/benchmark/scripts/run-vcf-smolvm.sh chr19
BENCH_BEDDER_BIN=/workspace/target/release/bedder \
  manuscript/benchmark/scripts/run-vcf-smolvm.sh full
```

Use `BENCH_RUNS` and `BENCH_WARMUPS` to change replication and
`BENCH_RESULT_DIR` to choose a new output directory. The runner alternates
bedder and BEDTools, validates every output, and records VCF timings and peak
RSS. BEDTools uses `-sorted`; decompression, parsing, and VCF output writing
are timed for both tools. It does not run the optional Python `repeat_sequence`
annotation.

The commands in [simple-repeats.sh](simple-repeats.sh) are the direct,
one-shot form of this native VCF workflow, including the optional VCF/BCF
annotation example.

## BED4 cross-tool panel: chromosome 19

Compare 89,648 HG002 chromosome 19 variants against the complete UCSC hg38
simpleRepeat track (967,506 intervals). The expected output has 6,882 records.

Prepare inputs for both the native VCF benchmark and this BED4 panel. Supply the HG002 VCF
and adjacent `.tbi`, `simple-repeats.bed.gz`, and `hg38.fai` in the repository
root; [simple-repeats.sh](simple-repeats.sh) contains the download commands.
Alternatively, set `HG002_VCF`, `SIMPLE_REPEATS_BED`, and `HG38_FAI` to their paths.
Preparation requires host `bcftools` and Python 3, projects inputs to shared BED4
records, and generates truth with an independent sweep-line oracle.

```bash
manuscript/benchmark/scripts/prepare-simple-repeats.sh
BENCH_RUNS=10 BENCH_RESULT_DIR=chr19-rerun-01 \
  manuscript/benchmark/scripts/run-smolvm.sh simple-repeats-chr19
```

## BED4 cross-tool panel: full genome

Use the same prepared repeat track with all 4,048,427 HG002 variant records;
175,403 records should match.

```bash
BENCH_RUNS=10 BENCH_RESULT_DIR=full-rerun-01 \
  manuscript/benchmark/scripts/run-smolvm.sh simple-repeats-full
```

These two cases project the VCF and repeat track to shared BED4 records so all
six tools can be compared on identical interval-membership work. They are
separate from the native VCF-versus-BEDTools benchmark above.

## Results and reruns

Results go to `$BEDDER_CMP_ROOT/results/$BENCH_RESULT_DIR/` (the case name is
the default directory). Use a fresh name for each rerun to preserve prior results.
Inspect `summary.tsv` for wall time and peak RSS, `raw/measurements.jsonl` for
individual runs, and `validation.json`, `inputs.tsv`, and `versions.tsv` for
correctness and provenance.

The runner defaults to one warm-up and three measured runs; the commands above
use ten measurements for the manuscript cases. Tool order rotates between runs.
Set `BENCH_TOOLS=bedder,bedtools` to select a subset and `BENCH_WARMUPS` to change
warm-ups. Preparation and validation are outside timing; BEDOPS sorting costs
are recorded separately in `preparation.json`. Keep the host otherwise idle.

For comparisons between saved bedder binaries, including optimization and CPU
build experiments, see [EXPERIMENTS.md](benchmark/EXPERIMENTS.md),
[CACHE-A-EXPERIMENT.md](benchmark/CACHE-A-EXPERIMENT.md), and
[REMOVE-LAST-EXPERIMENT.md](benchmark/REMOVE-LAST-EXPERIMENT.md).
