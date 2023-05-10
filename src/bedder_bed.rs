use crate::string::String;
pub use noodles::bed;
use std::io;
use std::io::BufRead;
use std::rc::Rc;

impl crate::position::Positioned for bed::record::Record<3> {
    fn chrom(&self) -> &str {
        self.reference_sequence_name()
    }

    fn start(&self) -> u64 {
        // noodles position is 1-based.
        self.start_position().get() as u64 - 1
    }

    fn stop(&self) -> u64 {
        self.end_position().get() as u64
    }
}

pub struct BedderBed<R>
where
    R: BufRead,
{
    reader: bed::Reader<R>,
    buf: crate::string::String,
    last_record: Option<Rc<bed::record::Record<3>>>,
    line_number: u64,
}

impl<R> crate::position::PositionedIterator for BedderBed<R>
where
    R: BufRead,
{
    type Item = bed::record::Record<3>;

    fn next_position(
        &mut self,
        _q: Option<&dyn crate::position::Positioned>,
    ) -> Option<Result<Self::Item, std::io::Error>> {
        self.buf.clear();
        loop {
            self.line_number += 1;
            return match self.reader.read_line(&mut self.buf) {
                Ok(0) => None,
                Ok(_) => {
                    if self.buf.starts_with('#') {
                        continue;
                    }
                    let record = self
                        .buf
                        .parse()
                        .map_err(|e| return io::Error::new(io::ErrorKind::InvalidData, e));
                    //self.last_record = Some(Rc::new(record.unwrap()));
                    Some(record)
                }
                Err(e) => Some(Err(e)),
            };
        }
    }

    fn name(&self) -> String {
        String::from(format!("bed:{}", self.line_number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{intersection::IntersectionIterator, *};
    use std::collections::HashMap;
    use std::io::Cursor;

    #[test]
    fn test_bed_read() {
        // write a test for bed from a string using BufRead
        let a = bed::Reader::new(Cursor::new("chr1\t20\t30\nchr1\t21\t33"));
        let b = bed::Reader::new(Cursor::new("chr1\t21\t30\nchr1\t22\t33"));

        let ar = BedderBed {
            reader: a,
            buf: String::new(),
            last_record: None,
            line_number: 0,
        };

        let br = BedderBed {
            reader: b,
            buf: String::new(),
            last_record: None,
            line_number: 0,
        };
        let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);

        let it =
            IntersectionIterator::new(ar, vec![br], &chrom_order).expect("error creating iterator");

        let mut n = 0;
        it.for_each(|int| {
            let int = int.expect("error getting intersection");
            //dbg!(&int.overlapping);
            assert!(int.overlapping.len() == 2);
            n += 1;
        });
        assert!(n == 2);
    }
}
