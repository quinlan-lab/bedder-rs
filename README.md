<!--- 
# build
target=x86_64-unknown-linux-gnu
export RUSTFLAGS="-C target-feature=-crt-static -C relocation-model=pie"
cargo test --release --target $target \
&& cargo build --release --target $target
--->

[![status](https://github.com/brentp/bedder-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/brentp/bedder-rs/actions/)

# bedder (tools)

This library aims to provide:

- [x] an abstraction so any interval types from sorted sources can be intersected together
- [x] the rust implementation of the heap and Queue to find intersections with minimal overhead
- [ ] bedder wrappers for:
  + [x] bed
  + [x] vcf/bcf
  + [ ] sam/bam/cram
  + [ ] gff/gtf
  + [ ] generalized tabixed/csi files
- [ ] downstream APIs to perform operations on the intersections
- [ ] a python library to interact with the intersections

The API looks as follows

Any genomic position from any data source can be intersected by this library as long as it implements this trait:

```rust

pub trait Positioned {
    fn chrom(&self) -> &str;
    fn start(&self) -> u64;
    fn stop(&self) -> u64;

    // extract a value from the Positioned object Field
    fn value(&self, f: Field) -> Result<Value, FieldError>;
}

/// Value can be any number of Ints, Floats, or Strings.
pub enum Value {
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

/// Field is either an integer: the i'th column.
/// Or a String, e.g. "INFO.DP".
pub enum Field {
    String(String),
    Int(usize),
}

pub enum FieldError {
    InvalidFieldIndex(usize),
    InvalidFieldName(String),
}

```

Then each file-type (VCF/BAM/etc) would implement this trait

```rust
// something that generates Positioned things (BED/VCF/BAM/GFF/etc.)
pub trait PositionedIterator {
    type Item: Positioned;

    /// Q can be ignored. See below for more detail.
    fn next_position(&mut self, q: Option<&dyn Positioned>) -> Option<Self::Item>;

    /// A name for the iterator (likely filename) used by this library when logging.
    fn name(&self)
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
a new struct is pulled from the file where that interval originated. Then the pulled interval is pushed onto a `dequeue`.
We then know the dequeue is in order. For each query interval, we drop from the dequeue any interval that is strictly _before_ the interval,
then pull into the Intersection result any interval that is not _after_ the interval. Then return the result from the `next` call.
We use `Rc` because each database interval may be attached to more than one query interval.

# Acknowledgements

- We received very valuable `rust` feedback from @sstadick.
- We leverage the excellent [noodles](https://github.com/zaeleus/noodles) library.
