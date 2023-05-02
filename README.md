
[![status](https://github.com/brentp/resort-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/brentp/resort-rs/actions/)

# heapiv

This library aims to provide:

1. an abstraction so any interval types can be intersected together
2. the rust implementation of the heap to handle intersections
3. downstream tools to perform operations on the intersections
4. a python library to interact with the intersections

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
```
// something that generates Positioned things (BED/VCF/BAM/GFF/etc.)
pub trait PositionedIterator {
    type Item: Positioned;

    fn next(&'a mut self) -> Option<Self::Item>;
}
```

So, anything that can create a `PositionedIterator` can be used by the library.
The library will follow the methodology in [irelate](https://github.com/brentp/irelate) that uses a min-heap
to combine sorted intervals.

That looks like this:

```rust
pub struct IntersectionIterator<'a, I: PositionedIterator, P: Positioned> {
  // opaque
}

pub struct Intersection<P: Positioned> {
    base_interval: Rc<P>,
    overlapping_positions: Vec<Rc<P>>,
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator
    for IntersectionIterator<'a, I, P>
{

    type Item = Intersection<P>;

    fn next(&mut self) -> Option<Self::Item> { ... }
}

let b = PositionedIterator::new(bedfile, vec![bam, otherbed], chrom_order_hashmap)

for intersection in b {
   //... do stuff with Intersection here.
}

```

# Implementation Brief

All Positioned structs are pulled through a min-heap. Each time an interval is pulled from the min heap (with the smallest genomic position),
an new struct is pulled from the file where that interval originated. Then the pulled interval is pushed onto a `dequeue`.
We then know the dequeue is in order. For each query interval, we drop from the dequeue any interval that is strictly *before* the interval,
then pull into the Intersection result any interval that is not *after* the interval. Then return the result from the `next` call.
