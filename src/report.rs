use crate::position::Position;

#[derive(Debug)]
pub struct ReportFragment {
    pub a: Option<Position>,
    pub b: Vec<Position>,
    pub id: usize,
}

#[derive(Debug)]
pub struct Report(Vec<ReportFragment>);

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
        let mut result = Vec::new();
        self.0.iter().for_each(|frag| {
            if frag.id >= result.len() {
                result.resize(frag.id + 1, 0);
            }
            result[frag.id] += frag.b.len() as u64;
        });
        result
    }
}
