use crate::intersection::{Intersection, Intersections};
use crate::position::Position;
use crate::report::{Report, ReportFragment};
use crate::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};
#[allow(unused_imports)]
use crate::string::String;
use parking_lot::Mutex;
use std::sync::Arc;

/// Extract pieces of base_interval that do no overlap overlaps
fn inverse(base_interval: &Position, overlaps: &[Intersection]) -> Vec<Arc<Mutex<Position>>> {
    let mut last_start = base_interval.start();
    let mut result = Vec::new();
    for o in overlaps {
        let o = o.interval.try_lock().expect("failed to lock interval");
        if o.start() > last_start {
            let mut p = base_interval.clone_box();
            p.set_start(last_start);
            p.set_stop(o.start());
            result.push(Arc::new(Mutex::new(p)))
        }
        last_start = o.stop();
    }
    if last_start < base_interval.stop() {
        let mut p = base_interval.clone_box();
        p.set_start(last_start);
        result.push(Arc::new(Mutex::new(p)))
    }
    result
}

// with a-piece, we changed the bounds of the interval so we need to then limit b intervals
// to those that still overlap the new bounds.
// Takes &mut result as it modifies the b vectors in place.
fn filter_part_overlaps(result: &mut [ReportFragment]) {
    // No need for initial assert, it doesn't prevent the deadlock scenario where 'a' and 'b'
    // within the same fragment point to the same locked Arc.

    for fr in result {
        // Check if a exists; if a_piece was None, 'a' will be None.
        // If a doesn't exist, no filtering based on 'a' is possible/needed.
        let a_arc_option = fr.a.as_ref();
        if a_arc_option.is_none() {
            continue;
        }
        let a_arc = a_arc_option.unwrap();

        // Lock 'a' once for this fragment
        let a = a_arc
            .try_lock()
            .expect("filter_part_overlaps: failed to lock a interval");

        // Use retain_mut for in-place filtering without extra allocation
        fr.b.retain_mut(|b_arc| {
            // Check if a and b point to the same Arc instance
            if Arc::ptr_eq(a_arc, b_arc) {
                // If they are the same, the overlap is inherent (based on how 'a' was derived)
                // Keep this b interval.
                true
            } else {
                // If they are different Arcs, we can safely lock b
                let b = b_arc
                    .try_lock()
                    .expect("filter_part_overlaps: failed to lock b interval");
                // Check for overlap: a.start <= b.stop && a.stop >= b.start
                
                // drop(b) happens here automatically when guard goes out of scope
                // Keep b only if it overlaps with a
                a.start() <= b.stop() && a.stop() >= b.start()
            }
        });
        // drop(a) happens here automatically when guard goes out of scope
    }
}

#[inline]
fn has_overlap(a: &Position, b: &Position) -> bool {
    a.start() <= b.stop() && a.stop() >= b.start()
}

