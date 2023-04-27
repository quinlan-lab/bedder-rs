use smartstring::alias::String;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::position::{Position, Positioned, PositionedIterator};

pub struct IntersectionIterator<'a, I: PositionedIterator> {
    base_iterator: I,
    other_iterators: Vec<I>,
    min_heap: BinaryHeap<ReverseOrderPosition<'a>>,
    chromosome_order: &'a HashMap<String, usize>,
}

pub struct Intersection<P: Positioned> {
    base_interval: P,
    overlapping_positions: Vec<P>,
}

struct ReverseOrderPosition<'a> {
    position: Position,
    chromosome_order: &'a HashMap<String, usize>,
    file_index: usize,
}

impl<'a> PartialEq for ReverseOrderPosition<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.position.chromosome == other.position.chromosome
            && self.position.start == other.position.start
            && self.position.stop == other.position.stop
    }
}

impl<'a> Eq for ReverseOrderPosition<'a> {}

impl<'a> PartialOrd for ReverseOrderPosition<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for ReverseOrderPosition<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        let order = self
            .chromosome_order
            .get(&self.position.chromosome)
            .unwrap()
            .cmp(
                self.chromosome_order
                    .get(&other.position.chromosome)
                    .unwrap(),
            );

        match order {
            Ordering::Equal => match self.position.start.cmp(&other.position.start).reverse() {
                Ordering::Equal => self.position.stop.cmp(&other.position.stop).reverse(),
                _ => order,
            },
            _ => order,
        }
    }
}

//pub struct IntersectionIterator<'a, 'b, I: PositionedIterator<'b>> {

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> IntersectionIterator<'a, I> {
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
                position: positioned.position(),
                chromosome_order: self.chromosome_order,
                file_index: 0,
            });
        }

        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next() {
                self.min_heap.push(ReverseOrderPosition {
                    position: positioned.position(),
                    chromosome_order: self.chromosome_order,
                    file_index: i + 1, // Adjust the file_index accordingly
                });
            }
        }
    }
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator for IntersectionIterator<'a, I> {
    //impl<'a: 'b, 'b, I: PositionedIterator<'b>> Iterator for IntersectionIterator<'a, 'b, I> {
    type Item = Intersection<P>;

    fn next(&mut self) -> Option<Self::Item> {
        let base_interval = self.base_iterator.next()?;
        let base_position = base_interval.position();

        let mut overlapping_positions: Vec<Position> = Vec::new();
        let other_iterators = self.other_iterators.as_mut_slice();
        while let Some(ReverseOrderPosition {
            position,
            file_index,
            ..
        }) = &self.min_heap.peek()
        {
            if position.chromosome == base_position.chromosome
                && position.start <= base_position.stop
            {
                let file_index = *file_index;
                let ReverseOrderPosition {
                    position: overlap, ..
                } = self.min_heap.pop().unwrap();
                let f = other_iterators
                    .get_mut(file_index)
                    .expect("expected interval iterator at file index");
                let n = f.next();
                if n.is_some() {
                    self.min_heap.push(ReverseOrderPosition {
                        position: n.unwrap().position(),
                        chromosome_order: self.chromosome_order,
                        file_index: file_index,
                    });
                }

                if overlap.stop >= base_position.start {
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
