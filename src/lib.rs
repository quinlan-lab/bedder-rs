//! Bedder is a library for intersecting genomic data.

/// Intersection iterators and data structures.
pub mod intersection;

/// Position traits.
pub mod position;

/// a std::String::String unless other string features are enabled.
pub mod string;

#[cfg(feature = "bed")]
/// Bed parser implementing the PositionedIterator trait.
pub mod bedder_bed;


#[cfg(feature = "vcf")]
/// Vcf parser implementing the PositionedIterator trait.
pub mod bedder_vcf;
