use smartstring::alias::String;
use std::cmp::Ordering;
use std::collections::{vec_deque::VecDeque, BinaryHeap, HashMap};
use std::io;
use std::io::{Error, ErrorKind};
use std::rc::Rc;
//use std::sync::Arc as Rc;

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
    dequeue: VecDeque<Intersection<P>>,

    // this is only kept for error checking so we can track if intervals are out of order.
    previous_interval: Option<Rc<P>>,
}

#[derive(Debug)]
pub struct Intersection<P: Positioned> {
    /// the Positioned that was intersected
    pub interval: Rc<P>,
    /// a unique identifier indicating the source of this interval.
    pub id: u32,
}

#[derive(Debug)]
pub struct Intersections<P: Positioned> {
    pub base_interval: Rc<P>,
    pub overlapping: Vec<Intersection<P>>,
}

struct ReverseOrderPosition<'a, P: Positioned> {
    position: P,
    chromosome_order: &'a HashMap<String, usize>,
    id: usize, // file_index
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
            .expect("Invalid chromosome")
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
    ) -> io::Result<Self> {
        let min_heap = BinaryHeap::new();
        let mut ii = IntersectionIterator {
            base_iterator,
            other_iterators,
            min_heap,
            chromosome_order,
            dequeue: VecDeque::new(),
            previous_interval: None,
        };
        ii.init_heap()?;
        Ok(ii)
    }

    fn init_heap(&mut self) -> io::Result<()> {
        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next_position() {
                let positioned = positioned?;
                self.min_heap.push(ReverseOrderPosition {
                    position: positioned,
                    chromosome_order: self.chromosome_order,
                    id: i,
                });
            }
        }
        Ok(())
    }

    /// drop intervals from Q that are strictly before the base interval.
    fn pop_front(&mut self, base_interval: Rc<P>) {
        while !self.dequeue.is_empty()
            && lt(
                self.dequeue[0].interval.clone(),
                Rc::clone(&base_interval),
                self.chromosome_order,
            )
        {
            _ = self.dequeue.pop_front();
        }
    }

    fn out_of_order(&self, interval: Rc<P>) -> bool {
        return match &self.previous_interval {
            None => false, // first interval in file.
            Some(previous_interval) => {
                let pci = self.chromosome_order[previous_interval.chrom()];
                let ici = self.chromosome_order[interval.chrom()];
                pci > ici || (pci == ici && previous_interval.start() > interval.start())
            }
        };
    }

    fn pull_through_heap(&mut self, base_interval: Rc<P>) -> io::Result<()> {
        let other_iterators = self.other_iterators.as_mut_slice();
        while let Some(ReverseOrderPosition {
            position,
            id: file_index,
            ..
        }) = self.min_heap.pop()
        {
            // must always pull into the heap.
            let f = other_iterators
                .get_mut(file_index)
                .expect("expected interval iterator at file index");
            if let Some(next_position) = f.next_position() {
                let next_position = next_position?;

                // check that intervals within a file are in order.
                if !(position.start() <= next_position.start()
                    || self.chromosome_order[position.chrom()]
                        < self.chromosome_order[next_position.chrom()])
                {
                    let msg = format!(
                        "database intervals out of order ({} -> {}) in iterator: {}",
                        region_str(position),
                        region_str(next_position),
                        other_iterators[file_index].name()
                    );
                    return Err(Error::new(ErrorKind::Other, msg));
                }
                self.min_heap.push(ReverseOrderPosition {
                    position: next_position,
                    chromosome_order: self.chromosome_order,
                    id: file_index,
                });
            }

            // and we must always add the position to the Q
            let rc_pos = Rc::new(position);
            let int = Intersection {
                interval: rc_pos.clone(),
                id: file_index as u32,
            };
            self.dequeue.push_back(int);

            // if this position is after base_interval, we can stop pulling through heap.
            if lt(base_interval.clone(), rc_pos, self.chromosome_order) {
                break;
            }
        }
        Ok(())
    }
}

#[inline]
/// if a is strictly less than b. either on earlier chrom, or stops before the start of b.
fn lt<P: Positioned>(a: Rc<P>, b: Rc<P>, chromosome_order: &HashMap<String, usize>) -> bool {
    // TODO: make this return Ordering::Less so we can call it only once.
    if a.chrom() != b.chrom() {
        chromosome_order[a.chrom()] < chromosome_order[b.chrom()]
    } else {
        a.stop() <= b.start()
    }
}

