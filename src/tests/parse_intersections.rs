use crate::intersection::{Intersection, Intersections};
use crate::interval::Interval;
use crate::position::Position;
use crate::string::String;
use linear_map::LinearMap;
use std::sync::Arc;

pub(crate) fn parse_intersections(input: &str) -> Intersections {
    let mut intersections = Vec::new();
    let mut base_interval = None;

    let mut id = 0;
    for line in input.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let mut parts = line.split(':');

        if let (Some(name), Some(ranges)) = (parts.next(), parts.next()) {
            let name = name.trim();

            let ranges: Vec<(u64, u64)> = ranges
                .split(',')
                .map(|range| {
                    let range = range.trim();
                    let mut range_parts = range.split('-');

                    if let (Some(start), Some(end)) = (range_parts.next(), range_parts.next()) {
                        if let (Ok(start), Ok(end)) = (start.parse(), end.parse()) {
                            return (start, end);
                        }
                    }

                    panic!("Invalid range format: {}", range);
                })
                .collect();

            if name == "a" {
                assert_eq!(ranges.len(), 1);
                let interval = Interval {
                    chrom: String::from("chr1"),
                    start: ranges[0].0,
                    stop: ranges[0].1,
                    fields: LinearMap::new(),
                };
                base_interval = Some(interval);
            } else {
                id += 1;
                for se in ranges {
                    let interval = Interval {
                        chrom: String::from("chr1"),
                        start: se.0,
                        stop: se.1,
                        fields: LinearMap::new(),
                    };

                    intersections.push(Intersection {
                        interval: Arc::new(Position::Interval(interval)),
                        id: id - 1,
                    });
                }
            }
        }
    }

    let base_interval = base_interval.expect("No base interval found");

    Intersections {
        base_interval: Arc::new(Position::Interval(base_interval)),
        overlapping: intersections,
    }
}

#[test]
fn test_parse() {
    let input = "a: 1-10\nb: 3-6, 8-12\nb:9-20";
    let intersections = parse_intersections(input);

    // Access the generated Intersections struct
    assert_eq!(intersections.base_interval.start(), 1);
    assert_eq!(intersections.base_interval.stop(), 10);
    eprintln!("{:?}", intersections.overlapping);
    assert_eq!(intersections.overlapping.len(), 3);
    assert_eq!(intersections.overlapping[0].interval.start(), 3);
    assert_eq!(intersections.overlapping[0].interval.stop(), 6);
    assert_eq!(intersections.overlapping[1].interval.start(), 8);
    assert_eq!(intersections.overlapping[1].interval.stop(), 12);
    assert_eq!(intersections.overlapping[0].id, 0);
    assert_eq!(intersections.overlapping[1].id, 0);

    assert_eq!(intersections.overlapping[2].id, 1);
    assert_eq!(intersections.overlapping[2].interval.start(), 9);
    assert_eq!(intersections.overlapping[2].interval.stop(), 20);
}
