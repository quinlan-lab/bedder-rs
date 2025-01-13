use crate::intersection::{Intersection, Intersections};
use crate::position::Position;
pub use crate::report::{Report, ReportFragment};
#[allow(unused_imports)]
use crate::string::String;
use bitflags::bitflags;
use clap::ValueEnum;
use std::sync::Arc;

bitflags! {
    /// IntersectionMode indicates requirements for the intersection.
    /// And extra fields that might be reported.
    #[derive(Eq, PartialEq, Debug, Clone)]
    pub struct IntersectionMode: u8 {
        // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary

        /// Default without extra requirements.
        const Default = 0b00000000;

        /// Return A(B) if it does *not* overlap B(A). Bedtools -v
        const Not = 0b00000001;

        /// Constraints are per piece of interval (not the sum of overlapping intervals)
        const PerPiece = 0b00000010;

    }
}

impl From<&str> for IntersectionMode {
    fn from(s: &str) -> Self {
        let mut result = Self::Default;
        for c in s.chars() {
            match c {
                'v' => result |= Self::Not,
                'p' => result |= Self::PerPiece,
                _ => panic!("unknown intersection mode {}", c),
            }
        }
        result
    }
}

/// IntersectionPart indicates what to report for the intersection.
#[derive(Eq, PartialEq, Debug, Clone, ValueEnum)]
pub enum IntersectionPart {
    /// Don't report the intersection.
    /// This is commonly used for -b to not report b intervals.
    None,
    /// Report each portion of A that overlaps B
    Part,
    /// Report the whole interval of A that overlaps B
    Whole,
    /// Report each portion of A that does *NOT* overlap B
    Inverse,
}

impl std::fmt::Display for IntersectionPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntersectionPart::None => write!(f, "None"),
            IntersectionPart::Part => write!(f, "Part"),
            IntersectionPart::Whole => write!(f, "Whole"),
            IntersectionPart::Inverse => write!(f, "Inverse"),
        }
    }
}

impl Default for IntersectionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// OverlapAmount indicates the amount of overlap required.
/// Either as bases or as a fraction of the total length.
#[derive(PartialEq, Debug, Clone)]
pub enum OverlapAmount {
    Bases(u64),
    Fraction(f32),
}

impl From<&str> for OverlapAmount {
    fn from(s: &str) -> Self {
        if let Some(f) = s.strip_suffix('%') {
            Self::Fraction(
                f.parse::<f32>()
                    .unwrap_or_else(|_| panic!("error parsing fraction as float {}", s))
                    / 100.0,
            )
        } else if s.contains('.') {
            Self::Fraction(
                s.parse::<f32>()
                    .unwrap_or_else(|_| panic!("error parsing fraction as float {}", s)),
            )
        } else {
            Self::Bases(
                s.parse::<u64>()
                    .unwrap_or_else(|_| panic!("error parsing bases as int {}", s)),
            )
        }
    }
}

impl std::fmt::Display for OverlapAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlapAmount::Bases(bases) => write!(f, "Bases({})", bases),
            OverlapAmount::Fraction(fraction) => write!(f, "Fraction({:.3})", fraction),
        }
    }
}

impl Default for OverlapAmount {
    fn default() -> Self {
        Self::Bases(1)
    }
}

/// Extract pieces of base_interval that do no overlap overlaps
fn inverse(base_interval: &Position, overlaps: &[Intersection]) -> Vec<Position> {
    let mut last_start = base_interval.start();
    let mut result = Vec::new();
    for o in overlaps {
        if o.interval.start() > last_start {
            let mut p = base_interval.clone_box();
            p.set_start(last_start);
            p.set_stop(o.interval.start());
            result.push(p)
        }
        last_start = o.interval.stop();
    }
    if last_start < base_interval.stop() {
        let mut p = base_interval.clone_box();
        p.set_start(last_start);
        result.push(p)
    }
    result
}

