# heapiv

This library aims to provide:

1. an abstraction so any interval types can be intersected together
2. the rust implementation of the heap to handle intersections
3. downstream tools to perform operations on the intersections
4. a python library to interact with the intersections

The API will look like something this:

```rust

pub trait Positioned<'a> {
    // we may instead make this 3 separate fn's for chrom, start, stop, but this may be more efficient
    fn position(&self) -> Position<'a>;

    // Value an enum TBD. this will allow getting info fields of VCF or integer fields of bams.
    fn get(&self, name: &str) -> Value;
}

#[derive(Debug)]
pub struct Position<'a> {
    pub chromosome: &'a str,
    pub start: u64,
    pub stop: u64,
}

// something that generates Positioned things (BED/VCF/BAM/GFF/etc.)
pub trait PositionedIterator<'a> {
    type Item: Positioned<'a>;

    fn next(&'a mut self) -> Option<Self::Item>;
}
```

So, anything that can create a `PositionedIterator` can be used by the library.
The library will follow the methodology in [irelate](https://github.com/brentp/irelate) that uses a min-heap
to combine sorted intervals.

That looks like this:

```rust
pub struct IntersectionIterator<'a, 'b, I: PositionedIterator<'b>> {
    base_iterator: I,
    other_iterators: Vec<I>,
    min_heap: BinaryHeap<ReverseOrderPosition<'a>>,
    chromosome_order: &'a HashMap<String, usize>,
    phantom: std::marker::PhantomData<&'b ()>,
}
```

where `base_iterator` is a `PositionedIterator` that is the query file. and `other_iterators` are any number of other files/interval iterators.
(`ReverseOrderPosition` is necessary because the heap is by default a priority(max)-heap.)

This library will handle the overlapping and grabbing from each file and will return:

```rust
pub struct Intersection<'a> {
    base_interval: Position<'a>,
    overlapping_positions: Vec<Position<'a>>,
}
```

In the future, we will add methods to `Intersection` that facilitate common actions such as masking or splitting the `base_interval` by
overlapping_positions in other intervals, etc.
