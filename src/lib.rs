//! Bedder is a library for intersecting genomic data.
#![deny(missing_docs)]

/// Intersection iterators and data structures.
pub mod intersection;

/// Position traits.
pub mod position;

// Interval type
pub mod interval;

/// a std::String::String unless other string features are enabled.
pub mod string;

pub mod sniff;

pub mod chrom_ordering;

#[cfg(feature = "bed")]
/// Bed parser implementing the PositionedIterator trait.
pub mod bedder_bed;

#[cfg(feature = "vcf")]
/// Vcf parser implementing the PositionedIterator trait.
pub mod bedder_vcf;
