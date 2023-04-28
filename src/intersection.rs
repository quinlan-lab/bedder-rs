use smartstring::alias::String;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::position::{Positioned, PositionedIterator};

pub struct IntersectionIterator<'a, I: PositionedIterator, P: Positioned> {
    base_iterator: I,
    other_iterators: Vec<I>,
    min_heap: BinaryHeap<ReverseOrderPosition<'a, P>>,
    chromosome_order: &'a HashMap<String, usize>,
}

pub struct Intersection<P: Positioned> {
    base_interval: P,
    overlapping_positions: Vec<P>,
}

struct ReverseOrderPosition<'a, P: Positioned> {
    position: P,
    chromosome_order: &'a HashMap<String, usize>,
    file_index: usize,
}

impl<'a, P: Positioned> PartialEq for ReverseOrderPosition<'a, P> {
    fn eq(&self, other: &Self) -> bool {
        self.position.start() == other.position.start()
            && self.position.stop() == other.position.stop()
            && self.position.chrom() == other.position.chrom()
    }
}

impl<'a, P: Positioned> Eq for ReverseOrderPosition<'a, P> {}

impl<'a, P: Positioned> PartialOrd for ReverseOrderPosition<'a, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, P: Positioned> Ord for ReverseOrderPosition<'a, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        let order = self
            .chromosome_order
            .get(self.position.chrom())
            .unwrap()
            .cmp(self.chromosome_order.get(other.position.chrom()).unwrap());

        match order {
            Ordering::Equal => {
                let so = self.position.start().cmp(&other.position.start()).reverse();
                match so {
                    Ordering::Equal => self.position.stop().cmp(&other.position.stop()).reverse(),
                    _ => so,
                }
            }
            _ => order,
        }
    }
}

//pub struct IntersectionIterator<'a, 'b, I: PositionedIterator<'b>> {

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> IntersectionIterator<'a, I, P> {
    pub fn new(
        base_iterator: I,
        other_iterators: Vec<I>,
        chromosome_order: &'a HashMap<String, usize>,
    ) -> Self {
        let min_heap = BinaryHeap::new();
        let mut ii = IntersectionIterator {
            base_iterator,
            other_iterators,
            min_heap,
            chromosome_order,
        };
        ii.init_heap();
        ii
    }

    fn init_heap(&mut self) {
        if let Some(positioned) = self.base_iterator.next() {
            self.min_heap.push(ReverseOrderPosition {
                position: positioned,
                chromosome_order: self.chromosome_order,
                file_index: 0,
            });
        }

        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next() {
                self.min_heap.push(ReverseOrderPosition {
                    position: positioned,
                    chromosome_order: self.chromosome_order,
                    file_index: i + 1, // Adjust the file_index accordingly
                });
            }
        }
    }
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator
    for IntersectionIterator<'a, I, P>
{
    //impl<'a: 'b, 'b, I: PositionedIterator<'b>> Iterator for IntersectionIterator<'a, 'b, I> {
    type Item = Intersection<P>;

    fn next(&mut self) -> Option<Self::Item> {
        let base_interval = self.base_iterator.next()?;

        let mut overlapping_positions: Vec<P> = Vec::new();
        let other_iterators = self.other_iterators.as_mut_slice();
        while let Some(ReverseOrderPosition {
            position,
            file_index,
            ..
        }) = &self.min_heap.peek()
        {
            if position.chrom() == base_interval.chrom() && position.start() <= base_interval.stop()
            {
                let file_index = *file_index;
                let ReverseOrderPosition {
                    position: overlap, ..
                } = self.min_heap.pop().unwrap();
                // NOTE: can't pop here. we leave it on the heap unless the stop is before this interval.
                let f = other_iterators
                    .get_mut(file_index)
                    .expect("expected interval iterator at file index");
                match f.next() {
                    Some(p) => {
                        self.min_heap.push(ReverseOrderPosition {
                            position: p,
                            chromosome_order: self.chromosome_order,
                            file_index: file_index,
                        });
                    }
                    _ => eprintln!("end of file"),
                }

                if overlap.stop() >= base_interval.start() {
                    overlapping_positions.push(overlap);
                }
            } else {
                break;
            }
        }

        Some(Intersection {
            base_interval,
            overlapping_positions,
        })
    }
}
