use crate::string::String;
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

    // this tracks which iterators have been called with Some(Positioned) for a given interval
    // so that calls after the first are called with None.
    called: Vec<bool>, // TODO: use bitset

    // we call this on the first iteration of pull_through_heap
    heap_initialized: bool,
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
        if self.position.chrom() != other.position.chrom() {
            return self
                .chromosome_order
                .get(self.position.chrom())
                .expect("Invalid chromosome")
                .cmp(self.chromosome_order.get(other.position.chrom()).unwrap())
                .reverse();
        }

        let so = self.position.start().cmp(&other.position.start()).reverse();
        match so {
            Ordering::Equal => self.position.stop().cmp(&other.position.stop()).reverse(),
            _ => so,
        }
    }
}

/// cmp will return Less if a is before b, Greater if a is after b, Equal if they overlap.
#[inline(always)]
fn cmp(
    a: &dyn Positioned,
    b: &dyn Positioned,
    chromosome_order: &HashMap<String, usize>,
) -> Ordering {
    if a.chrom() != b.chrom() {
        return chromosome_order[a.chrom()].cmp(&chromosome_order[b.chrom()]);
    }
    // same chrom.
    if a.stop() <= b.start() {
        return Ordering::Less;
    }
    if a.start() >= b.stop() {
        return Ordering::Greater;
    }
    // Equal simply means they overlap.
    Ordering::Equal
}

