//! Bedder is a library for intersecting genomic data.

/// Intersection iterators and data structures.
pub mod intersection;

/// What to do with the intersections
pub mod intersections;

/// Position traits.
pub mod position;

// Interval type
pub mod interval;

/// Open files and infer file format.
pub mod sniff;

/// Reports from intersections.
pub mod report;
pub mod report_options;

/// a std::String::String unless other string features are enabled.
pub mod string;

pub mod chrom_ordering;

// Determines how the output is written--format, compression, etc.
pub mod writer;

//#[cfg(feature = "bed")]
/// Bed parser implementing the PositionedIterator trait.
pub mod bedder_bed;

pub use rust_htslib::htslib as hts;

//#[cfg(feature = "vcf")]
/// Vcf parser implementing the PositionedIterator trait.
pub mod bedder_vcf;

mod tests;

pub mod hts_format;

/// Python bindings for bedder
pub mod py;

/// Lua bindings for bedder
pub mod lua_wrapper;

/// Column reporters for bedder
pub mod column;

#[cfg(test)]
mod py_test;
