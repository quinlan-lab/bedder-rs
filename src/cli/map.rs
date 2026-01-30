use std::collections::HashMap;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::cli::shared::HELP_TEMPLATE;

/// The aggregation operation to apply to B values.
#[derive(Debug, Clone, ValueEnum)]
pub enum AggOp {
    Count,
    Sum,
    Mean,
    Min,
    Max,
    Median,
}

impl AggOp {
    /// Given a vector of f64 values, compute the aggregate.
    /// Returns "." for empty data, except Count which returns "0".
    pub fn compute(&self, values: &mut Vec<f64>) -> String {
        if values.is_empty() {
            return match self {
                AggOp::Count => "0".to_string(),
                _ => ".".to_string(),
            };
        }
        match self {
            AggOp::Count => values.len().to_string(),
            AggOp::Sum => format_number(values.iter().sum()),
            AggOp::Mean => {
                let sum: f64 = values.iter().sum();
                format_number(sum / values.len() as f64)
            }
            AggOp::Min => format_number(
                values
                    .iter()
                    .cloned()
                    .fold(f64::INFINITY, f64::min),
            ),
            AggOp::Max => format_number(
                values
                    .iter()
                    .cloned()
                    .fold(f64::NEG_INFINITY, f64::max),
            ),
            AggOp::Median => {
                values
                    .sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = values.len() / 2;
                let median = if values.len() % 2 == 0 {
                    (values[mid - 1] + values[mid]) / 2.0
                } else {
                    values[mid]
                };
                format_number(median)
            }
        }
    }
}

fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Extract a value from a B interval for aggregation.
fn extract_value(b_pos: &bedder::position::Position, operation: &AggOp, column: usize) -> Option<f64> {
    match operation {
        AggOp::Count => Some(0.0), // dummy value for counting
        _ => b_pos.column_as_f64(column),
    }
}

/// Expand columns and operations into paired (column, operation) tuples.
/// Rules (matching bedtools map):
///   - len(c) == len(o): zip them
///   - len(c) == 1: replicate c to match len(o)
///   - len(o) == 1: replicate o to match len(c)
///   - otherwise: error
fn expand_ops(columns: &[usize], operations: &[AggOp]) -> Result<Vec<(usize, AggOp)>, Box<dyn std::error::Error>> {
    match (columns.len(), operations.len()) {
        (c, o) if c == o => Ok(columns.iter().copied().zip(operations.iter().cloned()).collect()),
        (1, _) => Ok(operations.iter().cloned().map(|op| (columns[0], op)).collect()),
        (_, 1) => Ok(columns.iter().copied().map(|col| (col, operations[0].clone())).collect()),
        (c, o) => Err(format!(
            "number of columns ({}) and operations ({}) must match, or one must be 1",
            c, o
        ).into()),
    }
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Map operation: aggregate values from overlapping B intervals for each A interval.",
    long_about = None,
    rename_all = "kebab-case",
    help_template = HELP_TEMPLATE,
    arg_required_else_help = true,
    after_long_help = "\
EXAMPLES:
    Given tests/map_a.bed (A):
        chr1\t100\t200\tgeneA\t10
        chr1\t300\t400\tgeneB\t20

    And tests/map_b.bed (B):
        chr1\t120\t180\tgeneA\t5
        chr1\t130\t170\tgeneB\t7
        chr1\t150\t190\tgeneA\t3
        chr1\t350\t380\tgeneB\t4

    1. Sum scores of all overlapping B intervals (default):

        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai
        chr1\t100\t200\tgeneA\t10\t15
        chr1\t300\t400\tgeneB\t20\t4

        All 3 B intervals overlapping geneA are summed (5+7+3=15).

    2. Only aggregate B intervals whose name matches A (-n):

        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai -n
        chr1\t100\t200\tgeneA\t10\t8
        chr1\t300\t400\tgeneB\t20\t4

        For geneA only geneA-named B intervals are used (5+3=8).
        The geneB-named B interval (score 7) is excluded.

    3. Group overlapping B by B's name (-G), one row per group:

        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai -G
        chr1\t100\t200\tgeneA\t10\tgeneA\t8
        chr1\t100\t200\tgeneA\t10\tgeneB\t7
        chr1\t300\t400\tgeneB\t20\tgeneB\t4

        geneA's overlapping B intervals are split into two groups.

    4. Multiple operations on the same column:

        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai -c 5 -O sum,mean,count
        chr1\t100\t200\tgeneA\t10\t15\t5\t3
        chr1\t300\t400\tgeneB\t20\t4\t4\t1

    5. Combine -G with -n:

        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai -G -n -O sum,count
        chr1\t100\t200\tgeneA\t10\tgeneA\t8\t2
        chr1\t300\t400\tgeneB\t20\tgeneB\t4\t1

        Groups by B name, but only keeps groups matching A's name."
)]
pub struct MapCmdArgs {
    #[arg(help = "input A file (query)", short = 'a')]
    pub query_path: PathBuf,

