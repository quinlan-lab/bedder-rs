use smartstring::alias::String;
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

pub trait Positioned {
    fn position(&self) -> Position;

    // extract a value from the Positioned object with a string key
    //fn value(&self, String) -> Value<'a>

    // extract a value from the Positioned object with an integer key. for a column.
    //fn ivalue(&self, usize) -> Value<'a>

    // get back the original line?
    //fn line(&self) -> &'a str;
}

#[derive(Debug)]
pub struct Position {
    pub chromosome: String,
    pub start: u64,
    pub stop: u64,
}

impl Position {
    pub fn new(chromosome: String, start: u64, stop: u64) -> Self {
        Position {
            chromosome,
            start,
            stop,
        }
    }
}

pub trait PositionedIterator {
    type Item: Positioned;

    fn next(&mut self) -> Option<Self::Item>;
}
