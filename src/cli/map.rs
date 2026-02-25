use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use pyo3::prelude::*;

use crate::cli::shared::HELP_TEMPLATE;

/// The aggregation operation to apply to B values.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum AggOp {
    Count,
    Sum,
    Mean,
    Min,
    Max,
    Median,
}

impl AggOp {
    /// Given a slice of f64 values, compute the aggregate.
    /// Returns "." for empty data, except Count which returns "0".
    pub fn compute(&self, values: &[f64]) -> String {
        if values.is_empty() {
            return match self {
                AggOp::Count => "0".to_string(),
                _ => ".".to_string(),
            };
        }
        match self {
            AggOp::Count => values.len().to_string(),
            AggOp::Sum => bedder::formatting::format_map_number(values.iter().sum()),
            AggOp::Mean => {
                let sum: f64 = values.iter().sum();
                bedder::formatting::format_map_number(sum / values.len() as f64)
            }
            AggOp::Min => bedder::formatting::format_map_number(
                values.iter().cloned().fold(f64::INFINITY, f64::min),
            ),
            AggOp::Max => bedder::formatting::format_map_number(
                values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            ),
            AggOp::Median => {
                let mut sorted = values.to_vec();
                sorted.sort_unstable_by(|a, b| a.total_cmp(b));
                let mid = sorted.len() / 2;
                let median = if sorted.len().is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                };
                bedder::formatting::format_map_number(median)
            }
        }
    }
}

/// Operation spec accepted by CLI: built-ins or `py:<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapOpSpec {
    Builtin(AggOp),
    Python(String),
}

impl std::str::FromStr for MapOpSpec {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if let Some(name) = raw.strip_prefix("py:") {
            if name.trim().is_empty() {
                return Err("python operation name cannot be empty after 'py:'".to_string());
            }
            return Ok(MapOpSpec::Python(name.to_string()));
        }

        let op = <AggOp as ValueEnum>::from_str(raw, true).map_err(|_| {
            format!(
                "invalid operation '{}'; expected one of count,sum,mean,min,max,median or py:<name>",
                raw
            )
        })?;
        Ok(MapOpSpec::Builtin(op))
    }
}

/// Value source spec accepted by CLI for `-c/--column`.
/// Supports BED columns or python extractors (`py:<name>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapValueSelector {
    BedColumn(usize),
    PythonExtractor(String),
}

impl std::str::FromStr for MapValueSelector {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if let Some(name) = raw.strip_prefix("py:") {
            if name.trim().is_empty() {
                return Err("python extractor name cannot be empty after 'py:'".to_string());
            }
            return Ok(MapValueSelector::PythonExtractor(name.to_string()));
        }

        let col = raw.parse::<usize>().map_err(|_| {
            format!(
                "invalid column selector '{}'; expected integer BED column or py:<name>",
                raw
            )
        })?;
        if col == 0 {
            return Err("column index must be >= 1 (columns are 1-indexed)".to_string());
        }
        Ok(MapValueSelector::BedColumn(col))
    }
}

#[derive(Debug)]
enum RuntimeAggOp<'py> {
    Builtin(AggOp),
    Python(bedder::py::CompiledMapPython<'py>),
}

#[derive(Debug)]
enum RuntimeValueSelector<'py> {
    BedColumn(usize),
    PythonExtractor(bedder::py::CompiledMapValuePython<'py>),
}

/// Extract a value from a B interval for aggregation.
/// Logs a warning (once per column) when a non-numeric value is encountered.
fn extract_value(
    b_pos: &bedder::position::Position,
    operation: &RuntimeAggOp<'_>,
    selector: &RuntimeValueSelector<'_>,
    warned_columns: &mut HashSet<usize>,
) -> Result<Option<f64>, Box<dyn std::error::Error>> {
    match operation {
        // Count follows map semantics: it counts overlaps regardless of selector/extractor value.
        RuntimeAggOp::Builtin(AggOp::Count) => Ok(Some(0.0)),
        _ => match selector {
            RuntimeValueSelector::BedColumn(column) => {
                let val = b_pos.column_as_f64(*column);
                if val.is_none() && warned_columns.insert(*column) {
                    log::warn!("Non-numeric value in column {}.", column);
                }
                Ok(val)
            }
            RuntimeValueSelector::PythonExtractor(extractor) => {
                extractor.eval_position(b_pos).map_err(|e| {
                    std::io::Error::other(format!(
                        "python column extractor 'py:{}' failed: {}",
                        extractor.function_name(),
                        e
                    ))
                    .into()
                })
            }
        },
    }
}