    #[arg(help = "input B file (database)", short = 'b')]
    pub other_path: PathBuf,

    #[arg(
        help = "genome file for chromosome ordering",
        short = 'g',
        long = "genome",
        required = true
    )]
    pub genome_file: PathBuf,

    #[arg(
        help = "1-indexed column(s) of B to aggregate (default: 5 = score). Comma-separated for multiple.",
        short = 'c',
        long = "column",
        default_value = "5",
        value_delimiter = ','
    )]
    pub columns: Vec<usize>,

    #[arg(
        help = "aggregation operation(s) to apply. Comma-separated for multiple.",
        short = 'O',
        long = "operation",
        default_value = "sum",
        value_delimiter = ',',
        value_enum
    )]
    pub operations: Vec<AggOp>,

    #[arg(
        help = "output file (default: stdout)",
        short = 'o',
        long = "output",
        default_value = "-"
    )]
    pub output_path: PathBuf,

    #[arg(
        short = 'G',
        long = "group-by-b",
        help = "For each A, group its overlapping B intervals by name, then summarize each group separately."
    )]
    pub group_by_b: bool,

    #[arg(
        short = 'n',
        long = "name-match",
        help = "Only summarize B intervals whose name matches A's name."
    )]
    pub name_match: bool,
}

pub fn map_command(args: MapCmdArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ops = expand_ops(&args.columns, &args.operations)?;

    let chrom_order =
        bedder::chrom_ordering::parse_genome(std::fs::File::open(&args.genome_file)?)?;

    let a_file = std::io::BufReader::new(std::fs::File::open(&args.query_path)?);
    let (a_reader, a_file_type) = bedder::sniff::open(a_file, &args.query_path)?;

    if !matches!(a_file_type, bedder::sniff::FileType::Bed) {
        return Err("map currently only supports BED files for -a".into());
    }

    let a_iter = a_reader.into_positioned_iterator();

    let b_file = std::io::BufReader::new(std::fs::File::open(&args.other_path)?);
    let (b_reader, b_file_type) = bedder::sniff::open(b_file, &args.other_path)?;

    if !matches!(b_file_type, bedder::sniff::FileType::Bed) {
        return Err("map currently only supports BED files for -b".into());
    }

    let b_iter = b_reader.into_positioned_iterator();

    let ii = bedder::intersection::IntersectionIterator::new(
        a_iter,
        vec![b_iter],
        &chrom_order,
        -1,    // max_distance: not used
        -1,    // n_closest: not used
        false, // can_skip_ahead: need every A interval reported
    )?;

    let output_path = if args.output_path.to_str() == Some("-") {
        "/dev/stdout"
    } else {
        args.output_path.to_str().unwrap()
    };
    let mut bed_writer = bedder::bedder_bed::simplebed::BedWriter::new(output_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    for intersection_result in ii {
        let intersection = intersection_result?;
        let base = intersection
            .base_interval
            .try_lock()
            .expect("failed to lock base_interval");

        let a_name: Option<String> = base.name().map(|s| s.to_string());

        let bed_record = match &*base {
            bedder::position::Position::Bed(bed) => &bed.0,
            _ => return Err("map only supports BED input".into()),
        };

        if args.group_by_b {
            // Group overlapping B intervals by B name.
            // Each group accumulates one Vec<f64> per operation.
            let mut groups: HashMap<String, Vec<Vec<f64>>> = HashMap::new();
            let mut insertion_order: Vec<String> = Vec::new();

            for overlap in &intersection.overlapping {
                let b_pos = overlap
                    .interval
                    .try_lock()
                    .expect("failed to lock b interval");

                let b_name = b_pos.name().unwrap_or(".").to_string();

                if args.name_match {
                    if let Some(ref an) = a_name {
                        if b_name != *an {
                            continue;
                        }
                    }
                }

                if !groups.contains_key(&b_name) {
                    insertion_order.push(b_name.clone());
                    groups.insert(b_name.clone(), vec![Vec::new(); ops.len()]);
                }
                let vecs = groups.get_mut(&b_name).unwrap();
                for (i, (col, op)) in ops.iter().enumerate() {
                    if let Some(val) = extract_value(&b_pos, op, *col) {
                        vecs[i].push(val);
                    }
                }
            }

            if groups.is_empty() {
                let mut record = bed_record.clone();
                record.push_field(bedder::bedder_bed::BedValue::String(".".to_string()));
                for (_, op) in &ops {
                    record.push_field(bedder::bedder_bed::BedValue::String(
                        op.compute(&mut Vec::new()),
                    ));
                }
                bed_writer.write_record(&record).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                })?;
            } else {
                for b_name in &insertion_order {
                    let vecs = groups.get_mut(b_name).unwrap();
                    let mut record = bed_record.clone();
                    record.push_field(bedder::bedder_bed::BedValue::String(b_name.clone()));
                    for (i, (_, op)) in ops.iter().enumerate() {
                        let agg_result = op.compute(&mut vecs[i]);
                        record.push_field(bedder::bedder_bed::BedValue::String(agg_result));
                    }
                    bed_writer.write_record(&record).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                }
            }
        } else {
            // Standard path: one row per A interval with one aggregate column per op
            let mut value_vecs: Vec<Vec<f64>> = vec![Vec::new(); ops.len()];

            for overlap in &intersection.overlapping {
                let b_pos = overlap
                    .interval
                    .try_lock()
                    .expect("failed to lock b interval");

                if args.name_match {
                    if let Some(ref an) = a_name {
                        if let Some(bn) = b_pos.name() {
                            if bn != an.as_str() {
                                continue;
                            }
                        }
                        // B has no name → include it
                    }
                }

                for (i, (col, op)) in ops.iter().enumerate() {
                    if let Some(val) = extract_value(&b_pos, op, *col) {
                        value_vecs[i].push(val);
                    }
                }
            }

            let mut record = bed_record.clone();
            for (i, (_, op)) in ops.iter().enumerate() {
                let agg_result = op.compute(&mut value_vecs[i]);
                record.push_field(bedder::bedder_bed::BedValue::String(agg_result));
            }
            bed_writer.write_record(&record).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?;
        }
    }

    bed_writer.flush().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agg_count() {
        let mut values = vec![1.0, 2.0, 3.0];
        assert_eq!(AggOp::Count.compute(&mut values), "3");
    }

    #[test]
    fn test_agg_count_empty() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(AggOp::Count.compute(&mut values), "0");
    }

    #[test]
    fn test_agg_sum() {
        let mut values = vec![1.0, 2.0, 3.0];
        assert_eq!(AggOp::Sum.compute(&mut values), "6");
    }

    #[test]
    fn test_agg_sum_empty() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(AggOp::Sum.compute(&mut values), ".");
    }

    #[test]
    fn test_agg_mean() {
        let mut values = vec![1.0, 2.0, 3.0];
        assert_eq!(AggOp::Mean.compute(&mut values), "2");
    }

    #[test]
    fn test_agg_min() {
        let mut values = vec![3.0, 1.0, 2.0];
        assert_eq!(AggOp::Min.compute(&mut values), "1");
    }

    #[test]
    fn test_agg_max() {
        let mut values = vec![3.0, 1.0, 2.0];
        assert_eq!(AggOp::Max.compute(&mut values), "3");
    }

    #[test]
    fn test_agg_median_odd() {
        let mut values = vec![3.0, 1.0, 2.0];
        assert_eq!(AggOp::Median.compute(&mut values), "2");
    }

    #[test]
    fn test_agg_median_even() {
        let mut values = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(AggOp::Median.compute(&mut values), "2.5");
    }

    #[test]
    fn test_agg_mean_empty() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(AggOp::Mean.compute(&mut values), ".");
    }

    #[test]
    fn test_format_number_integer() {
        assert_eq!(format_number(42.0), "42");
        assert_eq!(format_number(-3.0), "-3");
        assert_eq!(format_number(0.0), "0");
    }

    #[test]
    fn test_format_number_float() {
        assert_eq!(format_number(2.5), "2.5");
        assert_eq!(format_number(1.333), "1.333");
    }
}
