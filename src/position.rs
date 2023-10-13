use crate::string::String;
use std::fmt::{self, Debug};
use std::io;
use std::result;

/// A Value is a vector of integers, floats, or strings.
/// Often this will be a single value.
#[derive(Debug)]
pub enum Value {
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

/// Field is either an integer, as in a bed column
/// or a string, as in a vcf info field.
#[derive(Debug)]
pub enum Field {
    String(String),
    Int(usize),
}

/// Error returned when a field is not found.
#[derive(Debug)]
pub enum FieldError {
    InvalidFieldIndex(usize),
    InvalidFieldName(String),
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FieldError::InvalidFieldIndex(i) => write!(f, "invalid column index: {}", i),
            FieldError::InvalidFieldName(s) => write!(f, "invalid column name: {}", s),
        }
    }
}

impl std::error::Error for FieldError {}

/// A Positioned has a position in the genome. It is a bed-like (half-open) interval.
/// It also has a means to extract values from integer or string columns.
pub trait Positioned: Debug {
    fn chrom(&self) -> &str;
    /// 0-based start position.
    fn start(&self) -> u64;
    /// non-inclusive end;
    fn stop(&self) -> u64;

    // get back the original line?
    //fn line(&self) -> &'a str;
}

pub trait Valued {
    // extract a value from the Positioned object Col
    fn value(&self, b: Field) -> result::Result<Value, FieldError>;
}

#[derive(Debug)]
pub enum Position {
    Bed(crate::bedder_bed::Record<3>),
    // Note: we use a Box here because a vcf Record is large.
    Vcf(Box<crate::bedder_vcf::Record>),
    Interval(crate::interval::Interval),
    // catch-all in case we have another interval type.
    // #[cfg(feature = "dyn_positioned")]
    Other(Box<dyn Positioned>),
}

impl Position {
    #[inline]
    pub fn chrom(&self) -> &str {
        match self {
            Position::Bed(b) => b.chrom(),
            Position::Vcf(v) => v.chrom(),
            Position::Interval(i) => &i.chrom,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.chrom(),
        }
    }

    #[inline]
    pub fn start(&self) -> u64 {
        match self {
            Position::Bed(b) => b.start(),
            Position::Vcf(v) => v.start(),
            Position::Interval(i) => i.start,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.start(),
        }
    }

    #[inline]
    pub fn stop(&self) -> u64 {
        match self {
            Position::Bed(b) => b.stop(),
            Position::Vcf(v) => v.stop(),
            Position::Interval(i) => i.stop,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.stop(),
        }
    }
}

// Delegate the boxed version of this trait object to the inner object.
impl Positioned for Box<dyn Positioned> {
    fn chrom(&self) -> &str {
        self.as_ref().chrom()
    }

    fn start(&self) -> u64 {
        self.as_ref().start()
    }

    fn stop(&self) -> u64 {
        self.as_ref().stop()
    }
}

impl PartialEq for dyn Positioned {
    fn eq(&self, other: &dyn Positioned) -> bool {
        self.start() == other.start()
            && self.stop() == other.stop()
            && self.chrom() == other.chrom()
    }
}

/// PositionedIterator is an iterator over Positioned objects.
pub trait PositionedIterator {
    /// A name for the iterator. This is most often the file path, perhaps with the line number appended.
    /// Used to provide informative messages to the user.
    fn name(&self) -> String;

    /// return the next Positioned from the iterator.
    /// It is fine for implementers to ignore `q`;
    /// Some iterators may improve performance by using `q` to index-skip.
    /// `q` will be Some only on the first call for a given query interval.
    /// Calls where `q` is None should return the next Positioned in the iterator (file) that has not
    /// been returned previously. Intervals should only be returned once (even across many identical query intervals)
    /// and they should always be returned in order (querys will always be in order).
    /// Thus, if the implementer heeds `q` it should check that the returned Positioned is greater than the previously
    /// returned position (Positioned equal to previously returned position should have already been returned).
    fn next_position(
        &mut self,
        q: Option<&Position>,
    ) -> Option<std::result::Result<Position, io::Error>>;
}