/// Expand columns and operations into paired (column, operation) tuples.
/// Rules (matching bedtools map):
///   - len(c) == len(o): zip them
///   - len(c) == 1: replicate c to match len(o)
///   - len(o) == 1: replicate o to match len(c)
///   - otherwise: error
fn expand_ops<C: Clone, O: Clone>(
    columns: &[C],
    operations: &[O],
) -> Result<Vec<(C, O)>, Box<dyn std::error::Error>> {
    match (columns.len(), operations.len()) {
        (c, o) if c == o => Ok(columns
            .iter()
            .cloned()
            .zip(operations.iter().cloned())
            .collect()),
        (1, _) => Ok(operations
            .iter()
            .cloned()
            .map(|op| (columns[0].clone(), op))
            .collect()),
        (_, 1) => Ok(columns
            .iter()
            .cloned()
            .map(|col| (col, operations[0].clone()))
            .collect()),
        (c, o) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "number of columns ({}) and operations ({}) must match, or one must be 1",
                c, o
            ),
        )
        .into()),
    }
}

fn compute_operation(
    operation: &RuntimeAggOp<'_>,
    values: &[f64],
) -> Result<String, Box<dyn std::error::Error>> {
    match operation {
        RuntimeAggOp::Builtin(op) => Ok(op.compute(values)),
        RuntimeAggOp::Python(op) => op.eval_values(values).map_err(|e| {
            std::io::Error::other(format!(
                "python operation 'py:{}' failed: {}",
                op.function_name(),
                e
            ))
            .into()
        }),
    }
}

fn load_python_functions<'py>(
    py: Python<'py>,
    python_file: Option<&PathBuf>,
) -> Result<HashMap<String, bedder::py::PythonFunction<'py>>, Box<dyn std::error::Error>> {
    let python_file = python_file.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "python operations/extractors require --python <file>",
        )
    })?;

    bedder::py::initialize_python(py)?;
    let code = std::fs::read_to_string(python_file)?;
    let c_code = CString::new(code).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "python file '{}' contains a NUL byte",
                python_file.display()
            ),
        )
    })?;
    py.run(&c_code, None, None)?;
    let main_module = py.import("__main__")?;
    let globals_for_columns = main_module.dict();
    bedder::py::introspect_python_functions(py, globals_for_columns).map_err(Into::into)
}

fn compile_selector<'py>(
    selector: &MapValueSelector,
    functions_map: Option<&HashMap<String, bedder::py::PythonFunction<'py>>>,
) -> Result<RuntimeValueSelector<'py>, Box<dyn std::error::Error>> {
    match selector {
        MapValueSelector::BedColumn(col) => Ok(RuntimeValueSelector::BedColumn(*col)),
        MapValueSelector::PythonExtractor(name) => {
            if let Some(functions_map) = functions_map {
                let extractor = bedder::py::CompiledMapValuePython::new(name, functions_map)
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "failed to compile python column extractor 'py:{}': {}",
                                name, e
                            ),
                        )
                    })?;
                Ok(RuntimeValueSelector::PythonExtractor(extractor))
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "python column extractors require --python <file>; missing for -c py:{}",
                        name
                    ),
                )
                .into())
            }
        }
    }
}

