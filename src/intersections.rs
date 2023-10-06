use crate::intersection::{Intersection, Intersections};
use crate::position::{Position, Positioned};

/// Simple interval is used to return only the chrom, start, end.
#[derive(Debug)]
pub struct SimpleInterval<'a> {
    chrom: &'a str,
    start: u64,
    stop: u64,
}

impl<'a> Positioned for SimpleInterval<'a> {
    fn chrom(&self) -> &str {
        self.chrom
    }
    fn start(&self) -> u64 {
        self.start
    }
    fn stop(&self) -> u64 {
        self.stop
    }
}

/// IntersectionMode determines what part of overlap is returned.
/// The f32 is the proportion of overlap required.
pub enum IntersectionMode {
    // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary
    /// Return Chunks of A(B) that overlap B(A)
    Chunks(f32),
    /// Return All of A(B) that overlaps B(A)
    All(f32),
    /// Return A(B) if it does not overlap B(A)
    None(f32),
}

impl Intersections {
    pub fn intersections<'a>(
        &'a self,
        a_mode: Option<IntersectionMode>,
        b_mode: Option<IntersectionMode>,
        count: bool,
    ) -> Vec<SimpleInterval<'a>> {
        match (a_mode, b_mode) {
            (None, None) => self.a_chunks(None),
            (Some(a_fraction), None) => unimplemented!(),
            (None, Some(b_fraction)) => unimplemented!(),
            (Some(a_fraction), Some(b_fraction)) => unimplemented!(),
        }
    }

    pub fn a_chunks<'a>(&'a self, mode: Option<IntersectionMode>) -> Vec<SimpleInterval<'a>> {
        todo!("handle mode fraction of overlap");
        self.overlapping
            .iter()
            .map(|inter| {
                let start = inter.interval.start().max(self.base_interval.start());
                let stop = inter.interval.stop().min(self.base_interval.stop());
                SimpleInterval {
                    chrom: inter.interval.chrom(),
                    start: start,
                    stop: stop,
                }
            })
            .collect()
    }
}
