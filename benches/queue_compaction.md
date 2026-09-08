# Overlap queue compaction benchmark

Baseline: commit `d805426` (includes the enqueue guard). The fixed version adds
only overlap-mode stale counting and stable deque retention. Distance/closest
selection and queue retention for those modes are unchanged.

The existing overlap scan counts expired entries. Cleanup runs when at least
4,096 entries are stale and at least half the deque is stale. It preserves start
order and lookahead, uses cached coordinates, and does not shrink capacity.
Normal inputs incur the final threshold check; trapped expired entries also
increment the counter. No extra scan is scheduled solely because the queue grows.

## Method

- AMD Ryzen 7 7840HS, Linux x86_64, rustc 1.94.0.
- `cargo bench --bench queue_compaction --no-run`, default features, release
  profile (fat LTO, one codegen unit), CLI-matching mimalloc allocator.
- Separate baseline and fixed executables, pinned to CPU 0.
- One warmup per executable per case, followed by nine runs per executable,
  alternating execution order. Reported times and RSS are medians.
- Time is measured inside the executable across iterator construction,
  enumeration, output dropping, and iterator destruction. Inputs are generated
  incrementally; there is no file parsing or output formatting in the timing.
- Peak process RSS comes from `/usr/bin/time -f %M` and includes runtime overhead.
- An initial batch overlapped with compilation and had noisy timings. The table
  uses a complete repeat after all builds and tests finished.

| Workload | A records | Baseline ms | Fixed ms | Time change | Baseline RSS KiB | Fixed RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| sparse | 3,000,000 | 493.46 | 505.66 | +2.47% | 4,436 | 4,440 |
| dense | 500,000 | 332.26 | 325.04 | -2.17% | 4,436 | 4,440 |
| all-live | 10,000 | 317.81 | 313.94 | -1.22% | 8,536 | 8,544 |
| nested | 20,000 | 153.52 | 38.70 | -74.79% | 8,540 | 6,488 |
| nested | 40,000 | 601.04 | 77.44 | -87.12% | 10,588 | 6,496 |
| distance | 1,000,000 | 189.45 | 193.63 | +2.21% | 4,444 | 4,444 |
| closest | 10,000 | 53.36 | 53.09 | -0.50% | 6,488 | 6,460 |

All cases use A intervals of length 1, spaced 10 bases apart, and one B stream.
Sparse B intervals have length 1; dense B intervals have length 1,000 (up to 100
simultaneous overlaps). All-live B intervals extend past the entire A sweep.
Nested inputs add one B interval spanning the sweep ahead of length-1 records.
Distance uses max_distance=25, n_closest=0. Closest uses unbounded n_closest=3.
There are as many regular B records as A records, plus one long B in nested cases.

The benchmark asserts expected overlap counts for all overlap-only cases and
checks the number of A records processed. Total returned counts matched between
both executables for every case, including distance/closest controls. The controls
measure existing behavior; they do not establish correctness of the known
pre-existing distance/closest ordering issues.

## Interpretation

Nested inputs run about 4x faster at 20,000 records and 7.8x faster at 40,000.
Doubling the nested input approximately doubles fixed runtime while baseline
runtime grows about fourfold. Peak fixed RSS stays near 6.3 MiB across those sizes.

Normal-case median changes range from -2.2% to +2.5%. The sparse case costs about
4 ns more per A record in this run. The unchanged distance control also moves
+2.2%, so these small differences should not be treated as precise isolated
instruction costs. The large all-live queue does not trigger cleanup and shows
no measured slowdown. These are synthetic iterator benchmarks, not an end-to-end
genomic-file or allocator-residency study.

## Reproducing

The benchmark source is `benches/queue_compaction.rs`. Copy the executable emitted
by Cargo before and after applying the overlap fix, then run each with a workload
and record count, for example:

```sh
cargo bench --bench queue_compaction --no-run
/usr/bin/time -f '%M' taskset -c 0 /path/to/baseline nested 40000
/usr/bin/time -f '%M' taskset -c 0 /path/to/fixed nested 40000
```

Use the record counts from the table and alternate baseline/fixed execution order.
The executable prints `workload,count,seconds,returned_count`.
Local raw measurements and the comparison driver are in
`/tmp/bedder-queue-bench/results.csv` and `/tmp/bedder-queue-bench/compare.py`.

## Validation

- The supplied 5,000-record regression failed on baseline with queue length 5,001.
- It passes with cleanup, asserting queue length <= 4,098 and stale count <= 4,096.
- A second regression loads a wide A query, then advances A to expire records
  without relying on a growth trigger. It checks output order, lookahead on the
  same and next chromosomes, and retained result handles across cleanup.
- `cargo test --tests`: 119 passed, 1 previously ignored.
- `cargo fmt --check` and `git diff --check` pass.