impl Intersections {
    pub fn report(&self, report_options: &ReportOptions) -> Arc<Report> {
        let mut cached_report = self
            .cached_report
            .try_lock()
            .expect("failed to lock cached report");
        if let Some((ro, report)) = &*cached_report {
            if ro == report_options {
                return report.clone();
            }
        }
        // usually the result is [query, [[b1-piece, b1-piece2, ...], [b2-piece, ...]]]],
        // in fact, usually, there's only a single b and a single interval from b, so it's:
        // [query, [[b1-piece]]]
        // but if a_piece is Part, then there can be multiple query intervals.
        let mut result = Vec::new();

        let max_id = self.overlapping.iter().map(|o| o.id).max().unwrap_or(0);
        let mut grouped_intersections = vec![vec![]; max_id as usize + 1];

        // Group overlaps by Intersection.id
        // since all constraints on overlap are per source.
        // TODO: avoid this allocation by filtering on id in push_overlap_fragments
        for intersection in &self.overlapping {
            grouped_intersections[intersection.id as usize].push(intersection.clone());
        }
        if grouped_intersections.iter().any(|v| !v.is_empty()) {
            log::trace!("grouped_intersections: {:?}", grouped_intersections);
        }

        for (b_idx, overlaps) in grouped_intersections.iter().enumerate() {
            // so now all overlaps are from b[id]
            if report_options.a_mode == IntersectionMode::PerPiece {
                // each b_interval must go with the a_piece that it overlaps.
                let mut b_satisified = vec![];
                for b_interval in overlaps {
                    let bases_overlap = self
                        .calculate_overlap(self.base_interval.clone(), b_interval.interval.clone());
                    let base = self
                        .base_interval
                        .try_lock()
                        .expect("failed to lock interval");
                    let b = b_interval
                        .interval
                        .try_lock()
                        .expect("failed to lock interval");
                    if Intersections::satisfies_requirements(
                        bases_overlap,
                        base.stop() - base.start(),
                        &report_options.a_requirements,
                        &report_options.a_mode,
                    ) && Intersections::satisfies_requirements(
                        bases_overlap,
                        b.stop() - b.start(),
                        &report_options.b_requirements,
                        &report_options.b_mode,
                    ) {
                        drop(b);
                        drop(base);
                        b_satisified.push(b_interval.clone());
                    }
                }
                if !b_satisified.is_empty() {
                    self.push_overlap_fragments(
                        &mut result,
                        &b_satisified,
                        &report_options.a_piece,
                        &report_options.b_piece,
                        b_idx,
                    );
                }
            } else {
                // Calculate cumulative overlap and sum of lengths for this group
                let total_bases_overlap = self.calculate_total_overlap(overlaps, report_options);
                let base = self
                    .base_interval
                    .try_lock()
                    .expect("failed to lock interval");
                if Intersections::satisfies_requirements(
                    total_bases_overlap,
                    base.stop() - base.start(),
                    &report_options.a_requirements,
                    &report_options.a_mode,
                ) && Intersections::satisfies_requirements(
                    total_bases_overlap,
                    overlaps
                        .iter()
                        .map(|o| {
                            let ov = o.interval.try_lock().expect("failed to lock interval");
                            ov.stop() - ov.start()
                        })
                        .sum(),
                    &report_options.b_requirements,
                    &report_options.b_mode,
                ) {
                    drop(base);
                    self.push_overlap_fragments(
                        &mut result,
                        overlaps,
                        &report_options.a_piece,
                        &report_options.b_piece,
                        b_idx,
                    );
                }
            }
        }
        if report_options.a_mode == IntersectionMode::Not && self.overlapping.is_empty() {
            self.push_overlap_fragments(
                &mut result,
                &[],
                &report_options.a_piece,
                &report_options.b_piece,
                usize::MAX,
            );
        }

        if matches!(report_options.a_piece, IntersectionPart::Piece) {
            // Pass as mutable reference now
            filter_part_overlaps(&mut result);
        }

        let report = Arc::new(Report::new(result));
        *cached_report = Some((report_options.clone(), report.clone()));
        report
    }

    fn satisfies_requirements(
        bases_overlap: u64,
        interval_length: u64,
        requirements: &OverlapAmount,
        mode: &IntersectionMode,
    ) -> bool {
        match requirements {
            OverlapAmount::Bases(bases) => {
                if *mode == IntersectionMode::Not {
                    bases_overlap < *bases
                } else {
                    bases_overlap >= *bases
                }
            }
            OverlapAmount::Fraction(fraction) => {
                let required_overlap = *fraction * interval_length as f32;
                if *mode == IntersectionMode::Not {
                    (bases_overlap as f32) < required_overlap
                } else {
                    (bases_overlap as f32) >= required_overlap
                }
            }
        }
    }

    #[inline]
    fn calculate_overlap(
        &self,
        interval_a: Arc<Mutex<Position>>,
        interval_b: Arc<Mutex<Position>>,
    ) -> u64 {
        let a = interval_a.try_lock().expect("failed to lock interval");
        let b = interval_b.try_lock().expect("failed to lock interval");
        (a.stop().min(b.stop())).saturating_sub(a.start().max(b.start()))
    }

