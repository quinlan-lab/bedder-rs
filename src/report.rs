use crate::position::Position;
use parking_lot::Mutex;
use std::sync::Arc;
#[derive(Debug, Clone)]
pub struct ReportFragment {
    pub a: Option<Arc<Mutex<Position>>>,
    pub b: Vec<Arc<Mutex<Position>>>,
    // id is the file index of the source
    pub id: usize,
}

#[derive(Debug, Clone)]
pub struct Report(Vec<ReportFragment>);

/// implement Iteration for Report to get each fragment
impl<'a> IntoIterator for &'a Report {
    type Item = &'a ReportFragment;
    type IntoIter = std::slice::Iter<'a, ReportFragment>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// implement mutable Iteration for Report to get each fragment
impl<'a> IntoIterator for &'a mut Report {
    type Item = &'a mut ReportFragment;
    type IntoIter = std::slice::IterMut<'a, ReportFragment>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

/// implement Indexing for Report to get each fragment
impl std::ops::Index<usize> for Report {
    type Output = ReportFragment;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for Report {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Report {
    /// Create a new report from a vector of fragments.
    pub fn new(frags: Vec<ReportFragment>) -> Self {
        Self(frags)
    }

    /// Get an iterator over the fragments.
    pub fn iter(&self) -> std::slice::Iter<'_, ReportFragment> {
        self.0.iter()
    }

    /// Get a mutable iterator over the fragments.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, ReportFragment> {
        self.0.iter_mut()
    }

    /// The number of fragments in the report.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is the report empty?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of overlaps from each source(id)
    pub fn count_overlaps_by_id(&self) -> Vec<u64> {
        if self.0.is_empty() {
            return vec![];
        }
        let mut result = vec![0; 1];
        self.0.iter().for_each(|frag| {
            if frag.id >= result.len() {
                result.resize(frag.id + 1, 0);
            }
            result[frag.id] += frag.b.len() as u64;
        });
        result
    }

    /// The number of b-bases in each fragment from each source(id)
    /// This is determined by the overlap requirements, modes, and parts.
    pub fn count_bases_by_id(&self) -> Vec<u64> {
        if self.0.is_empty() {
            return vec![];
        }
        let mut result = vec![0; 1];
        self.0.iter().for_each(|frag| {
            if frag.id >= result.len() {
                result.resize(frag.id + 1, 0);
            }
            result[frag.id] += frag
                .b
                .iter()
                .map(|pos| {
                    let p = pos.lock();
                    p.stop() - p.start()
                })
                .sum::<u64>();
        });
        result
    }
}

impl ReportFragment {
    pub fn distance(&self) -> u64 {
        if let Some(a) = &self.a {
            log::info!("a: {:?}", self);
            let a = a.try_lock().expect("failed to lock a interval in distance");
            if self.b.is_empty() {
                return u64::MAX;
            }

            // Find the minimum distance to any b interval
            let min_distance = self
                .b
                .iter()
                .map(|b| {
                    let b = b.try_lock().expect("failed to lock b interval in distance");

                    // Check if intervals overlap
                    if a.start() < b.stop() && b.start() < a.stop() {
                        return 0; // Overlapping intervals have distance 0
                    }

                    // Calculate gap between non-overlapping intervals
                    if a.start() >= b.stop() {
                        // a is downstream of b
                        a.start() - b.stop()
                    } else {
                        // b is downstream of a
                        b.start() - a.stop()
                    }
                })
                .min()
                .unwrap(); // Safe because we checked is_empty() above

            min_distance
        } else {
            u64::MAX
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interval::Interval;
    use crate::position::Position;
    use linear_map::LinearMap;

    fn make_pos(start: u64, stop: u64) -> Arc<Mutex<Position>> {
        Arc::new(Mutex::new(Position::Interval(Interval {
            chrom: "chr1".into(),
            start,
            stop,
            fields: LinearMap::new(),
        })))
    }

    #[test]
    fn test_distance_no_a() {
        let frag = ReportFragment {
            a: None,
            b: vec![make_pos(10, 20)],
            id: 0,
        };
        assert_eq!(frag.distance(), u64::MAX);
    }

    #[test]
    fn test_distance_downstream() {
        let frag = ReportFragment {
            a: Some(make_pos(100, 110)),
            b: vec![make_pos(10, 20)],
            id: 0,
        };
        // a.start() - b.stop() = 100 - 20 = 80
        assert_eq!(frag.distance(), 80);
    }

    #[test]
    fn test_distance_downstream_multiple_b() {
        // New logic finds the minimum distance to any b interval
        let frag = ReportFragment {
            a: Some(make_pos(100, 110)),
            b: vec![make_pos(10, 20), make_pos(5, 15)],
            id: 0,
        };
        // Minimum distance is to (10, 20): a.start() - b.stop() = 100 - 20 = 80
        assert_eq!(frag.distance(), 80);
    }

    #[test]
    fn test_distance_overlapping() {
        let frag = ReportFragment {
            a: Some(make_pos(100, 110)),
            b: vec![make_pos(90, 105)],
            id: 0,
        };
        // a.start() (100) < b.stop() (105), so saturating_sub is 0.
        assert_eq!(frag.distance(), 0);
    }

    #[test]
    fn test_distance_touching() {
        let frag = ReportFragment {
            a: Some(make_pos(100, 110)),
            b: vec![make_pos(90, 100)],
            id: 0,
        };
        assert_eq!(frag.distance(), 0);
    }

    #[test]
    fn test_distance_upstream() {
        // Test case where b is upstream of a (b comes after a)
        let frag = ReportFragment {
            a: Some(make_pos(10, 20)),
            b: vec![make_pos(100, 110)],
            id: 0,
        };
        // b.start() - a.stop() = 100 - 20 = 80
        assert_eq!(frag.distance(), 80);
    }

    #[test]
    fn test_distance_empty_b() {
        let frag = ReportFragment {
            a: Some(make_pos(100, 110)),
            b: vec![],
            id: 0,
        };
        assert_eq!(frag.distance(), u64::MAX);
    }
}