fn region_str<P: Positioned>(p: P) -> std::string::String {
    format!("{}:{}-{}", p.chrom(), p.start() + 1, p.stop())
}
fn rc_region_str<P: Positioned>(p: Rc<P>) -> std::string::String {
    format!("{}:{}-{}", p.chrom(), p.start() + 1, p.stop())
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator
    for IntersectionIterator<'a, I, P>
{
    type Item = io::Result<Intersections<P>>;

    fn next(&mut self) -> Option<Self::Item> {
        let bi = self.base_iterator.next_position()?;
        // if bi is an error return the Result here

        let base_interval = match bi {
            Err(e) => return Some(Err(e)),
            Ok(p) => Rc::new(p),
        };

        if self.out_of_order(base_interval.clone()) {
            let msg = format!(
                "intervals from {} out of order {} should be before {}",
                self.base_iterator.name(),
                rc_region_str(
                    self.previous_interval
                        .as_ref()
                        .expect("out_of_order checks previous_interval is some")
                        .clone()
                ),
                rc_region_str(base_interval.clone()),
            );
            return Some(Err(Error::new(ErrorKind::Other, msg)));
        }

        self.previous_interval = Some(base_interval.clone());

        // drop intervals from Q that are strictly before the base interval.
        self.pop_front(base_interval.clone());

        // pull intervals through the min-heap until the base interval is strictly less than the
        // last pulled interval.
        // we want all intervals to pass through the min_heap so that they are ordered across files
        if let Err(e) = self.pull_through_heap(base_interval.clone()) {
            return Some(Err(e));
        }

        let mut overlapping_positions = Vec::new();
        // de-Q contains all intervals that can overlap with the base interval.
        // de-Q is sorted.
        // We iterate through (again) and add those to overlapping positions.
        for p in self.dequeue.iter() {
            if lt(
                Rc::clone(&p.interval),
                Rc::clone(&base_interval),
                self.chromosome_order,
            ) {
                // could pop here. but easier to do at start above.
                continue;
            }
            if lt(
                Rc::clone(&base_interval),
                Rc::clone(&p.interval),
                self.chromosome_order,
            ) {
                break;
            }
            overlapping_positions.push(Intersection {
                // NOTE: we're effectively making a copy here, but it's only incrementing the Rc and a u32...
                // we could avoid by by keeping entire intersection in Rc.
                interval: Rc::clone(&p.interval),
                id: p.id,
            });
        }

        Some(Ok(Intersections {
            base_interval,
            overlapping: overlapping_positions,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Interval {
        chrom: String,
        start: u64,
        stop: u64,
    }

    impl Positioned for Interval {
        fn start(&self) -> u64 {
            self.start
        }
        fn stop(&self) -> u64 {
            self.stop
        }
        fn chrom(&self) -> &str {
            &self.chrom
        }
    }
    struct Intervals {
        i: usize,
        name: String,
        ivs: Vec<Interval>,
    }

    impl Intervals {
        fn new(name: String, ivs: Vec<Interval>) -> Self {
            Intervals { i: 0, name, ivs }
        }
    }

    impl PositionedIterator for Intervals {
        type Item = Interval;

        fn name(&self) -> String {
            String::from(format!("{}:{}", self.name, self.i))
        }

        fn next_position(&mut self) -> Option<io::Result<Interval>> {
            if self.i >= self.ivs.len() {
                return None;
            }
            Some(Ok(self.ivs.remove(0)))
        }
    }

    #[test]
    fn bookend_and_chrom() {
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);
        let chrom = String::from("chr1");
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                },
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                },
            ],
        );

        let b_ivs = Intervals::new(
            String::from("B"),
            vec![
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 5,
                },
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                },
                Interval {
                    // this interval should not overlap.
                    chrom: chrom.clone(),
                    start: 10,
                    stop: 20,
                },
                Interval {
                    // this interval should not overlap.
                    chrom: String::from("chr2"),
                    start: 1,
                    stop: 20,
                },
            ],
        );

        let iter = IntersectionIterator::new(a_ivs, vec![b_ivs], &chrom_order)
            .expect("error getting iterator");
        iter.for_each(|intersection| {
            let intersection = intersection.expect("intersection");
            assert_eq!(intersection.overlapping.len(), 2);
            assert!(intersection
                .overlapping
                .iter()
                .all(|p| { p.interval.start() == 0 }));
        })
    }

    #[test]
    fn ordering_error() {
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 10,
                    stop: 1,
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                },
            ],
        );
        let iter =
            IntersectionIterator::new(a_ivs, vec![], &chrom_order).expect("error getting iterator");

        let e = iter.skip(1).next().expect("error getting next");
        assert!(e.is_err());
        let e = e.err().unwrap();
        assert!(e.to_string().contains("out of order"));

        // now repeat with database out of order.
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                },
            ],
        );
        // now repeat with database out of order.
        let b_ivs = Intervals::new(
            String::from("B"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 0,
                    stop: 2,
                },
            ],
        );

        let mut iter = IntersectionIterator::new(a_ivs, vec![b_ivs], &chrom_order)
            .expect("error getting iterator");
        let e = iter.next().expect("error getting next");
        assert!(e.is_err());
        let e = e.err().unwrap();
        assert!(e.to_string().contains("out of order"));
    }

    #[test]
    fn multiple_sources() {
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
            }],
        );
        let b_ivs = Intervals::new(
            String::from("B"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
            }],
        );
        let c_ivs = Intervals::new(
            String::from("c"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
            }],
        );
        let iter = IntersectionIterator::new(a_ivs, vec![b_ivs, c_ivs], &chrom_order)
            .expect("error getting iterator");
        let c = iter
            .map(|intersection| {
                let intersection = intersection.expect("error getting intersection");
                dbg!(&intersection.overlapping);
                assert_eq!(intersection.overlapping.len(), 2);
                // check that we got from source 1 and source 2.
                assert_ne!(
                    intersection.overlapping[0].id,
                    intersection.overlapping[1].id
                );
                1
            })
            .sum::<usize>();
        assert_eq!(c, 1);
    }

    #[test]
    #[ignore]
    fn zero_length() {
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 1,
                stop: 1,
            }],
        );
        let b_ivs = Intervals::new(
            String::from("B"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 1,
                stop: 1,
            }],
        );
        let iter = IntersectionIterator::new(a_ivs, vec![b_ivs], &chrom_order)
            .expect("error getting iterator");
        // check that it overlapped by asserting that the loop ran and also that there was an overlap within the loop.
        let c = iter
            .map(|intersection| {
                let intersection = intersection.expect("error getting intersection");
                assert!(intersection.overlapping.len() == 1);
                1
            })
            .sum::<usize>();
        // NOTE this fails as we likely need to fix the lt function.
        assert_eq!(c, 1);
    }
}
