use crate::chrom_ordering::Chromosome;
use crate::string::String;
use hashbrown::HashMap;
use std::cmp::Ordering;
use std::collections::{vec_deque::VecDeque, BinaryHeap};
use std::io;
use std::io::{Error, ErrorKind};
//use std::rc::Rc;
use std::sync::Arc as Rc;

use crate::position::{Position, PositionedIterator};

/// An iterator that returns the intersection of multiple iterators.
pub struct IntersectionIterator<'a> {
    base_iterator: Box<dyn PositionedIterator>,
    other_iterators: Vec<Box<dyn PositionedIterator>>,
    min_heap: BinaryHeap<ReverseOrderPosition>,
    chromosome_order: &'a HashMap<String, Chromosome>,
    // because multiple intervals from each stream can overlap a single base interval
    // and each interval from others may overlap many base intervals, we must keep a cache (Q)
    // we always add intervals in order with push_back and therefore remove with pop_front.
    // As soon as the front interval in cache is stricly less than the query interval, then we can pop it.
    dequeue: VecDeque<Intersection>,

    // this is only kept for error checking so we can track if intervals are out of order.
    previous_interval: Option<Rc<Position>>,

    // this tracks which iterators have been called with Some(Positioned) for a given interval
    // so that calls after the first are called with None.
    called: Vec<bool>,

    // we call this on the first iteration of pull_through_heap
    heap_initialized: bool,
}

/// An Intersection wraps the Positioned that was intersected with a unique identifier.
/// The u32 identifier matches the index of the database that was intersected.
#[derive(Debug)]
pub struct Intersection {
    /// the Positioned that was intersected
    pub interval: Rc<Position>,
    /// a unique identifier indicating the source of this interval.
    pub id: u32,
}

/// An Intersections wraps the base interval and a vector of overlapping intervals.
#[derive(Debug)]
pub struct Intersections {
    pub base_interval: Rc<Position>,
    pub overlapping: Vec<Intersection>,
}

struct ReverseOrderPosition {
    position: Position,
    chromosome_index: usize, // index order of chrom.
    id: usize,               // file_index
}

impl PartialEq for ReverseOrderPosition {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.position.start() == other.position.start()
            && self.position.stop() == other.position.stop()
            && self.chromosome_index == other.chromosome_index
    }
}

impl Eq for ReverseOrderPosition {}

