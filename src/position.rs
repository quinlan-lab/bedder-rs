use crate::string::String;
use std::io;
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

/// A Positioned has a position in the genome. It is a bed-like (half-open) interval.
pub trait Positioned {
    fn chrom(&self) -> &str;
    /// 0-based start position.
    fn start(&self) -> u64;
    /// non-inclusive end;
    fn stop(&self) -> u64;

    // extract a value from the Positioned object with a string key
    //fn value(&self, String) -> Value<'a>

    // extract a value from the Positioned object with an integer key. for a column.
    //fn ivalue(&self, usize) -> Value<'a>

    // get back the original line?
    //fn line(&self) -> &'a str;
}

pub trait PositionedIterator {
    type Item: Positioned;

    /// A name for the iterator. This is most often the file path, perhaps with the line number appended.
    /// Used to provide informative messages to the user.
    fn name(&self) -> String;

    /// return the next Positioned from the iterator.
    fn next_position(&mut self) -> Option<Result<Self::Item, io::Error>>;
}
