<!--- 
# build
target=x86_64-unknown-linux-gnu
export RUSTFLAGS="-C target-feature=-crt-static -C relocation-model=pie"
cargo test --release --target $target \
&& cargo build --release --target $target
--->

[![status](https://github.com/quinlan-lab/bedder-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/quinlan-lab/bedder-rs/actions/)

# bedder (tools)

This is an early release of the library for feedback, especially from rust practitioners. If interested,
read below and then, for example, have a look at [issue 2](https://github.com/quinlan-lab/bedder-rs/issues/2) and the associated [discussion](https://github.com/quinlan-lab/bedder-rs/discussions/3)

## Problem statement

BEDTools is extremely useful but adding features and maintaining existing ones is a challenge.
We want a library (in bedder) that is:

1. flexible enough to support most use-cases in BEDTools
2. fast enough
3. extensible so that we don't need a custom tool for every possible use-case.

## Solution

To do this, we provide the machinery to intersect common genomics file formats (and more can be added by implementing a simple trait)
and we allow the user to write custom python snippets that are applied to that intersection.
As a silly example, the user may want to count overlaps but only if the start position of the overlapping interval is even; that could be
done with this expression:

```Python
len(o for o in intersection.overlapping if o.start % 2 == 0])
```

It is common to require certain *constraints* on the intersections like a percent or number of bases of overlap.
We can get those with:

```python
a_mode = PyIntersectionMode.default() # report the full interval like -v in bedtools
b_part = PyIntersectionPart.inverse() # report the part of the b-intervals that do not overlap the a-interval
a_requirements = PyOverlapAmount.fraction(0.5) # require at least 50% of the a_interval to be covered
report = intersection.report(a_mode, None, None, b_part, a_requirements, None)
result = []
for ov in report:
    line = [f"{ov.a.chrom}\t{ov.a.start}\t{ov.a.stop}"]
    for b in ov.b:
        line.append(f"{b.start}\t{b.stop}")
    result.append("\t".join(line))
"\n".join(result)
```

This library aims to provide:

- [x] an abstraction so any interval types from sorted sources can be intersected together
- [x] the rust implementation of the heap and Queue to find intersections with minimal overhead
- [ ] bedder wrappers for:
  - [x] bed
  - [x] vcf/bcf
  - [ ] sam/bam/cram
  - [ ] gff/gtf
  - [ ] generalized tabixed/csi files
- [ ] downstream APIs to perform operations on the intersections
- [ ] a python library to interact with the intersections

The API looks as follows

Any genomic position from any data source can be intersected by this library as long as it implements this trait:

```rust

pub trait Positioned {
    fn chrom(&self) -> &str;

    fn start(&self) -> u64;
    fn set_start(&self, u64);

    fn stop(&self) -> u64;
    fn set_stop(&self, u64);
}
```

Then each file-type (VCF/BAM/etc) would implement this trait

```rust
// something that generates Positioned things (BED/VCF/BAM/GFF/etc.)
pub trait PositionedIterator {
    /// A name for the iterator. This is most often the file path, perhaps with the line number appended.
    /// Used to provide informative messages to the user.
    fn name(&self) -> String;

    /// return the next Positioned from the iterator.
    fn next_position(&mut self, q: Option<&Position>) -> Option<std::io::Result<Position>>;
}
```

Anything that can create a `PositionedIterator` can be used by the library.

Note the `q` argument to `next_position`. This can be ignored by implementers but can be used to skip.
For each query interval, we may make many calls to `next_position`. On the first of those calls, `q`
is `Some(query_position)`. The implementer can choose to use this information to skip (rather than stream)
for example with an index (or search) to the first interval that overlaps the `q`. Subsequent calls for the
same interval will be called with `q` of `None`. The implementer must:

1. Always return an interval (unless EOF is reached)
1. Always return intervals in order.
1. Never return an interval that was returned previously (even if the same query interval appears multiple times).

# Implementation Brief

All Positioned structs are pulled through a min-heap. Each time an interval (with the smallest genomic position) is pulled from the min heap,
a new struct is pulled from the file where that interval originated. Then the pulled interval is pushed onto a `queue` (actually a deque becase that's what is in the rust standard library).
We then know the queue is in order. For each query interval, we drop from the queue any interval that is strictly *before* the interval,
then pull into the Intersection result any interval that is not *after* the interval. Then return the result from the `next` call.
We use `Rc` because each database interval may be attached to more than one query interval.

# Acknowledgements

- We received very valuable `rust` feedback and code from @sstadick.
- We leverage the excellent [noodles](https://github.com/zaeleus/noodles) library.
