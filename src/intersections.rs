use crate::intersection::{Intersection, Intersections};
use crate::position::Position;
#[allow(unused_imports)]
use crate::string::String;
use bitflags::bitflags;
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

impl Intersections {
    pub fn report(
        &self,
        a_mode: IntersectionMode,
        b_mode: IntersectionMode,
        a_part: IntersectionPart,
        b_part: IntersectionPart,
        a_requirements: OverlapAmount,
        b_requirements: OverlapAmount,
    ) -> Vec<(Option<Position>, Vec<Vec<Position>>)> {
        let mut result = Vec::new();

        let max_id = self.overlapping.iter().map(|o| o.id).max().unwrap_or(0);
        let mut grouped_overlaps = vec![Vec::new(); max_id as usize + 1];

        // Group overlaps by Intersection.id
        for overlap in &self.overlapping {
            grouped_overlaps[overlap.id as usize].push(overlap.interval.clone());
        }

        for (id, overlaps) in grouped_overlaps.iter().enumerate() {
            if a_mode.contains(IntersectionMode::PerPiece) {
                // Check each piece individually
                for b_interval in overlaps {
                    let bases_overlap =
                        self.calculate_overlap(self.base_interval.clone(), b_interval);
                    if self.satisfies_requirements(
                        bases_overlap,
                        self.base_interval.stop() - self.base_interval.start(),
                        &a_requirements,
                        &a_mode,
                    ) && self.satisfies_requirements(
                        bases_overlap,
                        b_interval.stop() - b_interval.start(),
                        &b_requirements,
                        &b_mode,
                    ) {
                        self.append_to_result(&mut result, overlaps, &a_part, &b_part, id);
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
                    overlaps.iter().map(|o| o.stop() - o.start()).sum(),
                    &b_requirements,
                    &b_mode,
                ) {
                    self.append_to_result(&mut result, overlaps, &a_part, &b_part, id);
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
    fn calculate_overlap(&self, interval_a: Arc<Position>, interval_b: &Arc<Position>) -> u64 {
        // TODO: we don't handle the case where there is no overlap. possible underflow. But we should
        // only get overlapping intervals here.
        interval_a.stop().min(interval_b.stop()) - interval_a.start().max(interval_b.start())
    }

    #[inline]
    fn calculate_total_overlap(&self, overlaps: &[Arc<Position>]) -> u64 {
        // Implement logic to calculate total overlap in bases for a group of intervals
        overlaps
            .iter()
            .map(|o| self.calculate_overlap(self.base_interval.clone(), o))
            .sum()
    }
    fn append_to_result(
        &self,
        result: &mut Vec<(Option<Position>, Vec<Vec<Position>>)>,
        overlaps: &[Arc<Intersection>], // already groups and from id.
        a_part: &IntersectionPart,
        b_part: &IntersectionPart,
        id: usize, // index into result.
    ) {
        // Dynamically sized vector to group b_positions by their id
        let mut b_positions_grouped: Vec<Vec<Position>> = Vec::new();

        // Process overlaps
        for overlap in overlaps {
            while overlap.id as usize >= b_positions_grouped.len() {
                b_positions_grouped.push(Vec::new());
            }

            let mut b_interval = overlap.interval.dup();
            if let IntersectionPart::Part = b_part {
                // Adjust bounds for b_interval if b_part is Part
                self.adjust_bounds(&mut b_interval, &[overlap.clone()]);
            }
            b_positions_grouped[overlap.id as usize].push(b_interval);
        }

        let a_position = match a_part {
            IntersectionPart::None => None,
            IntersectionPart::Part => {
                // Create and adjust a_position if a_part is Part
                let mut a_interval = self.base_interval.dup();
                self.adjust_bounds(&mut a_interval, overlaps);
                Some(a_interval)
            }
            _ => Some(self.base_interval.dup()), // For Whole and Unique
        };

        match b_part {
            IntersectionPart::None => result.push((a_position, Vec::new())),
            IntersectionPart::Unique => {
                if let Some(first_overlap) = overlaps.first() {
                    let unique_b_position = vec![first_overlap.interval.dup()];
                    result.push((a_position, vec![unique_b_position]));
                }
            }
            _ => result.push((a_position, b_positions_grouped)), // For Part and Whole
        }
    }

    fn adjust_bounds(&self, interval: &mut Position, overlaps: &[Arc<Intersection>]) {
        // Implement logic to adjust the start and stop positions of the interval based on overlaps
        // Example:
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
