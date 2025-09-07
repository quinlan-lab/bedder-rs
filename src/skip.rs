use std::io;

pub trait Skip {
    fn skip_to(&mut self, chrom: &str, pos0: u64) -> io::Result<()>;
}
