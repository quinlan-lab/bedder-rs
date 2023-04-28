use smartstring::alias::String;
use std::cmp::Ordering;
use std::collections::{vec_deque::VecDeque, BinaryHeap, HashMap};
use std::rc::Rc;

use crate::position::{Positioned, PositionedIterator};

pub struct IntersectionIterator<'a, I: PositionedIterator, P: Positioned> {
    base_iterator: I,
    other_iterators: Vec<I>,
    min_heap: BinaryHeap<ReverseOrderPosition<'a, P>>,
    chromosome_order: &'a HashMap<String, usize>,
    // because multiple intervals from each stream can overlap a single base interval
    // and each interval from others may overlap many base intervals, we must keep a cache (Q)
    // we always add intervals in order with push_back and therefore remove with pop_front.
    // As soon as the front interval in cache is stricly less than the query interval, then we can pop it.
    dequeue: VecDeque<Rc<P>>,
}

pub struct Intersection<P: Positioned> {
    base_interval: P,
    overlapping_positions: Vec<Rc<P>>,
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
            dequeue: VecDeque::new(),
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

#[inline]
/// if a is strictly less than b. either on earlier chrom, or stops before the start of b.
fn lt<'a>(
    a: Rc<dyn Positioned>,
    b: Rc<dyn Positioned>,
    chromosome_order: &'a HashMap<String, usize>,
) -> bool {
    if a.chrom() != b.chrom() {
        chromosome_order[a.chrom()] < chromosome_order[b.chrom()]
    } else {
        a.stop() < b.start()
    }
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator
    for IntersectionIterator<'a, I, P>
{
    type Item = Intersection<P>;

    fn next(&mut self) -> Option<Self::Item> {
        let base_interval = Rc::new(self.base_iterator.next()?);

        // drop intervals from Q that are strictly before the base interval.
        while self.dequeue.len() > 0
            && lt(
                self.dequeue[0].clone(),
                base_interval,
                &self.chromosome_order,
            )
        {
            _ = self.dequeue.pop_front();
        }

        let other_iterators = self.other_iterators.as_mut_slice();

        // now pull through the min-heap until base_interval is strictly less than other interval
        // we want all intervals to pass through the min_heap so that they are ordered across files
        while let Some(ReverseOrderPosition {
            position: overlap,
            file_index,
            ..
        }) = &self.min_heap.pop()
        {
            // must always pull into the heap.
            let f = other_iterators
                .get_mut(*file_index)
                .expect("expected interval iterator at file index");
            match f.next() {
                Some(p) => {
                    self.min_heap.push(ReverseOrderPosition {
                        position: p,
                        chromosome_order: self.chromosome_order,
                        file_index: *file_index,
                    });
                }
                _ => eprintln!("end of file"),
            }
            // and we must always add the position to the Q
            let r = Rc::new(overlap);
            self.dequeue.push_back(r);

            // if this position is after base_interval, we can stop pulling from files via heap.
            if lt(base_interval, r.clone(), &self.chromosome_order) {
                break;
            }
        }

        let mut overlapping_positions = Vec::new();
        // Q contains all intervals that can overlap with the base interval.
        // Q is sorted.
        // We iterate through (again) and add those to overlapping positions.
        for p in &self.dequeue {
            if lt(p.clone(), base_interval, &self.chromosome_order) {
                // could pop here. but easier to do at start above.
                continue;
            }
            if lt(base_interval, p.clone(), &self.chromosome_order) {
                break;
            }
            overlapping_positions.push(p.clone());
        }

        Some(Intersection {
            base_interval,
            overlapping_positions,
        })
    }
}
