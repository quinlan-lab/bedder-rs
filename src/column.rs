use crate::intersection::Intersections;
use crate::py::CompiledPython;
use crate::report_options::ReportOptions;
use std::sync::Arc;
#[derive(Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    String(String),
    Flag(bool),
    VecInt(Vec<i32>),
    VecFloat(Vec<f32>),
    VecString(Vec<String>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Integer,
    Float,
    Character,
    String,
    Flag,
}

/// The number of Values to expect (similar to Number attribute in VCF INFO/FMT fields)
#[derive(Debug, PartialEq, Eq)]
pub enum Number {
    Not,
    One,
    R,
    A,
    Dot,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ColumnError {
    InvalidValue(String),
    InvalidType(String),
    InvalidNumber(String),
    InvalidValueParser(String),
    PythonError(String),
}

impl std::fmt::Display for ColumnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnError::InvalidValue(s) => write!(f, "Invalid value: {}", s),
            ColumnError::InvalidType(s) => write!(f, "Invalid type: {}", s),
            ColumnError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            ColumnError::InvalidValueParser(s) => write!(f, "Invalid value parser: {}", s),
            ColumnError::PythonError(s) => write!(f, "Python error: {}", s),
        }
    }
}

impl std::error::Error for ColumnError {}

/// A ColumnReporter tells bedder how to report a column in the output.
pub trait ColumnReporter {
    /// report the name, e.g. `count` for the INFO field of the VCF
    fn name(&self) -> &str;
    /// report the type, for the INFO field of the VCF
    fn ftype(&self) -> &Type; // Type is some enum from noodles or here that limits to relevant types
    fn description(&self) -> &str;
    fn number(&self) -> &Number;

    fn value(
        &self,
        r: &Intersections,
        report_options: Arc<ReportOptions>,
    ) -> Result<Value, ColumnError>;
}

pub enum ValueParser {
    PythonExpression(String),
    LuaExpression(String),
    // bool indicates whether we should use the intersection.report() constraints or just the count of overlaps
    Count(bool),
    Sum,
    Bases,
    ChromStartEnd,
    OriginalInterval,
}
pub struct Column<'py> {
    // name is also used as the header in the output
    name: String,
    ftype: Type,
    description: String,
    number: Number,
    // enum for python-expression|count|sum|bases|...
    pub value_parser: Option<ValueParser>,

    pub py: Option<CompiledPython<'py>>,
}

impl Column<'_> {
    pub fn new(
        name: String,
        ftype: Type,
        description: String,
        number: Number,
        value_parser: Option<ValueParser>,
    ) -> Self {
        Self {
            name,
            ftype,
            description,
            number,
            value_parser,
            py: None,
        }
    }
}

impl TryFrom<&str> for ValueParser {
    type Error = ColumnError;

    fn try_from(s: &str) -> Result<Self, ColumnError> {
        // if it start with py: use python :lua, use lua, then others are count, sum, bases
        if let Some(rest) = s.strip_prefix("py:") {
            // check if rest is an existing path ane read it into a string
            if let Ok(content) = std::fs::read_to_string(rest) {
                Ok(ValueParser::PythonExpression(content))
            } else {
                Ok(ValueParser::PythonExpression(rest.to_string()))
            }
        } else if let Some(rest) = s.strip_prefix("lua:") {
            Ok(ValueParser::LuaExpression(rest.to_string()))
        } else if s == "count" {
            Ok(ValueParser::Count(false))
        } else if s == "report_count" {
            Ok(ValueParser::Count(true))
        } else if s == "sum" {
            Ok(ValueParser::Sum)
        } else if s == "bases" {
            Ok(ValueParser::Bases)
        } else if s == "chrom_start_end" || s == "cse" {
            Ok(ValueParser::ChromStartEnd)
        } else if s == "original_interval" || s == "oi" {
            Ok(ValueParser::OriginalInterval)
        } else {
            Err(ColumnError::InvalidValueParser(s.to_string()))
        }
    }
}

impl std::fmt::Display for ValueParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueParser::PythonExpression(s) => write!(f, "py:{}", s),
            ValueParser::LuaExpression(s) => write!(f, "lua:{}", s),
            ValueParser::Count(false) => write!(f, "count"),
            ValueParser::Count(true) => write!(f, "report_count"),
            ValueParser::Sum => write!(f, "sum"),
            ValueParser::Bases => write!(f, "bases"),
            ValueParser::ChromStartEnd => write!(f, "chrom_start_end"),
            ValueParser::OriginalInterval => write!(f, "original_interval"),
        }
    }
}