impl PartialOrd for ReverseOrderPosition {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReverseOrderPosition {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        if self.chromosome_index != other.chromosome_index {
            return self.chromosome_index.cmp(&other.chromosome_index).reverse();
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
fn cmp(a: &Position, b: &Position, chromosome_order: &HashMap<String, Chromosome>) -> Ordering {
    if a.chrom() != b.chrom() {
        return chromosome_order[a.chrom()]
            .index
            .cmp(&chromosome_order[b.chrom()].index);
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

fn region_str(p: &Position) -> std::string::String {
    format!("{}:{}-{}", p.chrom(), p.start() + 1, p.stop())
}

/// An iterator that returns the intersection of multiple iterators for each query interval
impl<'a> Iterator for IntersectionIterator<'a> {
    type Item = io::Result<Intersections>;

    fn next(&mut self) -> Option<Self::Item> {
        let bi = self.base_iterator.next_position(None)?;

        // if bi is an error return the Result here
        let base_interval = match bi {
            Err(e) => return Some(Err(e)),
            Ok(p) => Rc::new(p),
        };
        if let Some(chrom) = self.chromosome_order.get(base_interval.chrom()) {
            if let Some(chrom_len) = chrom.length {
                if base_interval.stop() > chrom_len as u64 {
                    let msg = format!(
                        "interval beyond end of chromosome: {}",
                        region_str(base_interval.as_ref())
                    );
                    return Some(Err(Error::new(ErrorKind::Other, msg)));
                }
            }
        } else {
            let msg = format!("invalid chromosome: {}", region_str(base_interval.as_ref()));
            return Some(Err(Error::new(ErrorKind::Other, msg)));
        }

        if self.out_of_order(base_interval.clone()) {
            let p = self
                .previous_interval
                .as_ref()
                .expect("we know previous interval is_some from out_of_order");
            let msg = format!(
                "intervals from {} out of order {} should be before {}",
                self.base_iterator.name(),
                region_str(p),
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

/// Create a new IntersectionIterator given a query (base) and a vector of other positioned iterators.
impl<'a> IntersectionIterator<'a> {
    pub fn new(
        base_iterator: Box<dyn PositionedIterator>,
        other_iterators: Vec<Box<dyn PositionedIterator>>,
        chromosome_order: &'a HashMap<String, Chromosome>,
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

    fn init_heap(&mut self, base_interval: Rc<Position>) -> io::Result<()> {
        assert!(!self.heap_initialized);
        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next_position(Some(base_interval.as_ref())) {
                let positioned = positioned?;
                let chromosome_index = match self.chromosome_order.get(positioned.chrom()) {
                    Some(c) => c.index,
                    None => {
                        let msg = format!(
                            "invalid chromosome: {} in iterator {}",
                            region_str(&positioned),
                            self.other_iterators[i].name()
                        );
                        return Err(Error::new(ErrorKind::Other, msg));
                    }
                };
                self.min_heap.push(ReverseOrderPosition {
                    position: positioned,
                    chromosome_index,
                    id: i,
                });
            }
        }
        self.heap_initialized = true;
        Ok(())
    }

    /// drop intervals from Q that are strictly before the base interval.
    fn pop_front(&mut self, base_interval: Rc<Position>) {
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

    fn out_of_order(&self, interval: Rc<Position>) -> bool {
        return match &self.previous_interval {
            None => false, // first interval in file.
            Some(previous_interval) => {
                if previous_interval.chrom() != interval.chrom() {
                    let pci = self.chromosome_order[previous_interval.chrom()].index;
                    let ici = self.chromosome_order[interval.chrom()].index;
                    return pci > ici;
                }
                previous_interval.start() > interval.start()
                    || (previous_interval.start() == interval.start()
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

    fn pull_through_heap(&mut self, base_interval: Rc<Position>) -> io::Result<()> {
        self.zero_called();
        if !self.heap_initialized {
            // we wait til first iteration here to call init heap
            // because we need the base interval.
            self.init_heap(Rc::clone(&base_interval))?;
        }
        let other_iterators = self.other_iterators.as_mut_slice();

        while let Some(ReverseOrderPosition {
            position,
            chromosome_index,
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
            let arg: Option<&Position> = if !self.called[file_index] {
                self.called[file_index] = true;
                Some(base_interval.as_ref())
            } else {
                None
            };
            if let Some(next_position) = f.next_position(arg) {
                let next_position = next_position?;
                let next_chromosome = match self.chromosome_order.get(next_position.chrom()) {
                    Some(c) => c,
                    None => {
                        let msg = format!(
                            "invalid chromosome: {} in iterator {}",
                            region_str(&next_position),
                            other_iterators[file_index].name()
                        );
                        return Err(Error::new(ErrorKind::Other, msg));
                    }
                };

                // check that intervals within a file are in order.
                if !(position.start() <= next_position.start()
                    || chromosome_index < next_chromosome.index)
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
                    chromosome_index,
                    id: file_index,
                });
            }

            // and we must always add the position to the Q
            let rc_pos = Rc::new(position);
            let intersection = Intersection {
                interval: rc_pos.clone(),
                id: file_index as u32,
            };
            self.dequeue.push_back(intersection);

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
    use crate::chrom_ordering::parse_genome;
    use crate::interval::Interval;

    struct Intervals {
        i: usize,
        name: String,
        ivs: Vec<Position>,
    }

    impl Intervals {
        fn new(name: String, ivs: Vec<Interval>) -> Self {
            Intervals {
                i: 0,
                name,
                ivs: ivs
                    .into_iter()
                    .map(|i| Position::Interval(i))
                    .collect::<Vec<Position>>(),
            }
        }
        fn add(&mut self, iv: Interval) {
            self.ivs.push(Position::Interval(iv));
        }
    }
    impl PositionedIterator for Intervals {
        fn name(&self) -> String {
            String::from(format!("{}:{}", self.name, self.i))
        }

        fn next_position(&mut self, _o: Option<&Position>) -> Option<io::Result<Position>> {
            if self.i >= self.ivs.len() {
                return None;
            }
            let p = self.ivs.remove(0);
            Some(Ok(p))
        }
    }

    #[test]
    fn many_intervals() {
        let chrom_order = HashMap::from([
            (
                String::from("chr1"),
                Chromosome {
                    index: 0,
                    length: None,
                },
            ),
            (
                String::from("chr2"),
                Chromosome {
                    index: 1,
                    length: None,
                },
            ),
        ]);
        let mut a_ivs = Intervals::new(String::from("A"), Vec::new());
        let mut b_ivs = Intervals::new(String::from("B"), Vec::new());
        let n_intervals = 100;
        let times = 3;
        for i in 0..n_intervals {
            let iv = Interval {
                chrom: String::from("chr1"),
                start: i,
                stop: i + 1,
                ..Default::default()
            };
            a_ivs.add(iv);
            for _ in 0..times {
                let iv = Interval {
                    chrom: String::from("chr1"),
                    start: i,
                    stop: i + 1,
                    ..Default::default()
                };
                b_ivs.add(iv);
            }
        }

        b_ivs.ivs.sort_by(|a, b| a.start().cmp(&b.start()));

        let a_ivs: Box<dyn PositionedIterator> = Box::new(a_ivs);

        let mut iter = IntersectionIterator::new(a_ivs, vec![Box::new(b_ivs)], &chrom_order)
            .expect("error getting iterator");
        let mut n = 0;
        assert!(iter.all(|intersection| {
            let intersection = intersection.expect("error getting intersection");
            n += 1;
            assert!(intersection
                .overlapping
                .iter()
                .all(|p| p.interval.start() == intersection.base_interval.start()));
            intersection.overlapping.len() == times
        }));
        assert_eq!(n, n_intervals)
    }

    #[test]
    fn bookend_and_chrom() {
        let genome_str = "chr1\nchr2\nchr3\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let chrom = String::from("chr1");
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                    ..Default::default()
                },
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                    ..Default::default()
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
                    ..Default::default()
                },
                Interval {
                    chrom: chrom.clone(),
                    start: 0,
                    stop: 10,
                    ..Default::default()
                },
                Interval {
                    // this interval should not overlap.
                    chrom: chrom.clone(),
                    start: 10,
                    stop: 20,
                    ..Default::default()
                },
                Interval {
                    // this interval should not overlap.
                    chrom: String::from("chr2"),
                    start: 1,
                    stop: 20,
                    ..Default::default()
                },
            ],
        );

        let iter = IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order)
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
    fn interval_beyond_end_of_chrom() {
        let genome_str = "chr1\t22\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 10,
                    stop: 22,
                    ..Default::default()
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 23,
                    ..Default::default()
                },
            ],
        );
        let mut iter = IntersectionIterator::new(Box::new(a_ivs), vec![], &chrom_order)
            .expect("error getting iterator");

        let e = iter.nth(1).expect("error getting next");
        assert!(e.is_err());
        let e = e.err().unwrap();
        assert!(e.to_string().contains("beyond end of chromosome"));
    }

    #[test]
    fn ordering_error() {
        let genome_str = "chr1\nchr2\nchr3\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 10,
                    stop: 1,
                    ..Default::default()
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                    ..Default::default()
                },
            ],
        );
        let mut iter = IntersectionIterator::new(Box::new(a_ivs), vec![], &chrom_order)
            .expect("error getting iterator");

        let e = iter.nth(1).expect("error getting next");
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
                    ..Default::default()
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 2,
                    ..Default::default()
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
                    ..Default::default()
                },
                Interval {
                    chrom: String::from("chr1"),
                    start: 0,
                    stop: 2,
                    ..Default::default()
                },
            ],
        );

        let mut iter =
            IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order)
                .expect("error getting iterator");
        let e = iter.next().expect("error getting next");
        assert!(e.is_err());
        let e = e.err().unwrap();
        assert!(e.to_string().contains("out of order"));
    }

    #[test]
    fn multiple_sources() {
        let genome_str = "chr1\nchr2\nchr3\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
                ..Default::default()
            }],
        );
        let b_ivs = Intervals::new(
            String::from("B"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
                ..Default::default()
            }],
        );
        let c_ivs = Intervals::new(
            String::from("c"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 0,
                stop: 1,
                ..Default::default()
            }],
        );
        let iter = IntersectionIterator::new(
            Box::new(a_ivs),
            vec![Box::new(b_ivs), Box::new(c_ivs)],
            &chrom_order,
        )
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
        let genome_str = "chr1\nchr2\nchr3\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let a_ivs = Intervals::new(
            String::from("A"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 1,
                stop: 1,
                ..Default::default()
            }],
        );
        let b_ivs = Intervals::new(
            String::from("B"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 1,
                stop: 1,
                ..Default::default()
            }],
        );
        let iter = IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order)
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
