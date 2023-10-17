use crate::intersection::{Intersection, Intersections};
use crate::position::{Position, Positioned};
use bitflags::bitflags;

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

    fn set_start(&mut self, start: u64) {
        self.start = start;
    }
    fn set_stop(&mut self, stop: u64) {
        self.stop = stop;
    }
}

pub enum IntersectionOutput {
    /// Don't report the intersection.
    /// This is commonly used for -b to not report b intervals.
    None,
    /// Report each portion of A that overlaps B
    Part,
    /// Report the whole interval of A that overlaps B
    Whole,
}

bitflags! {
    /// IntersectionMode indicates requirements for the intersection.
    /// And extra fields that might be reported.
    pub struct IntersectionMode: u8 {
        // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary

        /// Default without extra requirements.
        const Empty = 0b00000000;

        /// Return A(B) if it does *not* overlap B(A). Bedtools -v
        const Not = 0b00000001;

        /// Report bases of overlap (-c)
        const Bases = 0b00000010;

        /// Report count of overlaps (-C)
        const Count = 0b00000100;
    }
}

impl Default for IntersectionMode {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug)]
pub enum OverlapAmount {
    Bases(u64),
    Fraction(f32),
}

impl Default for OverlapAmount {
    fn default() -> Self {
        Self::Bases(1)
    }
}

impl Intersections {
    pub fn intersections(
        &self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_output: IntersectionOutput,
        b_output: IntersectionOutput,
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
    ) -> Vec<SimpleInterval> {
        // now, given the arguments that determine what is reported (output)
        // and what is required (mode), we collect the intersections
        let mut results = Vec::new();
        let base = self.base_interval.clone();
        // iterate over the intersections and check the requirements
        self.overlapping.iter().for_each(|o| {
            let bases_overlapping =
                o.interval.stop().min(base.stop()) - o.interval.start().max(base.start());
        });
        results
    }
}

// write some tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::{ops::Deref, sync::Arc};

    #[test]
    fn test_simple() {
        // make a single Intersections
        let base = SimpleInterval {
            chrom: "chr1",
            start: 1,
            stop: 10,
        };
        let other = SimpleInterval {
            chrom: "chr1",
            start: 3,
            stop: 6,
        };
        let p = Position::Other(Box::new(base));
        let oi1 = Intersection {
            interval: Arc::new(Position::Other(Box::new(other))),
            id: 0,
        };
        let oi2 = Intersection {
            interval: Arc::new(Position::Other(Box::new(SimpleInterval {
                chrom: "chr1",
                start: 8,
                stop: 12,
            }))),
            id: 1,
        };
        let intersections = Intersections {
            base_interval: Arc::new(p),
            overlapping: vec![oi1, oi2],
        };

        intersections
            .overlapping
            .iter()
            .map(|o| o.interval.clone())
            .for_each(|i| {
                let overlap = i.stop().min(intersections.base_interval.stop())
                    - i.start().max(intersections.base_interval.start());
                println!("overlap: {:?}", overlap);
                println!("i: {:?}", i);
            });

        // clip a.start, end
        let interval = intersections.base_interval.clone();
        let mut pieces = vec![];
        intersections.overlapping.iter().for_each(|o| {
            let mut piece = *interval.as_ref();
            if piece.start() > o.interval.start() {
                piece.set_start(o.interval.start());
            }
            if piece.stop() > o.interval.stop() {
                piece.set_stop(o.interval.stop());
            }
            pieces.push(piece)
        });
    }
}
