use crate::string::String;
use std::fmt::{self, Debug};
use std::io;
use std::result;

/// A Value is a vector of integers, floats, or strings.
/// Often this will be a single value.
#[derive(Debug, Clone)]
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
    InvalidFieldValue(String),
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FieldError::InvalidFieldIndex(i) => write!(f, "invalid column index: {}", i),
            FieldError::InvalidFieldName(s) => write!(f, "invalid column name: {}", s),
            FieldError::InvalidFieldValue(s) => write!(f, "invalid field value: {}", s),
        }
    }
}

impl std::error::Error for FieldError {}

/// A Positioned has a position in the genome. It is a bed-like (half-open) interval.
/// It also has a means to extract values from integer or string columns.
pub trait Positioned: Debug + Sync + Send {
    fn chrom(&self) -> &str;

    /// 0-based start position.
    fn start(&self) -> u64;

    /// non-inclusive end.
    fn stop(&self) -> u64;

    /// set the start position.
    fn set_start(&mut self, start: u64);

    /// set the stop position.
    fn set_stop(&mut self, start: u64);

    // get back the original line?
    //fn line(&self) -> &'a str;

    // return a copy of the Positioned object.
    //fn dup(&self) -> Box<dyn Positioned>;

    /// Return a Box<dyn Positioned>
    fn clone_box(&self) -> Box<dyn Positioned>
    where
        Self: Sized;
}

pub trait Valued {
    // extract a value from the Positioned object Col
    fn value(&self, b: Field) -> result::Result<Value, FieldError>;
}

#[derive(Debug)]
pub enum Position {
    Bed(crate::bedder_bed::BedRecord),
    // Note: we use a Box here because a vcf Record is large.
    Vcf(Box<crate::bedder_vcf::BedderRecord>),
    Interval(crate::interval::Interval),
    // catch-all in case we have another interval type.
    // #[cfg(feature = "dyn_positioned")]
    Other(Box<dyn Positioned>),
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        return self.chrom() == other.chrom()
            && self.start() == other.start()
            && self.stop() == other.stop();
    }
}

impl Position {
    #[inline]
    pub fn chrom(&self) -> &str {
        match self {
            Position::Bed(b) => b.chrom(),
            Position::Vcf(v) => {
                let rid = match v.record.rid() {
                    Some(rid) => rid,
                    None => return "*",
                };
                let header = v.record.header();
                let name = header.rid2name(rid).unwrap();
                std::str::from_utf8(name).unwrap()
            }
            Position::Interval(i) => &i.chrom,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.chrom(),
        }
    }

    #[inline]
    /// 0-based start position.
    pub fn start(&self) -> u64 {
        match self {
            Position::Bed(b) => b.start(),
            Position::Vcf(v) => v.record.pos() as u64,
            Position::Interval(i) => i.start,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.start(),
        }
    }

    #[inline]
    pub fn stop(&self) -> u64 {
        match self {
            Position::Bed(b) => b.stop(),
            Position::Vcf(v) => v.record.end() as u64,
            Position::Interval(i) => i.stop,
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.stop(),
        }
    }

    pub fn set_start(&mut self, start: u64) {
        match self {
            Position::Bed(b) => b.set_start(start),
            Position::Vcf(v) => v.record.set_pos(start as i64),
            Position::Interval(i) => i.set_start(start),
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.set_start(start),
        }
    }

    pub fn set_stop(&mut self, stop: u64) {
        match self {
            Position::Bed(b) => b.set_stop(stop),
            Position::Vcf(_v) => {}
            Position::Interval(i) => i.set_stop(stop),
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(o) => o.set_stop(stop),
        }
    }

    pub fn clone_box(&self) -> Position {
        match self {
            Position::Bed(b) => Position::Bed(b.to_owned()),
            Position::Vcf(v) => Position::Vcf(Box::new((**v).clone())),
            Position::Interval(i) => Position::Interval(i.dup()),
            // #[cfg(feature = "dyn_positioned")]
            Position::Other(_o) => unimplemented!("TODO: clone Box<dyn Positioned>"),
        }
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