fn region_str<P: Positioned>(p: &P) -> std::string::String {
    format!("{}:{}-{}", p.chrom(), p.start() + 1, p.stop())
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> Iterator
    for IntersectionIterator<'a, I, P>
{
    type Item = io::Result<Intersections<P>>;

    fn next(&mut self) -> Option<Self::Item> {
        let bi = self.base_iterator.next_position(None)?;

        // if bi is an error return the Result here
        let base_interval = match bi {
            Err(e) => return Some(Err(e)),
            Ok(p) => Rc::new(p),
        };

        if self.out_of_order(base_interval.clone()) {
            let p = self
                .previous_interval
                .as_ref()
                .expect("we know previous interval is_some from out_of_order");
            let msg = format!(
                "intervals from {} out of order {} should be before {}",
                self.base_iterator.name(),
                region_str(p.as_ref()),
                region_str(base_interval.as_ref()),
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
        for o in self.dequeue.iter() {
            match cmp(
                o.interval.as_ref(),
                base_interval.as_ref(),
                self.chromosome_order,
            ) {
                Ordering::Less => continue,
                Ordering::Greater => break,
                Ordering::Equal => overlapping_positions.push(Intersection {
                    // NOTE: we're effectively making a copy here, but it's only incrementing the Rc and a u32...
                    // we could avoid by by keeping entire intersection in Rc.
                    interval: Rc::clone(&o.interval),
                    id: o.id,
                }),
            }
        }

        Some(Ok(Intersections {
            base_interval,
            overlapping: overlapping_positions,
        }))
    }
}

impl<'a, I: PositionedIterator<Item = P>, P: Positioned> IntersectionIterator<'a, I, P> {
    pub fn new(
        base_iterator: I,
        other_iterators: Vec<I>,
        chromosome_order: &'a HashMap<String, usize>,
    ) -> io::Result<Self> {
        let min_heap = BinaryHeap::new();
        let called = vec![false; other_iterators.len()];
        Ok(IntersectionIterator {
            base_iterator,
            other_iterators,
            min_heap,
            chromosome_order,
            dequeue: VecDeque::new(),
            previous_interval: None,
            called,
            heap_initialized: false,
        })
    }

    fn init_heap(&mut self, base_interval: Rc<P>) -> io::Result<()> {
        assert!(!self.heap_initialized);
        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next_position(Some(base_interval.as_ref())) {
                let positioned = positioned?;
                self.min_heap.push(ReverseOrderPosition {
                    position: positioned,
                    chromosome_order: self.chromosome_order,
                    id: i,
                });
            }
        }
        self.heap_initialized = true;
        Ok(())
    }

    /// drop intervals from Q that are strictly before the base interval.
    fn pop_front(&mut self, base_interval: Rc<P>) {
        while !self.dequeue.is_empty()
            && Ordering::Less
                == cmp(
                    self.dequeue[0].interval.as_ref(),
                    base_interval.as_ref(),
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
                pci > ici
                    || (pci == ici && previous_interval.start() > interval.start())
                    || (pci == ici
                        && previous_interval.start() == interval.start()
                        && previous_interval.stop() > interval.stop())
            }
        };
    }
    // reset the array that tracks which iterators have been called with Some(Positioned)
    #[inline]
    fn zero_called(&mut self) {
        let ptr = self.called.as_mut_ptr();
        unsafe { ptr.write_bytes(0, self.called.len()) };
    }

    fn pull_through_heap(&mut self, base_interval: Rc<P>) -> io::Result<()> {
        self.zero_called();
        if !self.heap_initialized {
            // we wait til first iteration here to call init heap
            // because we need the base interval.
            self.init_heap(Rc::clone(&base_interval))?;
        }
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
            // for a given base_interval, we make sure to call next_position with Some, only once.
            // subsequent calls will be with None.
            let arg: Option<&dyn Positioned> = if !self.called[file_index] {
                self.called[file_index] = true;
                Some(base_interval.as_ref())
            } else {
                None
            };
            if let Some(next_position) = f.next_position(arg) {
                let next_position = next_position?;

                // check that intervals within a file are in order.
                if !(position.start() <= next_position.start()
                    || self.chromosome_order[position.chrom()]
                        < self.chromosome_order[next_position.chrom()])
                {
                    let msg = format!(
                        "database intervals out of order ({} -> {}) in iterator: {}",
                        region_str(&position),
                        region_str(&next_position),
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
            if cmp(
                base_interval.as_ref(),
                rc_pos.as_ref(),
                self.chromosome_order,
            ) == Ordering::Greater
            {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Field, Result, Value, ValueError};

    #[derive(Debug, Clone)]
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

        fn value(&self, b: Field) -> Result {
            match b {
                Field::Int(i) => match i {
                    0 => Ok(Value::Strings(vec![self.chrom.clone()])),
                    1 => Ok(Value::Ints(vec![self.start as i64])),
                    2 => Ok(Value::Ints(vec![self.stop as i64])),
                    3 => Ok(Value::Strings(vec![String::from("hello")])),
                    _ => Err(ValueError::InvalidColumnIndex(i)),
                },
                Field::String(s) => match s.as_str() {
                    "chrom" => Ok(Value::Strings(vec![self.chrom.clone()])),
                    "start" => Ok(Value::Ints(vec![self.start as i64])),
                    "stop" => Ok(Value::Ints(vec![self.stop as i64])),
                    "name" => Ok(Value::Strings(vec![String::from("hello")])),
                    _ => Err(ValueError::InvalidColumnName(s)),
                },
            }
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

        fn next_position(&mut self, _o: Option<&dyn Positioned>) -> Option<io::Result<Interval>> {
            if self.i >= self.ivs.len() {
                return None;
            }
            Some(Ok(self.ivs.remove(0)))
        }
    }

    #[test]
    fn many_intervals() {
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);
        let mut ivs = Vec::new();
        let n_intervals = 100;
        for i in 0..n_intervals {
            ivs.push(Interval {
                chrom: String::from("chr1"),
                start: i,
                stop: i + 1,
            })
        }

        let a_ivs = Intervals::new(String::from("A"), ivs.clone());

        let times = 3;
        for _ in 0..times {
            for i in 0..n_intervals {
                ivs.push(Interval {
                    chrom: String::from("chr1"),
                    start: i,
                    stop: i + 1,
                })
            }
        }
        ivs.push(Interval {
            chrom: String::from("chr1"),
            start: n_intervals + 9,
            stop: n_intervals + 10,
        });
        ivs.sort_by(|a, b| a.start.cmp(&b.start));

        let b_ivs = Intervals::new(String::from("B"), ivs.clone());
        let mut iter = IntersectionIterator::new(a_ivs, vec![b_ivs], &chrom_order)
            .expect("error getting iterator");
        let mut n = 0;
        assert!(iter.all(|intersection| {
            let intersection = intersection.expect("error getting intersection");
            n += 1;
            assert!(intersection
                .overlapping
                .iter()
                .all(|p| p.interval.start() == intersection.base_interval.start()));
            intersection.overlapping.len() == times + 1
        }));
        assert_eq!(n, n_intervals)
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
