use crate::report::ReportFragment;

pub enum Value {
    Int(i32),
    Float(f32),
    String(String),
    Flag(bool),
    VecInt(Vec<i32>),
    VecFloat(Vec<f32>),
    VecString(Vec<String>),
}

pub enum Type {
    Integer,
    Float,
    Character,
    String,
    Flag,
}

/// The number of Values to expect (similar to Number attribute in VCF INFO/FMT fields)
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
}

/// A ColumnReporter tells bedder how to report a column in the output.
pub trait ColumnReporter {
    /// report the name, e.g. `count` for the INFO field of the VCF
    fn name(&self) -> &str;
    /// report the type, for the INFO field of the VCF
    fn ftype(&self) -> &Type; // Type is some enum from noodles or here that limits to relevant types
    fn description(&self) -> &str;
    fn number(&self) -> &Number;

    fn value(&self, r: &ReportFragment) -> Result<Value, ColumnError>; // Value probably something from noodles that encapsulates Float/Int/Vec<Float>/String/...
}

pub struct Column {
    name: String,
    ftype: Type,
    description: String,
    number: Number,
}

impl ColumnReporter for Column {
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

    fn value(&self, _r: &ReportFragment) -> Result<Value, ColumnError> {
        todo!()
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
impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ID={},Number={},Type={},Description=\"{}\"",
            self.name, self.number, self.ftype, self.description
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
impl TryFrom<&str> for Column {
    type Error = ColumnError;

    fn try_from(s: &str) -> Result<Self, ColumnError> {
        let parts = s.split(':').collect::<Vec<&str>>();
        Ok(Column {
            name: parts[0].to_string(),
            ftype: parts[1].try_into()?,
            description: parts[2].to_string(),
            number: parts[3].try_into()?,
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
