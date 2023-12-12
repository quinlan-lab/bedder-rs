use crate::intersection::Intersections;
use crate::position::Position;
#[allow(unused_imports)]
use crate::string::String;
use bitflags::bitflags;

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

        const PerPiece = 0b00000010;

    }
}

/// IntersectionPart indicates what to report for the intersection.
pub enum IntersectionPart {
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

pub enum OverlapSufficient {
    Bool(bool),
    Which(Vec<usize>),
}

impl OverlapAmount {
    /// sufficient returns the bases overlap is sufficient
    /// relative to the total length.
    /// If per_piece, then return the indices of the pieces that are sufficient.
    /// If invert, then return the opposite of the sufficient test.
    pub fn sufficient(
        &self,
        bases: &[u64],
        total_len: u64,
        per_piece: bool,
        invert: bool,
    ) -> OverlapSufficient {
        // NOTE we use ^ to flip based on the invert flag which simulates -v in bedtools.
        if per_piece {
            // return the indices of the pieces that are sufficient
            OverlapSufficient::Which(
                bases
                    .iter()
                    .enumerate()
                    .filter(|(_, &b)| match self {
                        OverlapAmount::Bases(n_bases) => (b >= *n_bases) ^ invert,
                        OverlapAmount::Fraction(f) => {
                            (b as f64 / total_len as f64 >= *f as f64) ^ invert
                        }
                    })
                    .map(|(i, _)| i)
                    .collect(),
            )
        } else {
            // return whether the total bases is sufficient
            let bases: u64 = bases.iter().sum();
            OverlapSufficient::Bool(match self {
                OverlapAmount::Bases(b) => (bases >= *b) ^ invert,
                OverlapAmount::Fraction(f) => {
                    (bases as f64 / total_len as f64 >= *f as f64) ^ invert
                }
            })
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
        a_part: IntersectionPart,
        b_part: IntersectionPart,
        // Q: overlap fraction is across all overlapping -b intervals?
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
        // TODO: should the 2nd of tuple be Option<Vec<Position>> ?
        // or Vec<Option<Position>> ? where each i is for the ith -b file?
    ) -> Vec<(Option<Position>, Vec<Vec<Position>>)> {
        // now, given the arguments that determine what is reported (output)
        // and what is required (mode), we collect the intersections
        let mut results = Vec::new();
        let base = self.base_interval.clone();
        // iterate over the intersections and check the requirements
        // TODO: do this without allocating.
        let bases: Vec<u64> = self
            .overlapping
            .iter()
            .map(|o| o.interval.stop().min(base.stop()) - o.interval.start().max(base.start()))
            .collect();
        let a_total = base.stop() - base.start();
        // problem: we probably want to move everything into sufficient so that it returns
        // somehow the a and b pieces that we need.
        // so it can accept IntersectionMode.
        // but then it also needs to accept a_requirements *and* b_requirements.
        // how can we break this down?
        let a_suff = a_requirements.sufficient(
            &bases,
            a_total,
            a_mode.contains(IntersectionMode::PerPiece),
            a_mode.contains(IntersectionMode::Not),
        );
        if a_suff {
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
        if !b_requirements.sufficient(&bases, b_total, b_mode.contains(IntersectionMode::PerPiece))
        {
            if matches!(b_mode, IntersectionMode::Not) {
                // TODO: what goes here?
                results.push((Some(base.as_ref().dup()), None));
            }
            return results;
        }

        // TODO: here we add just the a pieces. Need to check how to add the b pieces.
        for o in self.overlapping.iter() {
            match a_part {
                IntersectionPart::Part => {
                    let mut piece: Position = base.as_ref().dup();
                    piece.set_start(o.interval.start().max(piece.start()));
                    piece.set_stop(o.interval.stop().min(piece.stop()));
                    //pieces.push(piece);
                    results.push((Some(piece.dup()), None))
                }
                IntersectionPart::Whole => results.push((Some(base.as_ref().dup()), None)),
                IntersectionPart::Unique => {
                    results.push((Some(base.as_ref().dup()), None));
                    break;
                }
                IntersectionPart::None => {}
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

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
            IntersectionPart::Unique,
            IntersectionPart::None,
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
            IntersectionPart::Unique,
            IntersectionPart::None,
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
            IntersectionPart::Unique,
            IntersectionPart::None,
            OverlapAmount::Fraction(0.6),
            OverlapAmount::Bases(1),
        );
        // 5 bases of overlap is 0.5555 of the total 9 bases
        assert_eq!(r.len(), 0);

        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Not,
            IntersectionPart::Unique,
            IntersectionPart::None,
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
            IntersectionPart::Whole,
            IntersectionPart::None,
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
            IntersectionPart::Part,
            IntersectionPart::None,
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
        let bases = vec![15];
        let total_len = 100;
        assert!(overlap.sufficient(&bases, total_len, false));
    }

    #[test]
    fn test_sufficient_bases_with_fraction() {
        let overlap = OverlapAmount::Fraction(0.5);
        let bases = vec![50];
        let total_len = 100;
        assert!(overlap.sufficient(&bases, total_len, false));
        let bases = vec![49];
        assert!(!overlap.sufficient(&bases, total_len, false));
    }

    #[test]
    fn test_sufficient_bases_with_fraction_and_pieces() {
        // any individual piece must have 50% overlap
        let overlap = OverlapAmount::Fraction(0.5);
        let mut bases = vec![28, 28];
        let total_len = 100;
        // 56 total so OK.
        assert!(overlap.sufficient(&bases, total_len, false));
        // neither piece has 50% overlap
        assert!(!overlap.sufficient(&bases, total_len, true));
        bases[1] = 51;
        assert!(overlap.sufficient(&bases, total_len, true));
    }

    #[test]
    fn test_no_overlaps() {
        let intersections = make_example("a: 1-10");
        let r = intersections.report(
            IntersectionMode::Default,
            IntersectionMode::Default,
            IntersectionPart::Unique,
            IntersectionPart::None,
            OverlapAmount::Bases(5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 0);

        // check Not. should return A since there are no overlaps
        let intersections = make_example("a: 1-10");
        let r = intersections.report(
            IntersectionMode::Not,
            IntersectionMode::Default,
            IntersectionPart::Unique,
            IntersectionPart::None,
            OverlapAmount::Bases(5),
            OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
    }
}