    #[inline]
    fn calculate_total_overlap(
        &self,
        overlaps: &[Intersection],
        report_options: &ReportOptions,
    ) -> u64 {
        // Implement logic to calculate total overlap in bases for a group of intervals
        overlaps
            .iter()
            .map(|o| {
                // TODO: what to do here if distance and/or n_closest are > 0?
                let ovl = self.calculate_overlap(self.base_interval.clone(), o.interval.clone());

                if report_options.a_mode == IntersectionMode::PerPiece {
                    let a_req = match report_options.a_requirements {
                        OverlapAmount::Bases(bases) => ovl >= bases,
                        OverlapAmount::Fraction(fraction) => {
                            let iv = self
                                .base_interval
                                .try_lock()
                                .expect("failed to lock interval");
                            let interval_length = iv.stop() - iv.start();
                            ovl as f32 >= fraction * interval_length as f32
                        }
                    };
                    if !a_req {
                        return 0;
                    }
                }
                if report_options.b_mode == IntersectionMode::PerPiece {
                    let b_req = match report_options.b_requirements {
                        OverlapAmount::Bases(bases) => ovl >= bases,
                        OverlapAmount::Fraction(fraction) => {
                            let iv = o.interval.try_lock().expect("failed to lock interval");
                            let interval_length = iv.stop() - iv.start();
                            ovl as f32 >= fraction * interval_length as f32
                        }
                    };
                    if !b_req {
                        return 0;
                    }
                }
                ovl
            })
            .sum()
    }
    fn push_overlap_fragments(
        &self,
        result: &mut Vec<ReportFragment>,
        overlaps: &[Intersection], // already grouped and only from b_idx.
        a_piece: &IntersectionPart,
        b_piece: &IntersectionPart,
        b_idx: usize, // index bs of result.
    ) {
        assert!(overlaps.iter().all(|o| o.id as usize == b_idx));

        if matches!(a_piece, IntersectionPart::Whole) {
            let locked_base = self
                .base_interval
                .try_lock()
                .expect("failed to lock interval");
            let base = locked_base.clone_box();
            drop(locked_base);

            let base_start = base.start();
            let base_stop = base.stop();

            let make_b_positions = |intersection: &Intersection| -> Vec<Arc<Mutex<Position>>> {
                match b_piece {
                    IntersectionPart::None => vec![],
                    IntersectionPart::WholeWide | IntersectionPart::Whole => {
                        vec![intersection.interval.clone()]
                    }
                    IntersectionPart::Piece => {
                        let o = intersection
                            .interval
                            .try_lock()
                            .expect("failed to lock interval");
                        let mut b_interval = o.clone_box();
                        b_interval.set_start(b_interval.start().max(base_start));
                        b_interval.set_stop(b_interval.stop().min(base_stop));
                        drop(o);
                        vec![Arc::new(Mutex::new(b_interval))]
                    }
                    IntersectionPart::Inverse => {
                        let o = intersection
                            .interval
                            .try_lock()
                            .expect("failed to lock interval");
                        let mut b_positions = Vec::new();
                        if o.start() < base_start {
                            let mut b_interval = o.clone_box();
                            b_interval.set_stop(b_interval.start());
                            b_positions.push(Arc::new(Mutex::new(b_interval)));
                        }
                        if o.stop() > base_stop {
                            let mut b_interval = o.clone_box();
                            b_interval.set_start(base_stop);
                            b_positions.push(Arc::new(Mutex::new(b_interval)));
                        }
                        drop(o);
                        b_positions
                    }
                }
            };

            if overlaps.is_empty() {
                result.push(ReportFragment {
                    a: Some(Arc::new(Mutex::new(base))),
                    b: vec![],
                    id: b_idx,
                });
            } else {
                for o in overlaps {
                    result.push(ReportFragment {
                        a: Some(Arc::new(Mutex::new(base.clone_box()))),
                        b: make_b_positions(o),
                        id: b_idx,
                    });
                }
            }
            return;
        }

        let a_positions = match a_piece {
            // for None, we still need the a_interval to report the b_interval
            IntersectionPart::None | IntersectionPart::WholeWide => vec![self.base_interval.clone()],
            IntersectionPart::Piece => {
                // Create and adjust a_position if a_piece is Part
                overlaps
                    .iter()
                    .map(|o| {
                        let oi = o.interval.try_lock().expect("failed to lock interval");
                        let bi = self
                            .base_interval
                            .try_lock()
                            .expect("failed to lock interval");
                        //if oi.start() < bi.start() || oi.stop() > bi.stop() {
                        if !has_overlap(&oi, &bi) {
                            Arc::new(Mutex::new(bi.clone_box()))
                        } else if oi.start() > bi.start() || oi.stop() < bi.stop() {
                            let mut oc = bi.clone_box();
                            oc.set_start(oi.start().max(bi.start()));
                            oc.set_stop(oi.stop().min(bi.stop()));
                            drop(bi);
                            drop(oi);
                            Arc::new(Mutex::new(oc))
                        } else {
                            drop(oi);
                            Arc::new(Mutex::new(bi.clone_box()))
                        }
                    })
                    .collect()
            }
            IntersectionPart::Inverse => {
                let locked_base = self
                    .base_interval
                    .try_lock()
                    .expect("failed to lock interval");
                let base_clone = locked_base.clone_box();
                drop(locked_base); // Explicitly drop the lock
                inverse(&base_clone, overlaps)
            }
            IntersectionPart::Whole => unreachable!("handled above"),
        };

        a_positions.iter().for_each(|a_position| {
            let a_pos = if matches!(a_piece, IntersectionPart::None) {
                None
            } else {
                Some(a_position.clone())
            };
            result.push(match b_piece {
                // None, Part, Whole
                IntersectionPart::None => ReportFragment {
                    a: a_pos,
                    b: vec![],
                    id: b_idx,
                },
                IntersectionPart::Piece => {
                    let mut b_positions = Vec::new();
                    let base = self
                        .base_interval
                        .try_lock()
                        .expect("failed to lock interval");
                    for o in overlaps {
                        let o = o.interval.try_lock().expect("failed to lock interval");
                        let mut b_interval = o.clone_box();
                        b_interval.set_start(b_interval.start().max(base.start()));
                        b_interval.set_stop(b_interval.stop().min(base.stop()));
                        b_positions.push(Arc::new(Mutex::new(b_interval)));
                        drop(o);
                    }
                    drop(base);
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
                    let base = self
                        .base_interval
                        .try_lock()
                        .expect("failed to lock interval");
                    for o in overlaps {
                        let o = o.interval.try_lock().expect("failed to lock interval");
                        if o.start() < base.start() {
                            let mut b_interval = o.clone_box();
                            b_interval.set_stop(b_interval.start());
                            b_positions.push(Arc::new(Mutex::new(b_interval)));
                        }
                        if o.stop() > base.stop() {
                            let mut b_interval = o.clone_box();
                            b_interval.set_start(base.stop());
                            b_positions.push(Arc::new(Mutex::new(b_interval)));
                        }
                        drop(o);
                    }
                    ReportFragment {
                        a: a_pos,
                        b: b_positions,
                        id: b_idx,
                    }
                }
                IntersectionPart::WholeWide | IntersectionPart::Whole => ReportFragment {
                    a: a_pos,
                    b: overlaps
                        .iter()
                        .map(|o| o.interval.clone())
                        .collect::<Vec<_>>(),
                    id: b_idx,
                },
            });
        });
    }

