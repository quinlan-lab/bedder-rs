use bedder::chrom_ordering::parse_genome;
use bedder::intersection::IntersectionIterator;
use bedder::interval::Interval;
use bedder::position::{Position, Positioned, PositionedIterator};
use bedder::string::String;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use std::io;

struct Intervals {
    i: usize,
    name: String,
    n_intervals: usize,
    curr_max: f64,
    rng: rand::rngs::ThreadRng,
    interval_len: u64,
    saved_chrom: String,
}

impl Intervals {
    fn new(name: String, n_intervals: usize, interval_len: u64) -> Self {
        Intervals {
            i: 0,
            name: name,
            n_intervals,
            curr_max: 1.0,
            rng: rand::thread_rng(),
            interval_len: interval_len,
            saved_chrom: String::from("chr1"),
        }
    }
}

impl PositionedIterator for Intervals {
    fn name(&self) -> String {
        String::from(format!("{}:{}", self.name, self.i))
    }

    fn next_position(&mut self, _q: Option<&dyn Positioned>) -> Option<io::Result<Position>> {
        if self.i < self.n_intervals {
            self.i += 1;
            let r: f64 = self.rng.gen();
            self.curr_max *= r.powf(self.i as f64);
            let start = ((1.0 - self.curr_max) * (MAX_POSITION as f64)) as u64;
            Some(Ok(Position::Interval(Interval {
                chrom: self.saved_chrom.clone(),
                start: start,
                stop: start + self.interval_len,
                ..Default::default()
            })))
        } else {
            None
        }
    }
}

const MAX_POSITION: u64 = 10_000;

pub fn intersection_benchmark(c: &mut Criterion) {
    let genome_str = "chr1\nchr2\n";
    let chrom_order = parse_genome(genome_str.as_bytes()).unwrap();

    c.bench_function("simple intersection", |b| {
        b.iter_with_large_drop(|| {
            let a_ivs = Box::new(Intervals::new(String::from("a"), 100, 1000));
            let b_ivs = Box::new(Intervals::new(String::from("b"), 100_000, 100));
            let iter = IntersectionIterator::new(a_ivs, vec![b_ivs], &chrom_order)
                .expect("error getting iterator");

            iter.for_each(|intersection| {
                let intersection = intersection.expect("error getting intersection");
                black_box(intersection.overlapping);
            });
        });
    });
}

criterion_group!(benches, intersection_benchmark);
criterion_main!(benches);
