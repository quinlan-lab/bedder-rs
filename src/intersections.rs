use crate::intersection::{Intersection, Intersections};
use crate::position::Position;
#[allow(unused_imports)]
use crate::string::String;
use bitflags::bitflags;
use std::ops::Deref;
use std::sync::Arc;

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

pub struct ReportFragment {
    pub a: Option<Position>,
    pub b: Vec<Position>,
    pub id: usize,
}

impl Intersections {
    pub fn report(
        &self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_part: IntersectionPart,
        b_part: IntersectionPart,
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
    ) -> Vec<ReportFragment> {
        // usually the result is [query, [[b1-part, b1-part2, ...], [b2-part, ...]]]],
        // in fact, usually, there's only a single b and a single interval from b, so it's:
        // [query, [[b1-part]]]
        // but if a_part is Part, then there can be multiple query intervals.
        let mut result = Vec::new();

        let max_id = self.overlapping.iter().map(|o| o.id).max().unwrap_or(0);
        let mut grouped_intersections = vec![vec![]; max_id as usize + 1];

        // Group overlaps by Intersection.id
        // since all constraints on overlap are per source.
        // TODO: avoid this allocation by filtering on id in get_overlap_fragment
        for intersection in &self.overlapping {
            grouped_intersections[intersection.id as usize].push(intersection.clone());
        }

        for (b_idx, overlaps) in grouped_intersections.iter().enumerate() {
            // so now all overlaps are from b[id]
            if a_mode.contains(IntersectionMode::PerPiece) {
                // each b_interval must go with the a_piece that it overlaps.
                for b_interval in overlaps {
                    let bases_overlap = self
                        .calculate_overlap(self.base_interval.clone(), b_interval.interval.clone());
                    if self.satisfies_requirements(
                        bases_overlap,
                        self.base_interval.stop() - self.base_interval.start(),
                        &a_requirements,
                        &a_mode,
                    ) && self.satisfies_requirements(
                        bases_overlap,
                        b_interval.interval.stop() - b_interval.interval.start(),
                        &b_requirements,
                        &b_mode,
                    ) {
                        let frag = self.get_overlap_fragment(
                            &[b_interval.clone()],
                            &a_part,
                            &b_part,
                            b_idx,
                        );
                        result.push(frag);
                    }
                }
            } else {
                // Calculate cumulative overlap and sum of lengths for this group
                let total_bases_overlap = self.calculate_total_overlap(overlaps);
                if self.satisfies_requirements(
                    total_bases_overlap,
                    self.base_interval.stop() - self.base_interval.start(),
                    &a_requirements,
                    &a_mode,
                ) && self.satisfies_requirements(
                    total_bases_overlap,
                    overlaps
                        .iter()
                        .map(|o| o.interval.stop() - o.interval.start())
                        .sum(),
                    &b_requirements,
                    &b_mode,
                ) {
                    let fragment = self.get_overlap_fragment(overlaps, &a_part, &b_part, b_idx);
                    result.push(fragment);
                }
            }
        }

        result
    }

    fn satisfies_requirements(
        &self,
        bases_overlap: u64,
        interval_length: u64,
        requirements: &OverlapAmount,
        mode: &IntersectionMode,
    ) -> bool {
        match requirements {
            OverlapAmount::Bases(bases) => {
                if mode.contains(IntersectionMode::Not) {
                    bases_overlap < *bases
                } else {
                    bases_overlap >= *bases
                }
            }
            OverlapAmount::Fraction(fraction) => {
                let required_overlap = (*fraction * interval_length as f32) as u64;
                if mode.contains(IntersectionMode::Not) {
                    bases_overlap < required_overlap
                } else {
                    bases_overlap >= required_overlap
                }
            }
        }
    }

    #[inline]
    fn calculate_overlap(&self, interval_a: Arc<Position>, interval_b: Arc<Position>) -> u64 {
        // TODO: we don't handle the case where there is no overlap. possible underflow. But we should
        // only get overlapping intervals here.
        interval_a.stop().min(interval_b.stop()) - interval_a.start().max(interval_b.start())
    }

    #[inline]
    fn calculate_total_overlap(&self, overlaps: &[Intersection]) -> u64 {
        // Implement logic to calculate total overlap in bases for a group of intervals
        overlaps
            .iter()
            .map(|o| self.calculate_overlap(self.base_interval.clone(), o.interval.clone()))
            .sum()
    }
    fn get_overlap_fragment(
        &self,
        overlaps: &[Intersection], // already grouped and only from b_idx.
        a_part: &IntersectionPart,
        b_part: &IntersectionPart,
        b_idx: usize, // index bs of result.
    ) -> ReportFragment {
        assert!(overlaps.iter().all(|o| o.id as usize == b_idx));

        let a_position = match a_part {
            IntersectionPart::None => None,
            IntersectionPart::Part => {
                // Create and adjust a_position if a_part is Part
                // Q: what to do here with multiple b files? keep intersection to smallest joint overlap?
                let mut a_interval = self.base_interval.dup();
                self.adjust_bounds(&mut a_interval, overlaps);
                Some(a_interval)
            }
            _ => Some(self.base_interval.dup()), // For Whole and Unique
        };

        return match b_part {
            // None, Part, Unique, Whole
            IntersectionPart::None => ReportFragment {
                a: a_position,
                b: vec![],
                id: b_idx,
            },
            IntersectionPart::Unique => {
                unimplemented!("Unique B not implemented yet. Is it even possible?")
            }
            IntersectionPart::Part => {
                let mut b_positions = Vec::new();
                for o in overlaps {
                    let mut b_interval = o.interval.dup();
                    b_interval.set_start(b_interval.start().max(self.base_interval.start()));
                    b_interval.set_stop(b_interval.stop().min(self.base_interval.stop()));
                    b_positions.push(b_interval);
                }
                ReportFragment {
                    a: a_position,
                    b: b_positions,
                    id: b_idx,
                }
            }
            IntersectionPart::Whole => ReportFragment {
                a: a_position,
                b: overlaps
                    .iter()
                    .map(|o| o.interval.dup())
                    .collect::<Vec<_>>(),
                id: b_idx,
            },
        };
    }

    fn adjust_bounds(&self, interval: &mut Position, overlaps: &[Intersection]) {
        // Implement logic to adjust the start and stop positions of the interval based on overlaps
        // Example:
        // a: 1-10
        // b: 3-6, 8-12
        // a adjusted to 3-10
        if overlaps.is_empty() {
            return;
        }
        interval.set_start(
            overlaps
                .iter()
                .map(|o| o.interval.start())
                .min()
                .unwrap_or(interval.start())
                .max(interval.start()),
        );
        interval.set_stop(
            overlaps
                .iter()
                .map(|o| o.interval.stop())
                .max()
                .unwrap_or(interval.stop())
                .min(interval.stop()),
        );
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
