use crate::chrom_ordering::Chromosome;
use crate::report::Report;
use crate::report_options::ReportOptions;
use crate::string::String;
use hashbrown::HashMap;
use parking_lot::Mutex;
use std::cmp::Ordering;
use std::collections::{vec_deque::VecDeque, BinaryHeap};
use std::io;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

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
    previous_interval: Option<Arc<Mutex<Position>>>,

    // this tracks which iterators have been called with Some(Positioned) for a given interval
    // so that calls after the first are called with None.
    called: Vec<bool>,

    // we call this on the first iteration of pull_through_heap
    heap_initialized: bool,

    /// max_distance is the maximum distance from the base interval that we will consider.
    /// This takes precedence over n_closest.
    max_distance: i64,

    /// n_closest is the number of closest intervals to return. they may not all be overlapping.
    n_closest: i64,
}

/// An Intersection wraps the Positioned that was intersected with a unique identifier.
/// The u32 identifier matches the index of the database that was intersected.
#[derive(Debug)]
pub struct Intersection {
    /// the Positioned that was intersected
    pub interval: Arc<Mutex<Position>>,
    /// a unique identifier indicating the source of this interval.
    pub id: u32,
}

impl Clone for Intersection {
    fn clone(&self) -> Self {
        Intersection {
            interval: Arc::clone(&self.interval),
            id: self.id,
        }
    }
}

/// An Intersections wraps the base interval and a vector of overlapping intervals.
#[derive(Debug, Clone)]
pub struct Intersections {
    pub base_interval: Arc<Mutex<Position>>,
    pub overlapping: Vec<Intersection>,

    // report cache, keyed by report_options. Use Arc Mutex for interior mutability.
    pub(crate) cached_report: Arc<Mutex<Option<(ReportOptions, Arc<Report>)>>>,
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
impl Iterator for IntersectionIterator<'_> {
    type Item = io::Result<Intersections>;