impl Intersections {
    pub fn report(
        &self,
        a_mode: &IntersectionMode,
        b_mode: &IntersectionMode,
        a_part: &IntersectionPart,
        b_part: &IntersectionPart,
        a_requirements: &OverlapAmount,
        b_requirements: &OverlapAmount,
    ) -> Report {
        // usually the result is [query, [[b1-part, b1-part2, ...], [b2-part, ...]]]],
        // in fact, usually, there's only a single b and a single interval from b, so it's:
        // [query, [[b1-part]]]
        // but if a_part is Part, then there can be multiple query intervals.
        let mut result = Vec::new();

        let max_id = self.overlapping.iter().map(|o| o.id).max().unwrap_or(0);
        let mut grouped_intersections = vec![vec![]; max_id as usize + 1];

        // Group overlaps by Intersection.id
        // since all constraints on overlap are per source.
        // TODO: avoid this allocation by filtering on id in push_overlap_fragments
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
                    if Intersections::satisfies_requirements(
                        bases_overlap,
                        self.base_interval.stop() - self.base_interval.start(),
                        a_requirements,
                        a_mode,
                    ) && Intersections::satisfies_requirements(
                        bases_overlap,
                        b_interval.interval.stop() - b_interval.interval.start(),
                        b_requirements,
                        b_mode,
                    ) {
                        self.push_overlap_fragments(
                            &mut result,
                            &[b_interval.clone()],
                            a_part,
                            b_part,
                            b_idx,
                        );
                    }
                }
            } else {
                // Calculate cumulative overlap and sum of lengths for this group
                let total_bases_overlap = self.calculate_total_overlap(overlaps);
                if Intersections::satisfies_requirements(
                    total_bases_overlap,
                    self.base_interval.stop() - self.base_interval.start(),
                    a_requirements,
                    a_mode,
                ) && Intersections::satisfies_requirements(
                    total_bases_overlap,
                    overlaps
                        .iter()
                        .map(|o| o.interval.stop() - o.interval.start())
                        .sum(),
                    b_requirements,
                    b_mode,
                ) {
                    self.push_overlap_fragments(&mut result, overlaps, a_part, b_part, b_idx);
                }
            }
        }
        if a_mode.contains(IntersectionMode::Not) && self.overlapping.is_empty() {
            self.push_overlap_fragments(&mut result, &[], a_part, b_part, usize::MAX);
        }

        Report::new(result)
    }

    fn satisfies_requirements(
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
                let required_overlap = *fraction * interval_length as f32;
                if mode.contains(IntersectionMode::Not) {
                    (bases_overlap as f32) < required_overlap
                } else {
                    (bases_overlap as f32) >= required_overlap
                }
            }
        }
    }

    #[inline]
    fn calculate_overlap(&self, interval_a: Arc<Position>, interval_b: Arc<Position>) -> u64 {
        // NOTE!: we don't handle the case where there is no overlap. possible underflow. But we should
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
    fn push_overlap_fragments(
        &self,
        result: &mut Vec<ReportFragment>,
        overlaps: &[Intersection], // already grouped and only from b_idx.
        a_part: &IntersectionPart,
        b_part: &IntersectionPart,
        b_idx: usize, // index bs of result.
    ) {
        assert!(overlaps.iter().all(|o| o.id as usize == b_idx));

        let a_positions = match a_part {
            // for None, we still need the a_interval to report the b_interval
            IntersectionPart::None => vec![self.base_interval.clone_box()],
            IntersectionPart::Part => {
                // Create and adjust a_position if a_part is Part
                // Q: TODO: what to do here with multiple b files? keep intersection to smallest joint overlap?
                let mut a_interval = self.base_interval.clone_box();
                self.adjust_bounds(&mut a_interval, overlaps);
                vec![a_interval]
            }
            IntersectionPart::Whole => vec![self.base_interval.clone_box()],
            IntersectionPart::Inverse => inverse(&self.base_interval, overlaps),
        };

        a_positions.into_iter().for_each(|a_position| {
            let a_pos = if matches!(a_part, IntersectionPart::None) {
                None
            } else {
                Some(a_position.clone_box())
            };
            result.push(match b_part {
                // None, Part, Whole
                IntersectionPart::None => ReportFragment {
                    a: a_pos,
                    b: vec![],
                    id: b_idx,
                },
                IntersectionPart::Part => {
                    let mut b_positions = Vec::new();
                    for o in overlaps {
                        let mut b_interval = o.interval.clone_box();
                        b_interval.set_start(b_interval.start().max(self.base_interval.start()));
                        b_interval.set_stop(b_interval.stop().min(self.base_interval.stop()));
                        b_positions.push(b_interval);
                    }
                    ReportFragment {
                        a: a_pos,
                        b: b_positions,
                        id: b_idx,
                    }
                }
                IntersectionPart::Inverse => {
                    // if we have a: 1-10, b: 3-6, 8-12
                    // then we want to report b: [], 10-12
                    let mut b_positions = Vec::new();
                    for o in overlaps {
                        if o.interval.start() < self.base_interval.start() {
                            let mut b_interval = o.interval.clone_box();
                            b_interval.set_stop(b_interval.start());
                            b_positions.push(b_interval);
                        }
                        if o.interval.stop() > self.base_interval.stop() {
                            let mut b_interval = o.interval.clone_box();
                            b_interval.set_start(self.base_interval.stop());
                            b_positions.push(b_interval);
                        }
                    }
                    ReportFragment {
                        a: a_pos,
                        b: b_positions,
                        id: b_idx,
                    }
                }
                IntersectionPart::Whole => ReportFragment {
                    a: a_pos,
                    b: overlaps
                        .iter()
                        .map(|o| o.interval.clone_box())
                        .collect::<Vec<_>>(),
                    id: b_idx,
                },
            });
        });
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
    use super::*;
    use crate::tests::parse_intersections::parse_intersections;

    fn make_example(def: &str) -> Intersections {
        parse_intersections(def)
    }

    #[test]
    fn test_simple() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(5),
            &OverlapAmount::Bases(1),
        );
        eprintln!("{:?}", r);
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);

        assert_eq!(rf.b.len(), 2);
        assert_eq!(rf.b[0].start(), 3);
        assert_eq!(rf.b[0].stop(), 6);
        assert_eq!(rf.b[1].start(), 8);
        assert_eq!(rf.b[1].stop(), 12);
    }

    #[test]
    fn test_inverse() {
        let intersections = make_example("a: 1-10\nb: 3-6, 4-6, 8-12");
        let inv = inverse(&intersections.base_interval, &intersections.overlapping);
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].start(), 1);
        assert_eq!(inv[0].stop(), 3);
        assert_eq!(inv[1].start(), 6);
        assert_eq!(inv[1].stop(), 8);
    }

    #[test]
    fn test_inverse_no_results() {
        let intersections = make_example("a: 1-10\nb: 1-6, 6-10");
        let inv = inverse(&intersections.base_interval, &intersections.overlapping);
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_inverse_right_overhang() {
        let intersections = make_example("a: 1-10\nb: 1-6, 6-8");
        let inv = inverse(&intersections.base_interval, &intersections.overlapping);
        assert_eq!(inv.len(), 1);
        assert_eq!(inv[0].start(), 8);
        assert_eq!(inv[0].stop(), 10);
    }

    #[test]
    fn test_not() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(5),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 0);

        // now we increase the a base requirement so it is NOT met.
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(10), // not met.
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);
        let b = &rf.b;
        assert_eq!(b.len(), 2);

        // now we also use b not
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Not,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(10), // not met.
            &OverlapAmount::Bases(1),  // met but we required not.
        );
        assert_eq!(r.len(), 0);

        // now we also use b not
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Not,
            &IntersectionPart::Whole,
            &IntersectionPart::Part,
            &OverlapAmount::Bases(10), // not met.
            &OverlapAmount::Bases(10), // met but we required not.
        );
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);
        let bs = &rf.b;
        assert_eq!(2, bs.len());
        assert_eq!(bs[0].start(), 3);
        assert_eq!(bs[0].stop(), 6);
        assert_eq!(bs[1].start(), 8);
        assert_eq!(bs[1].stop(), 10);

        eprintln!("{:?}", r);
    }

    #[test]
    fn test_a_not() {
        let intersections = make_example("a: 1-10\nb: 3-4, 6-7, 8-12\nb:9-20");
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert!(r.is_empty());
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(6),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn test_a_not_with_empty() {
        let intersections = make_example("a: 1-10");
        let r = intersections.report(
            &IntersectionMode::Not,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(1, r.len());
        assert_eq!(r[0].id, usize::MAX);
    }

    #[test]
    fn test_b_inverse() {
        let intersections = make_example("a: 1-10\nb: 3-4, 6-7, 8-12\nb:9-20");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Inverse,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2);

        assert_eq!(r[0].b.len(), 1);
        assert_eq!(r[0].b[0].start(), 10);
        assert_eq!(r[0].b[0].stop(), 12);

        assert_eq!(r[1].b.len(), 1);
        assert_eq!(r[1].b[0].start(), 10);
        assert_eq!(r[1].b[0].stop(), 20);
    }

    #[test]
    fn test_multiple_bs() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12\nb:9-20");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Part,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2); // one for each b.
        assert!(r[0].id == 0);
        assert!(r[1].id == 1);
        assert_eq!(r[1].b[0].start(), 9);
        assert_eq!(r[1].b[0].stop(), 20);
        assert_eq!(r[1].a.as_ref().unwrap().start(), 9);
        assert_eq!(r[1].a.as_ref().unwrap().stop(), 10);
    }

    #[test]
    fn test_a_pieces() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::PerPiece,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Whole,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 2);
        let rf = &r[0];
        // test that a is 1-10
        assert_eq!(rf.a.as_ref().unwrap().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);
        // test that b is 3-6
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].start(), 3);
        assert_eq!(rf.b[0].stop(), 6);

        let rf = &r[1];
        assert_eq!(rf.a.as_ref().unwrap().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].start(), 8);
        assert_eq!(rf.b[0].stop(), 12);
    }

    #[test]
    fn test_a_pieces_ab_part() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::PerPiece,
            &IntersectionMode::Default,
            &IntersectionPart::Part,
            &IntersectionPart::Part,
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        // a: 3-6, b: 3-6
        // a: 8-10, b: 8-10
        assert_eq!(r.len(), 2);
        let rf = &r[0];
        // test that a is 3-6
        assert_eq!(rf.a.as_ref().unwrap().start(), 3);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 6);
        // test that b is 3-6
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].start(), 3);
        assert_eq!(rf.b[0].stop(), 6);

        let rf = &r[1];
        assert_eq!(rf.a.as_ref().unwrap().start(), 8);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].start(), 8);
        assert_eq!(rf.b[0].stop(), 10);
    }

    #[test]
    fn test_b_part() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Part,
            // only 4 bases of overlap.
            &OverlapAmount::Bases(5),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 0);
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::Part,
            &OverlapAmount::Bases(4),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().start(), 4);
        assert_eq!(rf.a.as_ref().unwrap().stop(), 10);

        assert_eq!(rf.b.len(), 2);
        // note that b is chopped to 4
        assert_eq!(rf.b[0].start(), 4);
        assert_eq!(rf.b[0].stop(), 6);
        assert_eq!(rf.b[1].start(), 8);
        // and 10.
        assert_eq!(rf.b[1].stop(), 10);
    }

    #[test]
    fn test_b_none() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::Whole,
            &IntersectionPart::None,
            // only 4 bases of overlap.
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].a.as_ref().unwrap().start(), 4);
        assert_eq!(r[0].a.as_ref().unwrap().stop(), 10);
        assert_eq!(r[0].b.len(), 0);
    }

    #[test]
    fn test_a_none() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let r = intersections.report(
            &IntersectionMode::Default,
            &IntersectionMode::Default,
            &IntersectionPart::None,
            &IntersectionPart::Part,
            // only 4 bases of overlap.
            &OverlapAmount::Bases(1),
            &OverlapAmount::Bases(1),
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].a, None);
        let rf = &r[0];
        // note that b is chopped to 4
        assert_eq!(rf.b[0].start(), 4);
        assert_eq!(rf.b[0].stop(), 6);
        assert_eq!(rf.b[1].start(), 8);
        // and 10.
        assert_eq!(rf.b[1].stop(), 10);
    }

    /*
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
    */

    #[test]
    fn test_sufficient_bases_with_bases() {
        assert!(Intersections::satisfies_requirements(
            15,
            100,
            &OverlapAmount::Bases(10),
            &IntersectionMode::Default
        ));
        assert!(!Intersections::satisfies_requirements(
            8,
            8,
            &OverlapAmount::Bases(10),
            &IntersectionMode::Default
        ));
    }

    #[test]
    fn test_satisifies_reqs_bases_with_not() {
        assert!(Intersections::satisfies_requirements(
            1,
            100,
            &OverlapAmount::Bases(10),
            &IntersectionMode::Not
        ));
    }

    #[test]
    fn test_sufficient_bases_with_fraction() {
        assert!(!Intersections::satisfies_requirements(
            28,
            100,
            &OverlapAmount::Fraction(0.5),
            &IntersectionMode::Default
        ));

        assert!(Intersections::satisfies_requirements(
            51,
            100,
            &OverlapAmount::Fraction(0.5),
            &IntersectionMode::Default
        ));
    }

    /*
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
    */
}
