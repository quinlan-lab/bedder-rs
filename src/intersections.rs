use crate::intersection::Intersections;
use crate::position::Position;
use bitflags::bitflags;

pub enum IntersectionOutput {
    /// Don't report the intersection.
    /// This is commonly used for -b to not report b intervals.
    None,
    /// Report each portion of A that overlaps B
    Part,
    /// Report the whole interval of A that overlaps B
    Whole,
}

bitflags! {
    /// IntersectionMode indicates requirements for the intersection.
    /// And extra fields that might be reported.
    pub struct IntersectionMode: u8 {
        // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary

        /// Default without extra requirements.
        const Empty = 0b00000000;

        /// Return A(B) if it does *not* overlap B(A). Bedtools -v
        const Not = 0b00000001;

        /// Report bases of overlap (-c)
        const Bases = 0b00000010;

        /// Report count of overlaps (-C)
        const Count = 0b00000100;
    }
}

impl Default for IntersectionMode {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug)]
pub enum OverlapAmount {
    Bases(u64),
    Fraction(f32),
}

impl Default for OverlapAmount {
    fn default() -> Self {
        Self::Bases(1)
    }
}

impl Intersections {
    pub fn intersections(
        &self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_output: IntersectionOutput,
        b_output: IntersectionOutput,
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
    ) -> Vec<Position> {
        // now, given the arguments that determine what is reported (output)
        // and what is required (mode), we collect the intersections
        let mut results = Vec::new();
        let base = self.base_interval.clone();
        // iterate over the intersections and check the requirements
        self.overlapping.iter().for_each(|o| {
            let bases_overlapping =
                o.interval.stop().min(base.stop()) - o.interval.start().max(base.start());
        });
        results
    }
}

// write some tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::intersection::Intersection;
    use crate::interval::Interval;
    use std::sync::Arc;

    #[test]
    fn test_simple() {
        // make a single Intersections
        let base = Interval {
            chrom: String::from("chr1"),
            start: 1,
            stop: 10,
            fields: Default::default(),
        };
        let other = Interval {
            chrom: String::from("chr1"),
            start: 3,
            stop: 6,
            fields: Default::default(),
        };
        let p = Position::Interval(base);
        let oi1 = Intersection {
            interval: Arc::new(Position::Interval(other)),
            id: 0,
        };
        let oi2 = Intersection {
            interval: Arc::new(Position::Interval(Interval {
                chrom: String::from("chr1"),
                start: 8,
                stop: 12,
                fields: Default::default(),
            })),
            id: 1,
        };
        let intersections = Intersections {
            base_interval: Arc::new(p),
            overlapping: vec![oi1, oi2],
        };

        intersections
            .overlapping
            .iter()
            .map(|o| o.interval.clone())
            .for_each(|i| {
                let overlap = i.stop().min(intersections.base_interval.stop())
                    - i.start().max(intersections.base_interval.start());
                println!("overlap: {:?}", overlap);
                println!("i: {:?}", i);
            });

        // clip a.start, end
        let interval = intersections.base_interval.clone();
        eprintln!("overlapping: {:?}", intersections.overlapping);
        let mut pieces = vec![];
        intersections.overlapping.iter().for_each(|o| {
            let mut piece: Position = interval.as_ref().dup();
            piece.set_start(o.interval.start().max(piece.start()));
            piece.set_stop(o.interval.stop().min(piece.stop()));
            pieces.push(piece)
        });
        eprintln!("pieces: {:?}", pieces);
    }
}