    fn next(&mut self) -> Option<Self::Item> {
        let bi = self.base_iterator.next_position(None)?;

        // if bi is an error return the Result here
        let base_interval = match bi {
            Err(e) => return Some(Err(e)),
            Ok(p) => p,
        };
        if let Some(chrom) = self.chromosome_order.get(base_interval.chrom()) {
            if let Some(chrom_len) = chrom.length {
                if base_interval.stop() > chrom_len as u64 {
                    let msg = format!(
                        "interval beyond end of chromosome: {}",
                        region_str(&base_interval)
                    );
                    return Some(Err(Error::new(ErrorKind::Other, msg)));
                }
            }
        } else {
            let msg = format!("invalid chromosome: {}", region_str(&base_interval));
            return Some(Err(Error::new(ErrorKind::Other, msg)));
        }

        if self.out_of_order(&base_interval) {
            let p = self
                .previous_interval
                .as_ref()
                .expect("we know previous interval is_some from out_of_order");
            let msg = format!(
                "intervals from {} out of order {} should be before {}",
                self.base_iterator.name(),
                region_str(&p.try_lock().expect("failed to lock previous interval")),
                region_str(&base_interval),
            );
            return Some(Err(Error::new(ErrorKind::Other, msg)));
        }
        // drop intervals from Q that are strictly before the base interval.
        self.pop_front(&base_interval);

        let base_interval = Arc::new(Mutex::new(base_interval));
        self.previous_interval = Some(base_interval.clone());

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
        let base_interval_locked = base_interval
            .try_lock()
            .expect("failed to lock base_interval");
        if self.n_closest <= 0 && self.max_distance <= 0 {
            for o in self.dequeue.iter() {
                match cmp(
                    &o.interval.try_lock().expect("failed to lock interval"),
                    &*base_interval_locked,
                    self.chromosome_order,
                ) {
                    Ordering::Less => continue,
                    Ordering::Greater => break,
                    Ordering::Equal => overlapping_positions.push(o.clone()),
                }
            }
        } else {
            // logic for closest `n` and/or `max_distance` without extra allocations.

            // 1. Find the split point in `dequeue` which is the index of the first interval
            // that is NOT before our `base_interval`.
            let mut split_point = 0;
            for (i, o) in self.dequeue.iter().enumerate() {
                let interval = o.interval.try_lock().unwrap();
                let o_chrom = interval.chrom();
                let base_chrom = base_interval_locked.chrom();

                if o_chrom != base_chrom {
                    if self.chromosome_order[o_chrom].index
                        < self.chromosome_order[base_chrom].index
                    {
                        split_point = i + 1;
                        continue;
                    } else {
                        break;
                    }
                }

                if interval.stop() <= base_interval_locked.start() {
                    split_point = i + 1;
                } else {
                    break;
                }
            }

            // 2. We now have pointers to the end of the 'before' list and the start of the 'not-before' list.
            let mut before_ptr = split_point.checked_sub(1);
            let mut after_ptr = split_point;

            // 3. Collect all overlapping intervals first. They have distance 0.
            while let Some(o) = self.dequeue.get(after_ptr) {
                let interval = o.interval.try_lock().unwrap();
                if interval.chrom() == base_interval_locked.chrom()
                    && interval.start() < base_interval_locked.stop()
                {
                    overlapping_positions.push(o.clone());
                    after_ptr += 1;
                } else {
                    break;
                }
            }

            // 4. If we still need more intervals (for n_closest), get them from `before` and `after` parts of dequeue.
            if self.n_closest > 0 {
                while overlapping_positions.len() < self.n_closest as usize {
                    let before_o = before_ptr.and_then(|p| self.dequeue.get(p));
                    let after_o = self.dequeue.get(after_ptr);

                    if before_o.is_none() && after_o.is_none() {
                        break;
                    }

                    let dist_l = before_o.map_or(u64::MAX, |o| {
                        let interval = o.interval.try_lock().unwrap();
                        if interval.chrom() != base_interval_locked.chrom() {
                            return u64::MAX;
                        }
                        let dist = base_interval_locked.start() - interval.stop();
                        if self.max_distance > 0 && dist > self.max_distance as u64 {
                            u64::MAX
                        } else {
                            dist
                        }
                    });

                    let dist_r = after_o.map_or(u64::MAX, |o| {
                        let interval = o.interval.try_lock().unwrap();
                        if interval.chrom() != base_interval_locked.chrom() {
                            return u64::MAX;
                        }
                        let dist = interval.start() - base_interval_locked.stop();
                        if self.max_distance > 0 && dist > self.max_distance as u64 {
                            u64::MAX
                        } else {
                            dist
                        }
                    });

                    if dist_l == u64::MAX && dist_r == u64::MAX {
                        break;
                    }

                    if dist_l <= dist_r {
                        overlapping_positions.push(before_o.unwrap().clone());
                        before_ptr = before_ptr.unwrap().checked_sub(1);
                    } else {
                        overlapping_positions.push(after_o.unwrap().clone());
                        after_ptr += 1;
                    }
                }

                // The found positions are the closest, but not necessarily sorted by distance.
                // We sort and truncate to get the exact n-closest in order.
                overlapping_positions.sort_by_key(|o| {
                    let interval = o.interval.try_lock().unwrap();
                    if interval.stop() > base_interval_locked.start()
                        && base_interval_locked.stop() > interval.start()
                    {
                        0
                    } else if interval.stop() <= base_interval_locked.start() {
                        base_interval_locked.start() - interval.stop()
                    } else {
                        interval.start() - base_interval_locked.stop()
                    }
                });
                overlapping_positions.truncate(self.n_closest as usize);
            } else {
                // n_closest is 0, but max_distance is set. Collect all within distance.
                while let Some(p) = before_ptr {
                    let o = &self.dequeue[p];
                    let interval = o.interval.try_lock().unwrap();
                    if interval.chrom() == base_interval_locked.chrom() {
                        assert!(base_interval_locked.start() >= interval.stop());
                        let dist = base_interval_locked.start() - interval.stop();
                        if self.max_distance >= 0 && dist <= self.max_distance as u64 {
                            overlapping_positions.push(o.clone());
                        } else {
                            break;
                        }
                    }
                    before_ptr = p.checked_sub(1);
                }
                while let Some(o) = self.dequeue.get(after_ptr) {
                    let interval = o.interval.try_lock().unwrap();
                    if interval.chrom() == base_interval_locked.chrom() {
                        assert!(interval.start() >= base_interval_locked.stop());
                        let dist = interval.start() - base_interval_locked.stop();
                        if self.max_distance >= 0 && dist <= self.max_distance as u64 {
                            overlapping_positions.push(o.clone());
                            after_ptr += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
        drop(base_interval_locked);

        Some(Ok(Intersections {
            base_interval: Arc::clone(&base_interval),
            overlapping: overlapping_positions,
            cached_report: Arc::new(Mutex::new(None)),
        }))
    }
}

/// Create a new IntersectionIterator given a query (base) and a vector of other positioned iterators.
impl<'a> IntersectionIterator<'a> {
    pub fn new(
        base_iterator: Box<dyn PositionedIterator>,
        other_iterators: Vec<Box<dyn PositionedIterator>>,
        chromosome_order: &'a HashMap<String, Chromosome>,
        max_distance: i64,
        n_closest: i64,
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
            max_distance,
            n_closest,
        })
    }

    fn init_heap(&mut self, base_interval: Arc<Mutex<Position>>) -> io::Result<()> {
        assert!(!self.heap_initialized);
        for (i, iter) in self.other_iterators.iter_mut().enumerate() {
            if let Some(positioned) = iter.next_position(Some(
                &base_interval
                    .try_lock()
                    .expect("failed to lock base_interval"),
            )) {
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

    /// drop intervals from Q that are strictly before the base interval and more than max_distance away.
    /// does not consider n_closest as that is handled elsewhere.
    fn pop_front(&mut self, base_interval: &Position) {
        loop {
            let should_pop = if let Some(intersection) = self.dequeue.front() {
                let interval = intersection
                    .interval
                    .try_lock()
                    .expect("failed to lock interval");

                if interval.chrom() != base_interval.chrom() {
                    self.chromosome_order[interval.chrom()].index
                        < self.chromosome_order[base_interval.chrom()].index
                } else if interval.stop() < base_interval.start() {
                    let dist = base_interval.start() - interval.stop();
                    self.max_distance > 0 && dist > self.max_distance as u64
                } else {
                    false
                }
            } else {
                false
            };

            if should_pop {
                self.dequeue.pop_front();
            } else {
                break;
            }
        }
    }

    fn out_of_order(&self, interval: &Position) -> bool {
        match &self.previous_interval {
            None => false, // first interval in file.
            Some(previous_interval) => {
                let previous_interval = previous_interval
                    .try_lock()
                    .expect("failed to lock previous interval");
                if previous_interval.chrom() != interval.chrom() {
                    let pci = self.chromosome_order[previous_interval.chrom()].index;
                    let ici = self.chromosome_order[interval.chrom()].index;
                    pci > ici
                } else {
                    previous_interval.start() > interval.start()
                        || (previous_interval.start() == interval.start()
                            && previous_interval.stop() > interval.stop())
                }
            }
        }
    }
    // reset the array that tracks which iterators have been called with Some(Positioned)
    #[inline]
    fn zero_called(&mut self) {
        let ptr = self.called.as_mut_ptr();
        unsafe { ptr.write_bytes(0, self.called.len()) };
    }

    fn pull_through_heap(&mut self, base_interval: Arc<Mutex<Position>>) -> io::Result<()> {
        self.zero_called();
        if !self.heap_initialized {
            // we wait til first iteration here to call init heap
            // because we need the base interval.
            self.init_heap(base_interval.clone())?;
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
            let l = base_interval
                .try_lock()
                .expect("failed to lock base_interval");
            let arg: Option<&Position> = if !self.called[file_index] {
                self.called[file_index] = true;
                Some(&l)
            } else {
                None
            };
            /*
            log::warn!(
                "arg: {:?}, file_index: {} heap-len: {} q-len: {}",
                arg,
                file_index,
                self.min_heap.len(),
                self.dequeue.len()
            );
            */
            // IMPORTANT!
            // TODO: next_position is called with Some(interval) every time.
            // TODO: problem. if we query, e.g. chr1:1-1000, then we already pushed on the heap intervals
            // TODO: ... then we query e.g. chr1:900-1100 and it could appear as though we have intervals out of order.
            // TODO: need to get all intervals from the first query and then query from 1000 to 1100. and not take any intervals
            // TODO: ... that start before 1000.
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
                    chromosome_index: next_chromosome.index,
                    id: file_index,
                });
            }
            drop(l);

            // and we must always add the position to the Q
            let rc_pos = Arc::new(Mutex::new(position));
            let intersection = Intersection {
                interval: rc_pos.clone(),
                id: file_index as u32,
            };
            self.dequeue.push_back(intersection);

            // if this position is after base_interval, we can stop pulling through heap.
            {
                if cmp(
                    &base_interval
                        .try_lock()
                        .expect("failed to lock base_interval"),
                    &rc_pos.try_lock().expect("failed to lock interval"),
                    self.chromosome_order,
                ) == Ordering::Less
                {
                    break;
                }
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
                    .map(Position::Interval)
                    .collect::<Vec<Position>>(),
            }
        }
        fn add(&mut self, iv: Interval) {
            self.ivs.push(Position::Interval(iv));
        }
    }
    impl PositionedIterator for Intervals {
        fn name(&self) -> String {
            format!("{}:{}", self.name, self.i).into()
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

        b_ivs.ivs.sort_by_key(|a| a.start());

        //let a_ivs: Box<dyn PositionedIterator> = Box::new(a_ivs);

        let mut iter =
            IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order, 0, 0)
                .expect("error getting iterator");
        let mut n = 0;
        assert!(iter.all(|intersection| {
            let intersection = intersection.expect("error getting intersection");
            n += 1;
            assert!(intersection.overlapping.iter().all(|p| p
                .interval
                .try_lock()
                .expect("failed to lock interval")
                .start()
                == intersection
                    .base_interval
                    .try_lock()
                    .expect("failed to lock base_interval")
                    .start()));
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

        let iter =
            IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order, 0, 0)
                .expect("error getting iterator");
        iter.for_each(|intersection| {
            let intersection = intersection.expect("intersection");
            assert_eq!(intersection.overlapping.len(), 2);
            assert!(intersection.overlapping.iter().all(|p| {
                p.interval
                    .try_lock()
                    .expect("failed to lock interval")
                    .start()
                    == 0
            }));
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
        let mut iter = IntersectionIterator::new(Box::new(a_ivs), vec![], &chrom_order, 0, 0)
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
        let mut iter = IntersectionIterator::new(Box::new(a_ivs), vec![], &chrom_order, 0, 0)
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
            IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order, 0, 0)
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
            0,
            0,
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
        let iter =
            IntersectionIterator::new(Box::new(a_ivs), vec![Box::new(b_ivs)], &chrom_order, 0, 0)
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

    #[test]
    fn test_pop_front_with_distance() {
        let genome_str = "chr1\nchr2\nchr3\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();
        let a_ivs = Intervals::new(String::from("A"), vec![]);

        let max_distance = 50;
        let n_closest = 5;

        let mut iter = IntersectionIterator::new(
            Box::new(a_ivs),
            vec![],
            &chrom_order,
            max_distance,
            n_closest,
        )
        .expect("error getting iterator");

        // Scenario 1: same chromosome, check distance
        let intervals_to_add = vec![
            Interval {
                chrom: String::from("chr1"),
                start: 100,
                stop: 110,
                ..Default::default()
            }, // dist=90 > 50, pop
            Interval {
                chrom: String::from("chr1"),
                start: 149,
                stop: 150,
                ..Default::default()
            }, // dist=50 <= 50, keep
            Interval {
                chrom: String::from("chr1"),
                start: 160,
                stop: 170,
                ..Default::default()
            }, // dist=30 <= 50, keep
        ];
        for iv in intervals_to_add {
            iter.dequeue.push_back(Intersection {
                interval: Arc::new(Mutex::new(Position::Interval(iv))),
                id: 0,
            });
        }

        let base_interval1 = Position::Interval(Interval {
            chrom: String::from("chr1"),
            start: 200,
            stop: 201,
            ..Default::default()
        });
        iter.pop_front(&base_interval1);
        assert_eq!(iter.dequeue.len(), 2);
        let starts: Vec<u64> = iter
            .dequeue
            .iter()
            .map(|i| {
                i.interval
                    .try_lock()
                    .expect("failed to lock interval")
                    .start()
            })
            .collect();
        assert_eq!(starts, vec![149, 160]);

        // Scenario 2: different chromosomes
        iter.dequeue.clear();
        let intervals_to_add = vec![
            Interval {
                chrom: String::from("chr1"),
                start: 100,
                stop: 110,
                ..Default::default()
            }, // chr1 < chr2, pop
            Interval {
                chrom: String::from("chr2"),
                start: 1,
                stop: 5,
                ..Default::default()
            }, // same chrom, dist=5, keep
            Interval {
                chrom: String::from("chr3"),
                start: 1,
                stop: 5,
                ..Default::default()
            }, // chr3 > chr2, keep
        ];
        for iv in intervals_to_add {
            iter.dequeue.push_back(Intersection {
                interval: Arc::new(Mutex::new(Position::Interval(iv))),
                id: 0,
            });
        }
        let base_interval2 = Position::Interval(Interval {
            chrom: String::from("chr2"),
            start: 10,
            stop: 11,
            ..Default::default()
        });
        iter.pop_front(&base_interval2);
        assert_eq!(iter.dequeue.len(), 2);
        let chroms: Vec<String> = iter
            .dequeue
            .iter()
            .map(|i| {
                i.interval
                    .try_lock()
                    .expect("failed to lock interval")
                    .chrom()
                    .to_string()
            })
            .collect();
        let chroms_str: Vec<&str> = chroms.iter().map(|s| s.as_ref()).collect();
        assert_eq!(chroms_str, vec!["chr2", "chr3"]);
    }

    #[test]
    fn test_closest_and_distance() {
        let genome_str = "chr1\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();

        let base_ivs = Intervals::new(
            String::from("A"),
            vec![Interval {
                chrom: String::from("chr1"),
                start: 100,
                stop: 110,
                ..Default::default()
            }],
        );

        let db_ivs = Intervals::new(
            String::from("B"),
            vec![
                Interval {
                    chrom: String::from("chr1"),
                    start: 50,
                    stop: 60,
                    ..Default::default()
                }, // dist 40
                Interval {
                    chrom: String::from("chr1"),
                    start: 80,
                    stop: 90,
                    ..Default::default()
                }, // dist 10
                Interval {
                    chrom: String::from("chr1"),
                    start: 95,
                    stop: 105,
                    ..Default::default()
                }, // overlap 0
                Interval {
                    chrom: String::from("chr1"),
                    start: 108,
                    stop: 115,
                    ..Default::default()
                }, // overlap 0
                Interval {
                    chrom: String::from("chr1"),
                    start: 120,
                    stop: 130,
                    ..Default::default()
                }, // dist 10
                Interval {
                    chrom: String::from("chr1"),
                    start: 150,
                    stop: 160,
                    ..Default::default()
                }, // dist 40
                Interval {
                    chrom: String::from("chr1"),
                    start: 200,
                    stop: 210,
                    ..Default::default()
                }, // dist 90
            ],
        );

        let n_closest = 5;
        let max_distance = 50;

        let mut iter = IntersectionIterator::new(
            Box::new(base_ivs),
            vec![Box::new(db_ivs)],
            &chrom_order,
            max_distance,
            n_closest,
        )
        .expect("error getting iterator");

        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.overlapping.len(), 5);

        // check that the distances are as expected.
        let base_interval_locked = first.base_interval.try_lock().unwrap();
        let mut distances: Vec<u64> = first
            .overlapping
            .iter()
            .map(|o| {
                let interval = o.interval.try_lock().unwrap();
                if interval.stop() > base_interval_locked.start()
                    && base_interval_locked.stop() > interval.start()
                {
                    0
                } else if interval.stop() <= base_interval_locked.start() {
                    base_interval_locked.start() - interval.stop()
                } else {
                    interval.start() - base_interval_locked.stop()
                }
            })
            .collect();
        distances.sort();

        assert_eq!(distances, vec![0, 0, 10, 10, 40]);
    }

    struct ClosestTestCase {
        name: &'static str,
        base_ivs: Vec<Interval>,
        db_ivs: Vec<Interval>,
        n_closest: i64,
        max_distance: i64,
        expected_overlapping_count: usize,
        expected_distances: Vec<u64>,
    }
    #[test]
    fn test_closest_logic() {
        let genome_str = "chr1\nchr2\n";
        let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();

        let base_interval_spec = Interval {
            chrom: String::from("chr1"),
            start: 100,
            stop: 110,
            ..Default::default()
        };

        // base is chr1:100-110
        // distances from base: 40, 10, 0, 0, 10, 40, 90
        // sorted distances: 0, 0, 10, 10, 40, 40, 90
        let db_ivs_template = vec![
            Interval {
                chrom: String::from("chr1"),
                start: 50,
                stop: 60,
                ..Default::default()
            }, // dist 40
            Interval {
                chrom: String::from("chr1"),
                start: 80,
                stop: 90,
                ..Default::default()
            }, // dist 10
            Interval {
                chrom: String::from("chr1"),
                start: 95,
                stop: 105,
                ..Default::default()
            }, // overlap 0
            Interval {
                chrom: String::from("chr1"),
                start: 108,
                stop: 115,
                ..Default::default()
            }, // overlap 0
            Interval {
                chrom: String::from("chr1"),
                start: 120,
                stop: 130,
                ..Default::default()
            }, // dist 10
            Interval {
                chrom: String::from("chr1"),
                start: 150,
                stop: 160,
                ..Default::default()
            }, // dist 40
            Interval {
                chrom: String::from("chr1"),
                start: 200,
                stop: 210,
                ..Default::default()
            }, // dist 90
        ];

        let test_cases = vec![
            ClosestTestCase {
                name: "n_closest only, no max_distance",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: db_ivs_template.clone(),
                n_closest: 5,
                max_distance: 0,
                expected_overlapping_count: 5,
                expected_distances: vec![0, 0, 10, 10, 40],
            },
            ClosestTestCase {
                name: "max_distance only, no n_closest",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: db_ivs_template.clone(),
                n_closest: 0,
                max_distance: 30,
                expected_overlapping_count: 4,
                expected_distances: vec![0, 0, 10, 10],
            },
            ClosestTestCase {
                name: "n_closest and max_distance, n_closest is restrictive",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: db_ivs_template.clone(),
                n_closest: 3,
                max_distance: 50,
                expected_overlapping_count: 3,
                expected_distances: vec![0, 0, 10],
            },
            ClosestTestCase {
                name: "n_closest and max_distance, max_distance is restrictive",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: db_ivs_template.clone(),
                n_closest: 5,
                max_distance: 30,
                expected_overlapping_count: 4,
                expected_distances: vec![0, 0, 10, 10],
            },
            ClosestTestCase {
                name: "no overlapping intervals, n_closest",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: vec![
                    Interval {
                        chrom: String::from("chr1"),
                        start: 50,
                        stop: 60,
                        ..Default::default()
                    }, // dist 40
                    Interval {
                        chrom: String::from("chr1"),
                        start: 80,
                        stop: 90,
                        ..Default::default()
                    }, // dist 10
                ],
                n_closest: 1,
                max_distance: 0,
                expected_overlapping_count: 1,
                expected_distances: vec![10],
            },
            ClosestTestCase {
                name: "n_closest with different chromosome",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: vec![
                    Interval {
                        chrom: String::from("chr1"),
                        start: 80,
                        stop: 90,
                        ..Default::default()
                    }, // dist 10
                    Interval {
                        chrom: String::from("chr2"),
                        start: 80,
                        stop: 90,
                        ..Default::default()
                    }, // different chrom
                ],
                n_closest: 2,
                max_distance: 0,
                expected_overlapping_count: 1,
                expected_distances: vec![10],
            },
            ClosestTestCase {
                name: "max_distance with different chromosome",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: vec![
                    Interval {
                        chrom: String::from("chr1"),
                        start: 80,
                        stop: 90,
                        ..Default::default()
                    }, // dist 10
                    Interval {
                        chrom: String::from("chr2"),
                        start: 80,
                        stop: 90,
                        ..Default::default()
                    }, // different chrom
                ],
                n_closest: 0,
                max_distance: 100,
                expected_overlapping_count: 1,
                expected_distances: vec![10],
            },
            ClosestTestCase {
                name: "no matches for n_closest",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: vec![],
                n_closest: 5,
                max_distance: 0,
                expected_overlapping_count: 0,
                expected_distances: vec![],
            },
            ClosestTestCase {
                name: "no matches for max_distance",
                base_ivs: vec![base_interval_spec.clone()],
                db_ivs: vec![Interval {
                    chrom: String::from("chr1"),
                    start: 0,
                    stop: 1,
                    ..Default::default()
                }],
                n_closest: 0,
                max_distance: 10,
                expected_overlapping_count: 0,
                expected_distances: vec![],
            },
        ];

        for case in test_cases {
            let base_ivs = Intervals::new(String::from("A"), case.base_ivs);
            let db_ivs = Intervals::new(String::from("B"), case.db_ivs);

            let mut iter = IntersectionIterator::new(
                Box::new(base_ivs),
                vec![Box::new(db_ivs)],
                &chrom_order,
                case.max_distance,
                case.n_closest,
            )
            .expect("error getting iterator");

            let first = iter.next().unwrap().unwrap();
            assert_eq!(
                first.overlapping.len(),
                case.expected_overlapping_count,
                "failed test '{}': expected count {} but got {}",
                case.name,
                case.expected_overlapping_count,
                first.overlapping.len()
            );

            let base_interval_locked = first.base_interval.try_lock().unwrap();
            let mut distances: Vec<u64> = first
                .overlapping
                .iter()
                .map(|o| {
                    let interval = o.interval.try_lock().unwrap();
                    if interval.stop() > base_interval_locked.start()
                        && base_interval_locked.stop() > interval.start()
                    {
                        0
                    } else if interval.stop() <= base_interval_locked.start() {
                        base_interval_locked.start() - interval.stop()
                    } else {
                        interval.start() - base_interval_locked.stop()
                    }
                })
                .collect();
            distances.sort();
            assert_eq!(
                distances, case.expected_distances,
                "failed test '{}': incorrect distances",
                case.name
            );
        }
    }
}
