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
    fn chrom(&self) -> &str;
    fn start(&self) -> u64;
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

    fn next(&mut self) -> Option<Self::Item>;
}