    /*
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
                .map(|o| {
                    o.interval
                        .try_lock()
                        .expect("failed to lock interval")
                        .start()
                })
                .min()
                .unwrap_or(interval.start())
                .max(interval.start()),
        );
        interval.set_stop(
            overlaps
                .iter()
                .map(|o| {
                    o.interval
                        .try_lock()
                        .expect("failed to lock interval")
                        .stop()
                })
                .max()
                .unwrap_or(interval.stop())
                .min(interval.stop()),
        );
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse_intersections::parse_intersections;
    use std::sync::Arc;

    fn make_example(def: &str) -> Intersections {
        parse_intersections(def)
    }

    #[test]
    fn test_whole_long_reports_full_a_per_overlap() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_piece = IntersectionPart::Whole;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);

        let r = intersections.report(&ro);
        assert_eq!(r.len(), 2);

        let a0 = r[0].a.as_ref().unwrap();
        let a1 = r[1].a.as_ref().unwrap();
        assert!(
            !Arc::ptr_eq(a0, a1),
            "Expected whole-long to clone A per hit"
        );

        let a0l = a0.lock();
        assert_eq!(a0l.start(), 1);
        assert_eq!(a0l.stop(), 10);
        drop(a0l);
        let a1l = a1.lock();
        assert_eq!(a1l.start(), 1);
        assert_eq!(a1l.stop(), 10);

        assert_eq!(r[0].b.len(), 1);
        assert_eq!(r[1].b.len(), 1);
        let b0 = r[0].b[0].lock();
        assert_eq!(b0.start(), 3);
        assert_eq!(b0.stop(), 6);
        let b1 = r[1].b[0].lock();
        assert_eq!(b1.start(), 8);
        assert_eq!(b1.stop(), 12);
    }

    #[test]
    fn test_simple() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_requirements = OverlapAmount::Bases(5);
        ro.b_requirements = OverlapAmount::Bases(1);

        let r = intersections.report(&ro);
        eprintln!("{:?}", r);
        assert_eq!(r.len(), 1);
        let rf = &r[0].a.as_ref().unwrap().lock();
        assert_eq!(rf.start(), 1);
        assert_eq!(rf.stop(), 10);

        let bs = &r[0].b;
        assert_eq!(bs.len(), 2);
        let b0 = &bs[0].lock();
        assert_eq!(b0.start(), 3);
        assert_eq!(b0.stop(), 6);
        let b1 = &bs[1].lock();
        assert_eq!(b1.start(), 8);
        assert_eq!(b1.stop(), 12);
    }

    #[test]
    fn test_inverse() {
        let intersections = make_example("a: 1-10\nb: 3-6, 4-6, 8-12");
        let inv = inverse(
            &intersections.base_interval.lock(),
            &intersections.overlapping,
        );
        assert_eq!(inv.len(), 2);
        let inv0 = inv[0].lock();
        assert_eq!(inv0.start(), 1);
        assert_eq!(inv0.stop(), 3);
        let inv1 = inv[1].lock();
        assert_eq!(inv1.start(), 6);
        assert_eq!(inv1.stop(), 8);
    }

    #[test]
    fn test_inverse_no_results() {
        let intersections = make_example("a: 1-10\nb: 1-6, 6-10");
        let inv = inverse(
            &intersections.base_interval.lock(),
            &intersections.overlapping,
        );
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_inverse_right_overhang() {
        let intersections = make_example("a: 1-10\nb: 1-6, 6-8");
        let inv = inverse(
            &intersections.base_interval.lock(),
            &intersections.overlapping,
        );
        assert_eq!(inv.len(), 1);
        let inv0 = inv[0].lock();
        assert_eq!(inv0.start(), 8);
        assert_eq!(inv0.stop(), 10);
    }

    #[test]
    fn test_not() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");

        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Not;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(5);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 0);

        // now we increase the a base requirement so it is NOT met.
        ro.a_requirements = OverlapAmount::Bases(10);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 10);
        let b = &rf.b;
        assert_eq!(b.len(), 2);

        // now we also use b not
        ro.b_mode = IntersectionMode::Not;
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 0);

        // now we also use b not
        ro.a_mode = IntersectionMode::Not;
        ro.b_mode = IntersectionMode::Not;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::Piece;
        ro.b_requirements = OverlapAmount::Bases(10);
        ro.a_requirements = OverlapAmount::Bases(10);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 10);
        let bs = &rf.b;
        assert_eq!(2, bs.len());
        assert_eq!(bs[0].try_lock().unwrap().start(), 3);
        assert_eq!(bs[0].try_lock().unwrap().stop(), 6);
        assert_eq!(bs[1].try_lock().unwrap().start(), 8);
        assert_eq!(bs[1].try_lock().unwrap().stop(), 10);

        eprintln!("{:?}", r);
    }

    #[test]
    fn test_a_not() {
        let intersections = make_example("a: 1-10\nb: 3-4, 6-7, 8-12\nb:9-20");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Not;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert!(r.is_empty());
        ro.a_requirements = OverlapAmount::Bases(6);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn test_a_piece() {
        let intersections = make_example("a: 2-23\nb: 8-12, 14-15");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Default;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::Piece;
        ro.b_piece = IntersectionPart::None;

        let r = intersections.report(&ro);
        eprintln!("{:?}", r);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].a.as_ref().unwrap().lock().start(), 8);
        assert_eq!(r[0].a.as_ref().unwrap().lock().stop(), 12);
        assert_eq!(r[1].a.as_ref().unwrap().lock().start(), 14);
        assert_eq!(r[1].a.as_ref().unwrap().lock().stop(), 15);
    }

    #[test]
    fn test_a_not_with_empty() {
        let intersections = make_example("a: 1-10");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Not;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(1, r.len());
        assert_eq!(r[0].id, usize::MAX);
    }

    #[test]
    fn test_b_inverse() {
        let intersections = make_example("a: 1-10\nb: 3-4, 6-7, 8-12\nb:9-20");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Default;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::Inverse;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 2);

        assert_eq!(r[0].b.len(), 1);
        assert_eq!(r[0].b[0].try_lock().unwrap().start(), 10);
        assert_eq!(r[0].b[0].try_lock().unwrap().stop(), 12);

        assert_eq!(r[1].b.len(), 1);
        assert_eq!(r[1].b[0].try_lock().unwrap().start(), 10);
        assert_eq!(r[1].b[0].try_lock().unwrap().stop(), 20);
    }

    #[test]
    fn test_multiple_bs() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12\nb:9-20");
        let mut ro = ReportOptions::default();
        ro.a_piece = IntersectionPart::Piece;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        eprintln!("{:?}", r[0]);
        eprintln!("{:?}", r[1]);
        eprintln!("{:?}", r[2]);
        assert_eq!(r.len(), 3); // one for each b.
        assert!(r[0].id == 0);
        assert!(r[1].id == 0);
        assert!(r[2].id == 1);
        assert_eq!(r[0].a.as_ref().unwrap().lock().start(), 3);
        assert_eq!(r[0].a.as_ref().unwrap().lock().stop(), 6);
        assert_eq!(r[0].b[0].try_lock().unwrap().start(), 3);
        assert_eq!(r[0].b[0].try_lock().unwrap().stop(), 6);

        assert_eq!(r[1].a.as_ref().unwrap().lock().start(), 8);
        assert_eq!(r[1].a.as_ref().unwrap().lock().stop(), 10);
        assert_eq!(r[1].b[0].try_lock().unwrap().start(), 8);
        assert_eq!(r[1].b[0].try_lock().unwrap().stop(), 12);

        assert_eq!(r[2].a.as_ref().unwrap().lock().start(), 9);
        assert_eq!(r[2].a.as_ref().unwrap().lock().stop(), 10);

        assert!(r[0].b.len() == 1); // need to check that only the b that overlaps this adjusted fragment is included.
    }

    #[test]
    fn test_a_pieces() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::PerPiece;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::WholeWide;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        // test that a is 1-10
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 1);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 10);
        // test that b is 3-6
        assert_eq!(rf.b.len(), 2);
        assert_eq!(rf.b[0].lock().start(), 3);
        assert_eq!(rf.b[0].lock().stop(), 6);

        assert_eq!(rf.b[1].lock().start(), 8);
        assert_eq!(rf.b[1].lock().stop(), 12);
    }

    #[test]
    fn test_a_pieces_ab_part() {
        let intersections = make_example("a: 1-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::PerPiece;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::Piece;
        ro.b_piece = IntersectionPart::Piece;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        eprintln!("r: {:?}", r);
        // a: 3-6, b: 3-6
        // a: 8-10, b: 8-10
        eprintln!("{:?}", r);
        assert_eq!(r.len(), 2);
        let rf = &r[0];
        // test that a is 3-6
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 3);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 6);
        // test that b is 3-6
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].lock().start(), 3);
        assert_eq!(rf.b[0].lock().stop(), 6);

        let rf = &r[1];
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 8);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 10);
        assert_eq!(rf.b.len(), 1);
        assert_eq!(rf.b[0].lock().start(), 8);
        assert_eq!(rf.b[0].lock().stop(), 10);
    }

    #[test]
    fn test_b_part() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Default;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::Piece;
        ro.a_requirements = OverlapAmount::Bases(5);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 0);
        ro.a_requirements = OverlapAmount::Bases(4);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        let rf = &r[0];
        assert_eq!(rf.a.as_ref().unwrap().lock().start(), 4);
        assert_eq!(rf.a.as_ref().unwrap().lock().stop(), 10);

        assert_eq!(rf.b.len(), 2);
        // note that b is chopped to 4
        assert_eq!(rf.b[0].lock().start(), 4);
        assert_eq!(rf.b[0].lock().stop(), 6);
        assert_eq!(rf.b[1].lock().start(), 8);
        // and 10.
        assert_eq!(rf.b[1].lock().stop(), 10);
    }

    #[test]
    fn test_b_none() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Default;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::WholeWide;
        ro.b_piece = IntersectionPart::None;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].a.as_ref().unwrap().lock().start(), 4);
        assert_eq!(r[0].a.as_ref().unwrap().lock().stop(), 10);
        assert_eq!(r[0].b.len(), 0);
    }

    #[test]
    fn test_a_none() {
        let intersections = make_example("a: 4-10\nb: 3-6, 8-12");
        let mut ro = ReportOptions::default();
        ro.a_mode = IntersectionMode::Default;
        ro.b_mode = IntersectionMode::Default;
        ro.a_piece = IntersectionPart::None;
        ro.b_piece = IntersectionPart::Piece;
        ro.a_requirements = OverlapAmount::Bases(1);
        ro.b_requirements = OverlapAmount::Bases(1);
        let r = intersections.report(&ro);
        assert_eq!(r.len(), 1);
        assert!(r[0].a.is_none());
        let rf = &r[0];
        // note that b is chopped to 4
        let bs = &rf.b;
        assert_eq!(bs.len(), 2);
        let b0 = &bs[0].lock();
        assert_eq!(b0.start(), 4);
        assert_eq!(b0.stop(), 6);
        let b1 = &bs[1].lock();
        assert_eq!(b1.start(), 8);
        // and 10.
        assert_eq!(b1.stop(), 10);
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

    #[test]
    fn test_calculate_overlap() {
        let intersections = make_example("a: 10-20");
        let interval_a = intersections.base_interval.clone();

        // overlapping
        let interval_b = make_example("a: 15-25").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            5
        );

        // overlapping
        let interval_b = make_example("a: 5-15").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            5
        );

        // contained
        let interval_b = make_example("a: 12-18").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            6
        );

        // contains
        let interval_b = make_example("a: 5-25").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            10
        );

        // non-overlapping
        let interval_b = make_example("a: 21-30").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            0
        );

        let interval_b = make_example("a: 1-9").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            0
        );

        // touching
        let interval_b = make_example("a: 20-30").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            0
        );

        let interval_b = make_example("a: 1-10").base_interval;
        assert_eq!(
            intersections.calculate_overlap(interval_a.clone(), interval_b),
            0
        );
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