impl std::fmt::Debug for ValueParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl ColumnReporter for Column<'_> {
    fn name(&self) -> &str {
        &self.name
    }

    fn ftype(&self) -> &Type {
        &self.ftype
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn number(&self) -> &Number {
        &self.number
    }

    fn value(
        &self,
        r: &Intersections,
        report_options: Arc<ReportOptions>,
    ) -> Result<Value, ColumnError> {
        // TODO: accept writer and write to it directly.
        match &self.value_parser {
            Some(ValueParser::Count(false)) => {
                // Return the count of overlapping intervals
                Ok(Value::Int(r.overlapping.len() as i32))
            }
            Some(ValueParser::Count(true)) => {
                let report = r.report(&report_options);
                Ok(Value::Int(report.len() as i32))
            }
            Some(ValueParser::Sum) => {
                // Sum the scores of overlapping intervals if they exist
                // This is more complex as we need to extract scores from the intervals
                // For now, return 0.0 as a placeholder
                todo!("sum");
            }
            Some(ValueParser::Bases) => {
                // Calculate total number of bases covered by overlapping intervals
                // Use the report functionality to get the base count

                let report = r.report(&report_options);

                let bases = report.count_bases_by_id();
                let total_bases: i32 = bases.iter().sum::<u64>() as i32;

                Ok(Value::Int(total_bases))
            }
            Some(ValueParser::ChromStartEnd) => {
                let base_interval = r
                    .base_interval
                    .try_lock()
                    .expect("failed to lock base_interval");
                let s = format!(
                    "{}\t{}\t{}",
                    base_interval.chrom(),
                    base_interval.start(),
                    base_interval.stop()
                );
                Ok(Value::String(s))
            }
            Some(ValueParser::OriginalInterval) => {
                // Return the original interval as a string
                let base_interval = r
                    .base_interval
                    .try_lock()
                    .expect("failed to lock base_interval");
                let s = format!(
                    "{}\t{}\t{}",
                    base_interval.chrom(),
                    base_interval.start(),
                    base_interval.stop()
                );
                Ok(Value::String(s))
            }
            Some(ValueParser::PythonExpression(expr)) => {
                // For Python expressions, we should use the compiled Python object
                if let Some(py) = &self.py {
                    // Convert intersection to PyIntersections and evaluate
                    let py_intersection =
                        crate::py::PyIntersections::new(r.clone(), report_options);
                    match py.eval(py_intersection) {
                        Ok(result) => Ok(Value::String(result)),
                        Err(e) => Err(ColumnError::PythonError(format!(
                            "Python error when evaluating expression: \"{}\": {}",
                            expr, e
                        ))),
                    }
                } else {
                    Err(ColumnError::PythonError(
                        "Python interpreter not initialized".to_string(),
                    ))
                }
            }
            Some(ValueParser::LuaExpression(_expr)) => {
                // Similar placeholder for Lua expressions
                // Would need to implement Lua evaluation similar to Python
                Err(ColumnError::InvalidValue(
                    "Lua expressions not yet implemented".to_string(),
                ))
            }
            None => {
                // Default behavior when no parser is specified - return count
                Ok(Value::Int(r.overlapping.len() as i32))
            }
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Integer => write!(f, "Integer"),
            Type::Float => write!(f, "Float"),
            Type::Character => write!(f, "Character"),
            Type::String => write!(f, "String"),
            Type::Flag => write!(f, "Flag"),
        }
    }
}

impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::Not => write!(f, "0"), // flag?
            Number::One => write!(f, "1"),
            Number::R => write!(f, "R"),
            Number::A => write!(f, "A"),
            Number::Dot => write!(f, "."),
        }
    }
}

/// Display like a VCF INFO field
impl std::fmt::Display for Column<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ID={},Number={},Type={},Description=\"{}\"",
            self.name, self.number, self.ftype, self.description
        )
    }
}

impl std::fmt::Debug for Column<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Column {{ name: {}, ftype: {}, description: {}, number: {}, value_parser: {:?} }}",
            self.name, self.ftype, self.description, self.number, self.value_parser
        )
    }
}

