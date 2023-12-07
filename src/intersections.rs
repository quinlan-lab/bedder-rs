use crate::intersection::Intersections;
use crate::position::Position;
#[allow(unused_imports)]
use crate::string::String;
use bitflags::bitflags;

/// IntersectionOutput indicates what to report for the intersection.
pub enum IntersectionOutput {
    /// Don't report the intersection.
    /// This is commonly used for -b to not report b intervals.
    None,
    /// Report each portion of A that overlaps B
    Part,
    /// Report the whole interval of A that overlaps B
    Whole,
    /// Report Unique As even if multiple Bs overlap
    Unique,
}

bitflags! {
    /// IntersectionMode indicates requirements for the intersection.
    /// And extra fields that might be reported.
    #[derive(Eq, PartialEq, Debug)]
    pub struct IntersectionMode: u8 {
        // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary

        /// Default without extra requirements.
        const Default = 0b00000000;

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
        Self::Default
    }
}

/// OverlapAmount indicates the amount of overlap required.
/// Either as bases or as a fraction of the total length.
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

impl OverlapAmount {
    /// sufficient_bases returns true if the bases overlap is sufficient
    /// relative to the total length.
    pub fn sufficient_bases(&self, bases: u64, total_len: u64) -> bool {
        match self {
            OverlapAmount::Bases(b) => bases >= *b,
            OverlapAmount::Fraction(f) => bases as f64 / total_len as f64 >= *f as f64,
        }
    }
}

impl Intersections {
    /// Given the intersection mode and requirements, return a vector of (a, b) tuples.
    /// The a and b positions are optional, depending on the intersection mode.
    pub fn report(
        &self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_output: IntersectionOutput,
        b_output: IntersectionOutput,
        // Q: overlap fraction is across all overlapping -b intervals?
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
        // TODO: should the 2nd of tuple be Option<Vec<Position>> ?
    ) -> Vec<(Option<Position>, Option<Position>)> {
        // now, given the arguments that determine what is reported (output)
        // and what is required (mode), we collect the intersections
        let mut results = Vec::new();
        let base = self.base_interval.clone();
        // iterate over the intersections and check the requirements
        let bases: u64 = self
            .overlapping
            .iter()
            .map(|o| o.interval.stop().min(base.stop()) - o.interval.start().max(base.start()))
            .sum();
        let a_total = base.stop() - base.start();
        if !a_requirements.sufficient_bases(bases, a_total) {
            // here handle Not (-v)
            if matches!(a_mode, IntersectionMode::Not) {
                results.push((Some(base.as_ref().dup()), None));
            }
            return results;
        }
        let b_total = self
            .overlapping
            .iter()
            .map(|o| o.interval.stop() - o.interval.start())
            .sum();
        if !b_requirements.sufficient_bases(bases, b_total) {
            if matches!(b_mode, IntersectionMode::Not) {
                // TODO: what goes here?
                results.push((Some(base.as_ref().dup()), None));
            }
            return results;
        }

        // TODO: here we add just the a pieces. Need to check how to add the b pieces.
        for o in self.overlapping.iter() {
            match a_output {
                IntersectionOutput::Part => {
                    let mut piece: Position = base.as_ref().dup();
                    piece.set_start(o.interval.start().max(piece.start()));
                    piece.set_stop(o.interval.stop().min(piece.stop()));
                    //pieces.push(piece);
                    results.push((Some(piece.dup()), None))
                }
                IntersectionOutput::Whole => results.push((Some(base.as_ref().dup()), None)),
                IntersectionOutput::Unique => {
                    results.push((Some(base.as_ref().dup()), None));
                    break;
                }
                IntersectionOutput::None => {}
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interval::Interval;
    use crate::tests::parse_intersections::parse_intersections;

    fn make_example(def: &str) -> Intersections {
        parse_intersections(def)
    }

    #[test]
    fn test_simple_unique() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Default,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            OverlapAmount::Bases(5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
        assert_eq!(
            r[0],
            (
                Some(Position::Interval(Interval {
                    chrom: String::from("chr1"),
                    start: 1,
                    stop: 10,
                    fields: Default::default(),
                })),
                None,
            ),
        );
    }
    #[test]
    fn test_simple_unique_insufficient_bases() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        // a: 1-10
        // b: 3-6, 8-12
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            // note we require 6 bases of overlap but only have 5
            OverlapAmount::Bases(6),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 0);
    }
    #[test]
    fn test_simple_unique_fraction() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        // a: 1-10
        // b: 3-6, 8-12
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            OverlapAmount::Fraction(0.6),
            OverlapAmount::Bases(1),
        );
        // 5 bases of overlap is 0.5555 of the total 9 bases
        assert_eq!(r.len(), 0);

        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            OverlapAmount::Fraction(0.5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn test_simple_whole() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        // a: 1-10
        // b: 3-6, 8-12
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionOutput::Whole,
            IntersectionOutput::None,
            OverlapAmount::Bases(1),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2);
        for v in r {
            assert_eq!(
                v,
                (
                    Some(Position::Interval(Interval {
                        chrom: String::from("chr1"),
                        start: 1,
                        stop: 10,
                        fields: Default::default(),
                    })),
                    None,
                ),
            );
        }
    }

    #[test]
    fn test_simple_parts() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        // a: 1-10
        // b: 3-6, 8-12
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionOutput::Part,
            IntersectionOutput::None,
            OverlapAmount::Bases(1),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            (
                Some(Position::Interval(Interval {
                    chrom: String::from("chr1"),
                    start: 3,
                    stop: 6,
                    fields: Default::default(),
                })),
                None,
            ),
        );
        assert_eq!(
            r[1],
            (
                Some(Position::Interval(Interval {
                    chrom: String::from("chr1"),
                    start: 8,
                    stop: 10,
                    fields: Default::default(),
                })),
                None,
            ),
        );
    }

    #[test]
    fn test_sufficient_bases_with_bases() {
        let overlap = OverlapAmount::Bases(10);
        let bases = 15;
        let total_len = 100;
        assert!(overlap.sufficient_bases(bases, total_len));
    }

    #[test]
    fn test_sufficient_bases_with_fraction() {
        let overlap = OverlapAmount::Fraction(0.5);
        let bases = 50;
        let total_len = 100;
        assert!(overlap.sufficient_bases(bases, total_len));
        assert!(!overlap.sufficient_bases(bases - 1, total_len));
    }

    #[test]
    fn test_no_overlaps() {
        let intersections = make_example("a: 1-10");
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Default,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            OverlapAmount::Bases(5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 0);

        // check Not. should return A since there are no overlaps
        let intersections = make_example("a: 1-10");
        let r = intersections.report(
            IntersectionMode::Not,
            IntersectionMode::Default,
            IntersectionOutput::Unique,
            IntersectionOutput::None,
            OverlapAmount::Bases(5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
    }
}