fn compile_builtin_ops(
    specs: &[(MapValueSelector, MapOpSpec)],
) -> Result<Vec<(RuntimeValueSelector<'static>, RuntimeAggOp<'static>)>, Box<dyn std::error::Error>>
{
    let mut compiled = Vec::with_capacity(specs.len());
    for (selector, spec) in specs {
        let compiled_selector: RuntimeValueSelector<'static> = compile_selector(selector, None)?;
        match spec {
            MapOpSpec::Builtin(op) => {
                compiled.push((compiled_selector, RuntimeAggOp::Builtin(op.clone())))
            }
            MapOpSpec::Python(name) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "python operations require --python <file>; missing for py:{}",
                        name
                    ),
                )
                .into());
            }
        }
    }
    Ok(compiled)
}

fn compile_python_ops<'py>(
    py: Python<'py>,
    specs: &[(MapValueSelector, MapOpSpec)],
    python_file: Option<&PathBuf>,
) -> Result<Vec<(RuntimeValueSelector<'py>, RuntimeAggOp<'py>)>, Box<dyn std::error::Error>> {
    let functions_map = load_python_functions(py, python_file)?;

    let mut compiled = Vec::with_capacity(specs.len());
    for (selector, spec) in specs {
        let compiled_selector = compile_selector(selector, Some(&functions_map))?;
        match spec {
            MapOpSpec::Builtin(op) => {
                compiled.push((compiled_selector, RuntimeAggOp::Builtin(op.clone())))
            }
            MapOpSpec::Python(name) => {
                let map_op =
                    bedder::py::CompiledMapPython::new(name, &functions_map).map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("failed to compile python operation 'py:{}': {}", name, e),
                        )
                    })?;
                compiled.push((compiled_selector, RuntimeAggOp::Python(map_op)));
            }
        }
    }

    Ok(compiled)
}

fn run_map_with_ops<'a, 'py>(
    ii: bedder::intersection::IntersectionIterator<'a>,
    args: &MapCmdArgs,
    ops: &[(RuntimeValueSelector<'py>, RuntimeAggOp<'py>)],
    bed_writer: &mut bedder::bedder_bed::simplebed::BedWriter,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut warned_columns: HashSet<usize> = HashSet::new();

    for intersection_result in ii {
        let intersection = intersection_result?;
        let base = intersection
            .base_interval
            .try_lock()
            .expect("failed to lock base_interval");

        let a_name: String = base.name().unwrap_or(".").to_string();

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

                if args.name_match && b_name != a_name {
                    continue;
                }

                // Keep first-seen order deterministic for stable output across runs.
                if !groups.contains_key(&b_name) {
                    insertion_order.push(b_name.clone());
                    groups.insert(b_name.clone(), vec![Vec::new(); ops.len()]);
                }
                let vecs = groups.get_mut(&b_name).expect("group key inserted above");
                for (i, (selector, op)) in ops.iter().enumerate() {
                    if let Some(val) = extract_value(&b_pos, op, selector, &mut warned_columns)? {
                        vecs[i].push(val);
                    }
                }
            }

            if groups.is_empty() {
                let mut record = bed_record.clone();
                record.push_field(bedder::bedder_bed::BedValue::String(".".to_string()));
                for (_, op) in ops {
                    record.push_field(bedder::bedder_bed::BedValue::String(compute_operation(
                        op,
                        &[],
                    )?));
                }
                bed_writer.write_record(&record)?;
            } else {
                for b_name in &insertion_order {
                    let vecs = groups.get_mut(b_name).expect("group key exists");
                    let mut record = bed_record.clone();
                    record.push_field(bedder::bedder_bed::BedValue::String(b_name.clone()));
                    for (i, (_, op)) in ops.iter().enumerate() {
                        let agg_result = compute_operation(op, &vecs[i])?;
                        record.push_field(bedder::bedder_bed::BedValue::String(agg_result));
                    }
                    bed_writer.write_record(&record)?;
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
                    let b_name = b_pos.name().unwrap_or(".");
                    if b_name != a_name {
                        continue;
                    }
                }

                for (i, (selector, op)) in ops.iter().enumerate() {
                    if let Some(val) = extract_value(&b_pos, op, selector, &mut warned_columns)? {
                        value_vecs[i].push(val);
                    }
                }
            }

            let mut record = bed_record.clone();
            for (i, (_, op)) in ops.iter().enumerate() {
                let agg_result = compute_operation(op, &value_vecs[i])?;
                record.push_field(bedder::bedder_bed::BedValue::String(agg_result));
            }
            bed_writer.write_record(&record)?;
        }
    }

    bed_writer.flush()?;
    Ok(())
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

        Groups by B name, but only keeps groups matching A's name.

    6. Python operation on mapped values:

        # tests/map_ops.py defines: def bedder_sum_plus_one(values) -> float
        $ bedder map -a tests/map_a.bed -b tests/map_b.bed -g tests/hg38.small.fai --python tests/map_ops.py -O py:sum_plus_one
        chr1\t100\t200\tgeneA\t10\t16
        chr1\t300\t400\tgeneB\t20\t5

    7. Python value extraction from mapped VCF records:

        # tests/map_ops.py defines: def bedder_vcf_dp(iv) -> float
        $ bedder map -a tests/map_a.bed -b tests/map_b.vcf -g tests/hg38.small.fai --python tests/map_ops.py -c py:vcf_dp -O sum,mean,count
        chr1\t100\t200\tgeneA\t10\t15\t5\t3
        chr1\t300\t400\tgeneB\t20\t4\t4\t1"
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
        help = "Value selector(s) for mapped B intervals. Use 1-indexed BED column(s) (default: 5 = score) or py:<name> extractors. Comma-separated for multiple.",
        short = 'c',
        long = "column",
        default_value = "5",
        value_delimiter = ','
    )]
    pub columns: Vec<MapValueSelector>,

    #[arg(
        help = "aggregation operation(s): count,sum,mean,min,max,median, or py:<name>. Comma-separated for multiple.",
        short = 'O',
        long = "operation",
        default_value = "sum",
        value_delimiter = ','
    )]
    pub operations: Vec<MapOpSpec>,

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

    #[arg(
        long = "python",
        help = "python file with bedder_<name> functions used by py:<name> operations"
    )]
    pub python_file: Option<PathBuf>,
}

