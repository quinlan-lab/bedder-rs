pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    Strings(Vec<String>),
}

pub trait Positioned<'a> {
    fn position(&self) -> Position<'a>;

    // extract a value from the Positioned object with a string key
    //fn value(&self, String) -> Value<'a>

    // extract a value from the Positioned object with an integer key. for a column.
    //fn ivalue(&self, usize) -> Value<'a>

    // get back the original line?
    //fn line(&self) -> &'a str;
}

#[derive(Debug)]
pub struct Position<'a> {
    pub chromosome: &'a str,
    pub start: u64,
    pub stop: u64,
}

impl<'a> Position<'a> {
    pub fn new(chromosome: &'a str, start: u64, stop: u64) -> Self {
        Position {
            chromosome,
            start,
            stop,
        }
    }
}

pub trait PositionedIterator<'a> {
    type Item: Positioned<'a>;

    fn next(&'a mut self) -> Option<Self::Item>;
}
