use clap::ValueEnum;
use std::{num::ParseFloatError, str::FromStr};

/// IntersectionMode indicates requirements for the intersection.
/// And extra fields that might be reported.
#[derive(Eq, PartialEq, Debug, Clone, ValueEnum)]
pub enum IntersectionMode {
    // https://bedtools.readthedocs.io/en/latest/content/tools/intersect.html#usage-and-option-summary
    /// Default without extra requirements.
    #[value(name = "default")]
    Default,

    /// Return A(B) if it does *not* overlap B(A). Bedtools -v
    #[value(name = "not")]
    Not,

    /// Constraints are per piece of interval (not the sum of overlapping intervals)
    #[value(name = "piece")]
    PerPiece,
}

impl From<&str> for IntersectionMode {
    fn from(s: &str) -> Self {
        // Use clap's value enum parsing which handles case-insensitivity and potential future aliases
        <Self as ValueEnum>::from_str(s, true).unwrap_or(Self::Default)
    }
}

impl FromStr for IntersectionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Leverage clap's parsing logic
        <Self as ValueEnum>::from_str(s, true)
    }
}

/// IntersectionPart indicates what to report for the intersection.
#[derive(Eq, PartialEq, Debug, Clone, ValueEnum)]
pub enum IntersectionPart {
    /// Don't report the intersection.
    /// This is commonly used for -b to not report b intervals.
    None,
    /// Report each portion of A that overlaps B
    Piece,
    /// Report the whole interval of A that overlaps B
    Whole,
    /// Report each portion of A that does *NOT* overlap B
    Inverse,
}

impl std::fmt::Display for IntersectionPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntersectionPart::None => write!(f, "none"),
            IntersectionPart::Piece => write!(f, "piece"),
            IntersectionPart::Whole => write!(f, "whole"),
            IntersectionPart::Inverse => write!(f, "inverse"),
        }
    }
}

impl From<&str> for IntersectionPart {
    fn from(s: &str) -> Self {
        <Self as ValueEnum>::from_str(s, true).unwrap_or(Self::Whole)
    }
}

impl FromStr for IntersectionPart {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "piece" => Ok(Self::Piece),
            "whole" => Ok(Self::Whole),
            "inverse" => Ok(Self::Inverse),
            _ => Err(format!("unknown intersection part {}", s)),
        }
    }
}

impl Default for IntersectionPart {
    fn default() -> Self {
        Self::Whole
    }
}

impl Default for &IntersectionPart {
    fn default() -> Self {
        &IntersectionPart::Whole
    }
}

impl Default for IntersectionMode {
    fn default() -> Self {
        Self::Default
    }
}

impl Default for &IntersectionMode {
    fn default() -> Self {
        &IntersectionMode::Default
    }
}

/// OverlapAmount indicates the amount of overlap required.
/// Either as bases or as a fraction of the total length.
#[derive(PartialEq, Debug, Clone)]
pub enum OverlapAmount {
    /// Number of bases that must overlap
    Bases(u64),
    /// Fraction of the interval that must overlap (0.0 to 1.0)
    Fraction(f32),
}

impl Eq for OverlapAmount {}

impl FromStr for OverlapAmount {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(f) = s.strip_suffix('%') {
            Ok(Self::Fraction(f.parse::<f32>()? / 100.0))
        } else if s.contains('.') {
            Ok(Self::Fraction(s.parse::<f32>()?))
        } else {
            Ok(Self::Bases(s.parse::<f32>()? as u64))
        }
    }
}

impl From<&str> for OverlapAmount {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap()
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

impl Default for &OverlapAmount {
    fn default() -> Self {
        &OverlapAmount::Bases(1)
    }
}

/// Options for configuring how intersections are reported.
///
/// # Examples
///
/// ```
/// use bedder::report_options::{ReportOptions, IntersectionMode, IntersectionPart, OverlapAmount};
///
/// let options = ReportOptions::builder()
///     .a_mode(IntersectionMode::Not)
///     .b_mode(IntersectionMode::Default)
///     .a_piece(IntersectionPart::Whole)
///     .b_piece(IntersectionPart::Piece)
///     .a_requirements(OverlapAmount::Bases(5))
///     .b_requirements(OverlapAmount::Fraction(0.5))
///     .build();
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportOptions {
    pub a_mode: IntersectionMode,
    pub b_mode: IntersectionMode,
    pub a_piece: IntersectionPart,
    pub b_piece: IntersectionPart,
    pub a_requirements: OverlapAmount,
    pub b_requirements: OverlapAmount,
}

impl ReportOptions {
    /// Create a new builder for ReportOptions
    pub fn builder() -> ReportOptionsBuilder {
        ReportOptionsBuilder::new()
    }
}

/// Builder for ReportOptions that allows for fluent construction of options.
///
/// # Examples
///
/// ```
/// use bedder::report_options::{ReportOptionsBuilder, IntersectionMode, IntersectionPart, OverlapAmount};
///
/// let builder = ReportOptionsBuilder::new()
///     .a_mode(IntersectionMode::Not)
///     .b_mode(IntersectionMode::Default);
/// ```
pub struct ReportOptionsBuilder {
    a_mode: IntersectionMode,
    b_mode: IntersectionMode,
    a_piece: IntersectionPart,
    b_piece: IntersectionPart,
    a_requirements: OverlapAmount,
    b_requirements: OverlapAmount,
}

impl Default for ReportOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportOptionsBuilder {
    /// Create a new ReportOptionsBuilder with default values
    pub fn new() -> Self {
        Self {
            a_mode: IntersectionMode::Default,
            b_mode: IntersectionMode::Default,
            a_piece: IntersectionPart::Whole,
            b_piece: IntersectionPart::Whole,
            a_requirements: OverlapAmount::Bases(1),
            b_requirements: OverlapAmount::Bases(1),
        }
    }

    /// Set the A mode
    pub fn a_mode(mut self, mode: IntersectionMode) -> Self {
        self.a_mode = mode;
        self
    }

    /// Set the B mode
    pub fn b_mode(mut self, mode: IntersectionMode) -> Self {
        self.b_mode = mode;
        self
    }

    /// Set the A part
    pub fn a_piece(mut self, part: IntersectionPart) -> Self {
        self.a_piece = part;
        self
    }

    /// Set the B part
    pub fn b_piece(mut self, part: IntersectionPart) -> Self {
        self.b_piece = part;
        self
    }

    /// Set the A requirements
    pub fn a_requirements(mut self, requirements: OverlapAmount) -> Self {
        self.a_requirements = requirements;
        self
    }

    /// Set the B requirements
    pub fn b_requirements(mut self, requirements: OverlapAmount) -> Self {
        self.b_requirements = requirements;
        self
    }

    /// Build the ReportOptions
    pub fn build(self) -> ReportOptions {
        ReportOptions {
            a_mode: self.a_mode,
            b_mode: self.b_mode,
            a_piece: self.a_piece,
            b_piece: self.b_piece,
            a_requirements: self.a_requirements,
            b_requirements: self.b_requirements,
        }
    }
}