/// implement parsing a column from a string like:
/// name:type:description:number
/// e.g.
/// count:Integer:Number of reads supporting the variant:1
/// qual:Float:Phred-scaled quality score:1
/// gt:String:Genotype:1
///
/// and return a Column
impl TryFrom<&str> for Column<'_> {
    type Error = ColumnError;

    fn try_from(s: &str) -> Result<Self, ColumnError> {
        let parts: Vec<&str> = s.splitn(5, ':').collect();
        if parts.len() == 1
            && (parts[0] == "cse"
                || parts[0] == "chrom_start_end"
                || parts[0] == "oi"
                || parts[0] == "original_interval")
        {
            return Ok(match parts[0] {
                "cse" | "chrom_start_end" => Column::new(
                    "chrom\tstart\tend".to_string(),
                    Type::String,
                    "Chromosome:start-end".to_string(),
                    Number::One,
                    Some(ValueParser::ChromStartEnd),
                ),
                "oi" | "original_interval" => Column::new(
                    "original_interval".to_string(),
                    Type::String,
                    "Original interval".to_string(),
                    Number::One,
                    Some(ValueParser::OriginalInterval),
                ),
                _ => unreachable!(),
            });
        }
        if parts.len() == 1 && (parts[0] == "count" || parts[0] == "sum" || parts[0] == "bases") {
            return Ok(match parts[0] {
                "count" => Column::new(
                    "count".to_string(),
                    Type::Integer,
                    "Count".to_string(),
                    Number::One,
                    Some(ValueParser::Count(false)),
                ),
                "sum" => Column::new(
                    "sum".to_string(),
                    Type::Float,
                    "Sum".to_string(),
                    Number::One,
                    Some(ValueParser::Sum),
                ),
                "bases" => Column::new(
                    "bases".to_string(),
                    Type::Integer,
                    "Bases".to_string(),
                    Number::One,
                    Some(ValueParser::Bases),
                ),
                _ => unreachable!(),
            });
        }

        if parts.len() < 2 {
            return Err(ColumnError::InvalidValue(format!(
                "Expected at least two fields (name and type), got: {}",
                s
            )));
        }

        let name = parts[0].to_string();
        let ftype = parts[1].try_into()?;
        let description = if parts.len() > 2 && !parts[2].is_empty() {
            parts[2].to_string()
        } else {
            name.clone()
        };
        let number = if parts.len() > 3 && !parts[3].is_empty() {
            parts[3].try_into()?
        } else {
            // default to "1", which in our conversion becomes Number::One.
            Number::One
        };
        let value_parser = if parts.len() > 4 && !parts[4].is_empty() {
            Some(parts[4].try_into()?)
        } else {
            None
        };

        Ok(Column {
            name,
            ftype,
            description,
            number,
            value_parser,
            py: None,
        })
    }
}

impl TryFrom<&str> for Type {
    type Error = ColumnError;

    fn try_from(s: &str) -> Result<Self, ColumnError> {
        match s.to_lowercase().as_str() {
            "integer" => Ok(Type::Integer),
            "float" => Ok(Type::Float),
            "character" => Ok(Type::Character),
            "string" => Ok(Type::String),
            "flag" => Ok(Type::Flag),
            _ => Err(ColumnError::InvalidType(s.to_string())),
        }
    }
}

impl TryFrom<&str> for Number {
    type Error = ColumnError;

    fn try_from(s: &str) -> Result<Self, ColumnError> {
        match s.to_lowercase().as_str() {
            "not" => Ok(Number::Not),
            "one" | "1" => Ok(Number::One),
            "r" => Ok(Number::R),
            "a" => Ok(Number::A),
            "dot" | "." => Ok(Number::Dot),
            _ => Err(ColumnError::InvalidNumber(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_column() {
        let input = "count:Integer:A count of something";
        let col = Column::try_from(input).unwrap();
        assert_eq!(col.name(), "count");
        assert_eq!(col.ftype(), &Type::Integer);
        assert_eq!(col.description(), "A count of something");
        assert_eq!(col.number(), &Number::One); // Default
        assert!(col.value_parser.is_none());
    }

    #[test]
    fn test_parse_full_column() {
        let input = "total:Float:A total value:R:sum";
        let col = Column::try_from(input).unwrap();
        assert_eq!(col.name(), "total");
        assert_eq!(col.ftype(), &Type::Float);
        assert_eq!(col.description(), "A total value");
        assert_eq!(col.number(), &Number::R);
        assert!(matches!(col.value_parser, Some(ValueParser::Sum)));
    }

    #[test]
    fn test_parse_with_python_expr() {
        let input = "calc:Float:Calculated value:1:py:x + y";
        let col = Column::try_from(input).unwrap();
        assert_eq!(col.name(), "calc");
        assert_eq!(col.ftype(), &Type::Float);
        assert_eq!(col.description(), "Calculated value");
        assert_eq!(col.number(), &Number::One);
        assert!(
            matches!(col.value_parser, Some(ValueParser::PythonExpression(expr)) if expr == "x + y")
        );
    }

    #[test]
    fn test_invalid_type() {
        let input = "count:Invalid:A description";
        assert!(matches!(
            Column::try_from(input),
            Err(ColumnError::InvalidType(_))
        ));
    }

    #[test]
    fn test_invalid_number() {
        let input = "count:Integer:A description:Invalid";
        let c = Column::try_from(input);
        assert!(
            matches!(c, Err(ColumnError::InvalidNumber(_))),
            "c: {:?}",
            c
        );
    }

    #[test]
    fn test_invalid_value_parser() {
        let input = "count:Integer:A description:1:invalid";
        assert!(matches!(
            Column::try_from(input),
            Err(ColumnError::InvalidValueParser(_))
        ));
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{:.4}", fl),
            Value::String(s) => write!(f, "{}", s),
            Value::Flag(b) => write!(f, "{}", b),
            Value::VecInt(v) => write!(
                f,
                "{}",
                v.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Value::VecFloat(v) => write!(
                f,
                "{}",
                v.iter()
                    .map(|f| format!("{:.4}", f))
                    .map(|s| {
                        if s.contains('.') {
                            s.trim_end_matches('0').trim_end_matches('.').to_string()
                        } else {
                            s
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Value::VecString(v) => write!(
                f,
                "{}",
                v.iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}
