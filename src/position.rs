pub trait Positioned<'a> {
    fn position(&self) -> Position<'a>;
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

pub trait PositionedIterator<'a, 'b> {
    type Item: Positioned<'a>;

    fn next(&'b mut self) -> Option<Self::Item>;
}
