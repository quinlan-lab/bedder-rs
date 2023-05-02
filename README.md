[![status](https://github.com/brentp/resort-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/brentp/resort-rs/actions/)

# heapiv

This library aims to provide:

- [x] an abstraction so any interval types from sorted sources can be intersected together
- [x] the rust implementation of the heap and Queue to find intersections with minimal overhead
- [ ] downstream APIs to perform operations on the intersections
- [ ] a python library to interact with the intersections

The API looks as follows

Any genomic position from any data source can be intersected by this library as long as it implements this trait:

```rust

pub trait Positioned {
    fn chrom(&self) -> &str;
    fn start(&self) -> u64;
    fn stop(&self) -> u64;

    // Value an enum TBD. this will allow getting info fields of VCF or integer fields of bams.
    //fn get(&self, name: &str) -> Value;
}
```

Then each file-type (VCF/BAM/etc) would implement this trait

```rust
// something that generates Positioned things (BED/VCF/BAM/GFF/etc.)
pub trait PositionedIterator {
    type Item: Positioned;

    fn next(&mut self) -> Option<Self::Item>;

    // A name for the iterator (likely filename) used by this library when logging.
    fn name(&self)
}
```

So, anything that can create a `PositionedIterator` can be used by the library.
The library will follow the methodology in [irelate](https://github.com/brentp/irelate) that uses a min-heap
to combine sorted intervals.

That looks like this:

```rust
pub struct Intersection<P: Positioned> {
    /// the Positioned that was intersected
    pub interval: Rc<P>,
    /// a unique identifier indicating the source (file) of this interval.
    pub id: u32,
}

pub struct Intersections<P: Positioned> {
    pub base_interval: Rc<P>,
    pub overlapping: Vec<Intersection<P>>,
}

let b = PositionedIterator::new(bedfile, vec![bam, otherbed], chrom_order_hashmap)

for intersection in b {
   //... do stuff with intersection.overlapping and intersection.base_interval here.
}

```

# Implementation Brief

All Positioned structs are pulled through a min-heap. Each time an interval is pulled from the min heap (with the smallest genomic position),
an new struct is pulled from the file where that interval originated. Then the pulled interval is pushed onto a `dequeue`.
We then know the dequeue is in order. For each query interval, we drop from the dequeue any interval that is strictly _before_ the interval,
then pull into the Intersection result any interval that is not _after_ the interval. Then return the result from the `next` call.
We use `Rc` because each database interval may be attached to more than one query interval.
