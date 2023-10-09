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
    pub fn intersections<'a>(
        &'a self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_output: IntersectionOutput,
        b_output: IntersectionOutput,
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
    ) -> Vec<SimpleInterval<'a>> {
        unimplemented!("Intersections::intersections")
    }
}
