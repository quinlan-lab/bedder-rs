use crate::position::Position;

#[derive(Debug, Clone)]
pub struct ReportFragment {
    pub a: Option<Position>,
    pub b: Vec<Position>,
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

/// implement Indexing for Report to get each fragment
impl std::ops::Index<usize> for Report {
    type Output = ReportFragment;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Report {
    /// Create a new report from a vector of fragments.
    pub fn new(frags: Vec<ReportFragment>) -> Self {
        Self(frags)
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
                .map(|pos| pos.stop() - pos.start())
                .sum::<u64>();
        });
        result
    }
}
