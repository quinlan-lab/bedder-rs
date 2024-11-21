//! Bedder is a library for intersecting genomic data.

/// Intersection iterators and data structures.
pub mod intersection;

/// What to do with the intersections
pub mod intersections;

/// Position traits.
pub mod position;

// Interval type
pub mod interval;

/// Reports from intersections.
pub mod report;

/// a std::String::String unless other string features are enabled.
pub mod string;

pub mod chrom_ordering;

pub mod writer;

//#[cfg(feature = "bed")]
/// Bed parser implementing the PositionedIterator trait.
pub mod bedder_bed;

//#[cfg(feature = "vcf")]
/// Vcf parser implementing the PositionedIterator trait.
pub mod bedder_vcf;

mod tests;
