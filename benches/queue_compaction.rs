//! Deterministic streaming workloads for comparing queue implementations.
//! Build with `cargo bench --bench queue_compaction --no-run`, then run the
//! executable with a workload name and record count. Use /usr/bin/time for RSS.
use bedder::chrom_ordering::parse_genome;
use bedder::intersection::IntersectionIterator;
use bedder::interval::Interval;
use bedder::position::{Position, PositionedIterator};
use bedder::string::String;
use std::{io, time::Instant};

// Match the CLI's allocator when the default allocator feature is enabled.
#[cfg(feature = "mimalloc_allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Intervals {
    index: u64,
    count: u64,
    length: u64,
    long_first: bool,
}

impl PositionedIterator for Intervals {
    fn name(&self) -> String {
        String::from("synthetic")
    }

    fn next_position(&mut self, _: Option<&Position>) -> Option<io::Result<Position>> {
        let (start, stop) = if self.long_first {
            self.long_first = false;
            (0, (self.count + 1) * 10)
        } else {
            if self.index == self.count {
                return None;
            }
            self.index += 1;
            let start = self.index * 10;
            (start, start + self.length)
        };
        Some(Ok(Position::Interval(Interval {
            chrom: String::from("chr1"),
            start,
            stop,
            ..Default::default()
        })))
    }
}

fn run(workload: &str, count: u64) {
    assert!(count > 0);
    let (length, long_first, distance, closest) = match workload {
        "sparse" => (1, false, 0, 0),
        "dense" => (1000, false, 0, 0),
        "nested" => (1, true, 0, 0),
        "all-live" => ((count + 1) * 10, false, 0, 0),
        "distance" => (1, false, 25, 0),
        "closest" => (1, false, 0, 3),
        _ => panic!("unknown workload: {workload}"),
    };
    let chromosomes = parse_genome("chr1\n".as_bytes()).unwrap();
    let make_intervals = |length, long_first| {
        Box::new(Intervals {
            index: 0,
            count,
            length,
            long_first,
        })
    };
    let start = Instant::now();
    let iter = IntersectionIterator::new(
        make_intervals(1, false),
        vec![make_intervals(length, long_first)],
        &chromosomes,
        distance,
        closest,
        false,
    )
    .unwrap();
    let mut queries = 0u64;
    let mut hits = 0u64;
    for result in iter {
        let result = result.unwrap();
        queries += 1;
        hits += std::hint::black_box(result.overlapping.len()) as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    assert_eq!(queries, count);
    match workload {
        "sparse" => assert_eq!(hits, count),
        "nested" => assert_eq!(hits, 2 * count),
        "dense" => assert_eq!(hits, (1..=count).map(|i| i.min(100)).sum::<u64>()),
        "all-live" => assert_eq!(hits, count * (count + 1) / 2),
        _ => {}
    }
    println!("{workload},{count},{elapsed:.9},{hits}");
}

fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|s| s != "--bench")
        .collect();
    if args.is_empty() {
        for (workload, count) in [
            ("sparse", 1_000_000),
            ("dense", 50_000),
            ("nested", 20_000),
            ("nested", 40_000),
            ("all-live", 5_000),
            ("distance", 100_000),
            ("closest", 5_000),
        ] {
            run(workload, count);
        }
    } else {
        assert_eq!(args.len(), 2, "usage: queue_compaction WORKLOAD COUNT");
        run(&args[0], args[1].parse().expect("invalid count"));
    }
}