pub fn map_command(args: MapCmdArgs) -> Result<(), Box<dyn std::error::Error>> {
    let ops = expand_ops(&args.columns, &args.operations)?;
    let has_python_specs = ops.iter().any(|(selector, op)| {
        matches!(selector, MapValueSelector::PythonExtractor(_))
            || matches!(op, MapOpSpec::Python(_))
    });

    let chrom_order =
        bedder::chrom_ordering::parse_genome(std::fs::File::open(&args.genome_file)?)?;

    let a_file = std::io::BufReader::new(std::fs::File::open(&args.query_path)?);
    let (a_reader, a_file_type) = bedder::sniff::open(a_file, &args.query_path)?;

    if !matches!(a_file_type, bedder::sniff::FileType::Bed) {
        return Err("map currently only supports BED files for -a (output is BED-based)".into());
    }

    let a_iter = a_reader.into_positioned_iterator();

    let b_file = std::io::BufReader::new(std::fs::File::open(&args.other_path)?);
    let (b_reader, _b_file_type) = bedder::sniff::open(b_file, &args.other_path)?;

    let b_iter = b_reader.into_positioned_iterator();

    let ii = bedder::intersection::IntersectionIterator::new(
        a_iter,
        vec![b_iter],
        &chrom_order,
        -1,    // max_distance: not used
        -1,    // n_closest: not used
        false, // can_skip_ahead: need every A interval reported
    )?;

    let mut bed_writer = if args.output_path.to_str() == Some("-") {
        bedder::bedder_bed::simplebed::BedWriter::from_writer(Box::new(std::io::BufWriter::new(
            std::io::stdout(),
        )))?
    } else {
        bedder::bedder_bed::simplebed::BedWriter::new(&args.output_path)?
    };

    if has_python_specs {
        Python::initialize();
        Python::attach(|py| -> Result<(), Box<dyn std::error::Error>> {
            // Compile Python callables once and reuse per row to avoid per-record Python lookup cost.
            let compiled_ops = compile_python_ops(py, &ops, args.python_file.as_ref())?;
            run_map_with_ops(ii, &args, &compiled_ops, &mut bed_writer)
        })?;
    } else {
        let compiled_ops = compile_builtin_ops(&ops)?;
        run_map_with_ops(ii, &args, &compiled_ops, &mut bed_writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agg_ops() {
        let vals = vec![3.0, 1.0, 2.0];
        assert_eq!(AggOp::Count.compute(&vals), "3");
        assert_eq!(AggOp::Sum.compute(&vals), "6");
        assert_eq!(AggOp::Mean.compute(&vals), "2");
        assert_eq!(AggOp::Min.compute(&vals), "1");
        assert_eq!(AggOp::Max.compute(&vals), "3");
        assert_eq!(AggOp::Median.compute(&vals), "2");

        // single-element median
        assert_eq!(AggOp::Median.compute(&[42.0]), "42");

        // even-length median
        assert_eq!(AggOp::Median.compute(&[1.0, 2.0, 3.0, 4.0]), "2.5");

        // empty: count returns "0", others return "."
        assert_eq!(AggOp::Count.compute(&[]), "0");
        assert_eq!(AggOp::Sum.compute(&[]), ".");
        assert_eq!(AggOp::Mean.compute(&[]), ".");
        assert_eq!(AggOp::Min.compute(&[]), ".");
        assert_eq!(AggOp::Max.compute(&[]), ".");
        assert_eq!(AggOp::Median.compute(&[]), ".");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(bedder::formatting::format_map_number(42.0), "42");
        assert_eq!(bedder::formatting::format_map_number(-3.0), "-3");
        assert_eq!(bedder::formatting::format_map_number(0.0), "0");
        assert_eq!(bedder::formatting::format_map_number(2.5), "2.5");
        assert_eq!(bedder::formatting::format_map_number(1.333), "1.333");
    }

    #[test]
    fn test_expand_ops() {
        // zip when equal length
        let ops = expand_ops(
            &[
                MapValueSelector::BedColumn(4),
                MapValueSelector::BedColumn(5),
            ],
            &[AggOp::Sum, AggOp::Mean],
        )
        .unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].0, MapValueSelector::BedColumn(4));
        assert_eq!(ops[1].0, MapValueSelector::BedColumn(5));

        // replicate single column
        let ops = expand_ops(
            &[MapValueSelector::BedColumn(5)],
            &[AggOp::Sum, AggOp::Count],
        )
        .unwrap();
        assert_eq!(ops.len(), 2);
        assert!(ops
            .iter()
            .all(|(c, _)| *c == MapValueSelector::BedColumn(5)));

        // replicate single operation
        let ops = expand_ops(
            &[
                MapValueSelector::BedColumn(4),
                MapValueSelector::BedColumn(5),
            ],
            &[AggOp::Sum],
        )
        .unwrap();
        assert_eq!(ops.len(), 2);

        // mismatch is an error
        assert!(expand_ops(
            &[
                MapValueSelector::BedColumn(4),
                MapValueSelector::BedColumn(5)
            ],
            &[AggOp::Sum, AggOp::Mean, AggOp::Count]
        )
        .is_err());
    }

    #[test]
    fn test_parse_map_value_selector() {
        assert_eq!(
            "5".parse::<MapValueSelector>().unwrap(),
            MapValueSelector::BedColumn(5)
        );
        assert_eq!(
            "py:dp".parse::<MapValueSelector>().unwrap(),
            MapValueSelector::PythonExtractor("dp".to_string())
        );
        assert!("0".parse::<MapValueSelector>().is_err());
        assert!("py:".parse::<MapValueSelector>().is_err());
        assert!("nope".parse::<MapValueSelector>().is_err());
    }

    #[test]
    fn test_parse_map_op_spec() {
        assert_eq!(
            "sum".parse::<MapOpSpec>().unwrap(),
            MapOpSpec::Builtin(AggOp::Sum)
        );
        assert_eq!(
            "py:myop".parse::<MapOpSpec>().unwrap(),
            MapOpSpec::Python("myop".to_string())
        );
        assert!("py:".parse::<MapOpSpec>().is_err());
        assert!("not-an-op".parse::<MapOpSpec>().is_err());
    }
}
