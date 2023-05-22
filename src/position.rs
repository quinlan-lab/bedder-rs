use crate::string::String;
use core::fmt;
use std::io;

#[derive(Debug)]
pub enum Value {
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

pub enum Field {
    String(String),
    Int(usize),
}

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

pub type Result = std::result::Result<Value, FieldError>;

/// A Positioned has a position in the genome. It is a bed-like (half-open) interval.
pub trait Positioned {
    fn chrom(&self) -> &str;
    /// 0-based start position.
    fn start(&self) -> u64;
    /// non-inclusive end;
    fn stop(&self) -> u64;

    // extract a value from the Positioned object Col
    fn value(&self, b: Field) -> Result;

    // get back the original line?
    //fn line(&self) -> &'a str;
}

impl PartialEq for dyn Positioned {
    fn eq(&self, other: &dyn Positioned) -> bool {
        self.start() == other.start()
            && self.stop() == other.stop()
            && self.chrom() == other.chrom()
    }
}

pub trait PositionedIterator {
    type Item: Positioned;

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
        q: Option<&dyn Positioned>,
    ) -> Option<std::result::Result<Self::Item, io::Error>>;
}
