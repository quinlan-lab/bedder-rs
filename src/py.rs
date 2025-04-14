use crate::bedder_bed::BedRecord;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyFunction, PyString};
use std::ffi::CString;

use crate::position::Position;
use crate::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};

// Wrapper for simplebed::BedRecord
/// A Python wrapper for a BED record.
///
/// Attributes:
///     chrom (str): The chromosome name
///     start (int): The start position (0-based)
///     stop (int): The end position (exclusive)
///     name (str, optional): The name field if present
///     score (float, optional): The score field if present
#[pyclass]
#[derive(Clone, Debug)] // Added Debug for easier inspection
pub struct PyBedRecord {
    inner: Arc<BedRecord>,
}

#[pymethods]
impl PyBedRecord {
    #[getter]
    /// Get the chromosome name
    fn chrom(&self) -> PyResult<String> {
        Ok(self.inner.0.chrom().to_string())
    }

    #[getter]
    /// Get the start position (0-based)
    fn start(&self) -> PyResult<u64> {
        Ok(self.inner.0.start())
    }

    #[getter]
    /// Get the end position (exclusive)
    fn stop(&self) -> PyResult<u64> {
        Ok(self.inner.0.end())
    }

    #[getter]
    /// Get the name field if present
    fn name(&self) -> PyResult<Option<String>> {
        Ok(self.inner.0.name().map(|s| s.to_string()))
    }

    #[getter]
    /// Get the score field if present
    fn score(&self) -> PyResult<Option<f64>> {
        Ok(self.inner.0.score())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    /// Get any additional fields beyond the standard BED fields
    fn other_fields(&self) -> PyResult<Vec<String>> {
        Ok(self
            .inner
            .0
            .other_fields()
            .iter()
            .map(|f| f.to_string())
            .collect())
    }
}

impl From<Arc<BedRecord>> for PyBedRecord {
    fn from(inner: Arc<BedRecord>) -> Self {
        PyBedRecord { inner }
    }
}

// Wrapper for bedder::report::ReportFragment
/// A fragment of a report containing intersection results.
///
/// Attributes:
///     a (Position, optional): The query interval
///     b (list[Position]): List of intervals that intersect with the query
///     id (int): Unique identifier for this fragment
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyReportFragment {
    inner: crate::report::ReportFragment,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyReportOptions {
    inner: Arc<ReportOptions>,
}

#[pymethods] // TODO: start here and implement
impl PyReportOptions {
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

impl PyReportOptions {
    pub(crate) fn new(report_options: Arc<ReportOptions>) -> Self {
        PyReportOptions {
            inner: report_options,
        }
    }
}

// No changes needed to ReportFragment relative to previous good answer
#[pymethods]
impl PyReportFragment {
    #[getter]
    /// Get the query interval if present
    fn a(&self) -> PyResult<Option<PyPosition>> {
        match &self.inner.a {
            Some(pos) => Ok(Some(PyPosition { inner: pos.clone() })),
            None => Ok(None),
        }
    }

    #[getter]
    /// Get the list of intersecting intervals
    fn b(&self) -> PyResult<Vec<PyPosition>> {
        Ok(self
            .inner
            .b
            .iter()
            .map(|pos| PyPosition { inner: pos.clone() })
            .collect())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    #[getter]
    /// Get the unique identifier for this fragment
    fn id(&self) -> PyResult<usize> {
        Ok(self.inner.id)
    }
}

impl From<crate::report::ReportFragment> for PyReportFragment {
    fn from(inner: crate::report::ReportFragment) -> Self {
        PyReportFragment { inner }
    }
}

// Wrapper for bedder::report::Report
/// A collection of intersection results.
///
/// Methods:
///     add_fragment(fragment): Add a report fragment to the collection
///     count_overlaps_by_id(): Get count of overlaps for each query ID
///     count_bases_by_id(): Get count of overlapping bases for each query ID
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyReport {
    inner: crate::report::Report,
}

#[pymethods]
impl PyReport {
    #[new]
    /// Create a new empty Report
    fn new() -> Self {
        PyReport {
            inner: crate::report::Report::new(Vec::new()),
        }
    }

    /// Add a report fragment to the collection
    fn add_fragment(&mut self, frag: PyReportFragment) -> PyResult<()> {
        let inner_frags = vec![frag.inner];
        self.inner = crate::report::Report::new(inner_frags);
        Ok(())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<PyReportIter>> {
        // TODO: fix: don't clone
        let inner = slf.inner.into_iter().cloned().collect::<Vec<_>>();
        let iter = PyReportIter { inner, index: 0 };
        Py::new(slf.py(), iter)
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self.inner.len())
    }

    fn __getitem__(&self, index: usize) -> PyResult<PyReportFragment> {
        Ok(PyReportFragment::from(self.inner[index].clone()))
    }

    /// Get count of overlaps for each query ID
    fn count_overlaps_by_id(&self) -> PyResult<Vec<u64>> {
        Ok(self.inner.count_overlaps_by_id())
    }

    /// Get count of overlapping bases for each query ID
    fn count_bases_by_id(&self) -> PyResult<Vec<u64>> {
        Ok(self.inner.count_bases_by_id())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

#[pyclass]
#[derive(Clone)]
struct PyReportIter {
    inner: Vec<crate::report::ReportFragment>,
    index: usize,
}

#[pymethods]
impl PyReportIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyReportFragment> {
        if slf.index >= slf.inner.len() {
            return None;
        }
        let fragment = slf.inner[slf.index].clone();
        slf.index += 1;
        Some(fragment.into())
    }
}

use std::sync::Arc;
// Wrapper for Position
/// A genomic interval that can represent BED or other formats.
///
/// Attributes:
///     chrom (str): The chromosome name
///     start (int): The start position (0-based)
///     stop (int): The end position (exclusive)
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyPosition {
    inner: Arc<Position>, // Use the trait object
}

#[pymethods]
impl PyPosition {
    /// Get the BED record if this position represents a BED interval
    fn bed(&self) -> PyResult<Option<PyBedRecord>> {
        if let Position::Bed(b) = &self.inner.as_ref() {
            // get an Arc to the underlying BedRecord
            let bed_record = Arc::new(b.clone());
            Ok(Some(PyBedRecord::from(bed_record)))
        } else {
            Ok(None)
        }
    }

    #[getter]
    /// Get the chromosome name
    fn chrom(&self) -> PyResult<String> {
        Ok(self.inner.chrom().to_string())
    }

    #[getter]
    /// Get the start position (0-based)
    fn start(&self) -> PyResult<u64> {
        Ok(self.inner.start())
    }

    #[getter]
    /// Get the end position (exclusive)
    fn stop(&self) -> PyResult<u64> {
        Ok(self.inner.stop())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Python wrapper for IntersectionMode
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyIntersectionMode {
    inner: IntersectionMode,
}

#[pymethods]
impl PyIntersectionMode {
    #[new]
    fn new(mode_str: &str) -> Self {
        PyIntersectionMode {
            inner: IntersectionMode::from(mode_str),
        }
    }

    #[staticmethod]
    fn default() -> Self {
        PyIntersectionMode {
            inner: IntersectionMode::default(),
        }
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Python wrapper for IntersectionPart
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyIntersectionPart {
    inner: IntersectionPart,
}

#[pymethods]
impl PyIntersectionPart {
    #[staticmethod]
    fn none() -> Self {
        PyIntersectionPart {
            inner: IntersectionPart::None,
        }
    }

    #[staticmethod]
    fn part() -> Self {
        PyIntersectionPart {
            inner: IntersectionPart::Part,
        }
    }

    #[staticmethod]
    fn whole() -> Self {
        PyIntersectionPart {
            inner: IntersectionPart::Whole,
        }
    }

    #[staticmethod]
    fn inverse() -> Self {
        PyIntersectionPart {
            inner: IntersectionPart::Inverse,
        }
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Python wrapper for OverlapAmount
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyOverlapAmount {
    inner: OverlapAmount,
}

#[pymethods]
impl PyOverlapAmount {
    #[new]
    fn new(amount: &str) -> Self {
        PyOverlapAmount {
            inner: OverlapAmount::from(amount),
        }
    }

    #[staticmethod]
    fn bases(bases: u64) -> Self {
        PyOverlapAmount {
            inner: OverlapAmount::Bases(bases),
        }
    }

    #[staticmethod]
    fn fraction(fraction: f32) -> Self {
        PyOverlapAmount {
            inner: OverlapAmount::Fraction(fraction),
        }
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// A Python wrapper for Intersections
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyIntersections {
    inner: crate::intersection::Intersections,
    report_options: Arc<ReportOptions>,
}

impl PyIntersections {
    pub fn new(
        inner: crate::intersection::Intersections,
        report_options: Arc<ReportOptions>,
    ) -> Self {
        PyIntersections {
            inner,
            report_options,
        }
    }
}
#[pymethods]
impl PyIntersections {
    #[getter]
    /// Get the base interval
    fn base_interval(&self) -> PyResult<PyPosition> {
        Ok(PyPosition {
            inner: self.inner.base_interval.clone(),
        })
    }
    /// Get the base interval
    fn a(&self) -> PyResult<PyPosition> {
        Ok(PyPosition {
            inner: self.inner.base_interval.clone(),
        })
    }

    #[getter]
    /// Get the list of overlapping intervals
    fn overlapping(&self) -> PyResult<Vec<PyPosition>> {
        Ok(self
            .inner
            .overlapping
            .iter()
            .map(|i| PyPosition {
                inner: i.interval.clone(),
            })
            .collect())
    }

    /// Report intersections based on specified modes and requirements
    fn report(&self) -> PyResult<PyReport> {
        Ok(PyReport {
            inner: self.inner.report(self.report_options.clone()),
        })
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// A compiled Python f-string that can be reused for better performance
pub struct CompiledPython<'py> {
    _code: String,
    _module: Bound<'py, PyModule>,
    f: Bound<'py, PyFunction>,
}

use pyo3_ffi::c_str;

const FN_NAME: &str = "bedder";

fn wrap_python_code(code: &str) -> String {
    let mut indented_code_lines: Vec<String> = code
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| format!("    {}", line)) // Add indentation to each line
        .collect::<Vec<String>>();

    // Try to add a return statement to the last line if it's not already there
    if !indented_code_lines.is_empty()
        && !indented_code_lines[indented_code_lines.len() - 1]
            .trim()
            .starts_with("return")
    {
        let last_line = indented_code_lines.pop().unwrap();
        indented_code_lines.push(format!(
            "    return {}",
            last_line.strip_prefix("    ").unwrap()
        ));
    }

    let indented_code = indented_code_lines.join("\n");

    format!("def {FN_NAME}(intersection):\n{}", indented_code)
}

// Add this function to initialize the Python environment
pub fn initialize_python(py: Python<'_>) -> PyResult<()> {
    // Register the bedder_py module in sys.modules
    let bedder_module = PyModule::new(py, "bedder_py")?;
    bedder_py(&bedder_module)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("bedder_py", &bedder_module)?;

    // Import bedder classes into the __builtins__ module so they're globally available
    let builtins = py.import("builtins")?;
    let bedder_classes = [
        "PyBedRecord",
        "PyPosition",
        "PyReport",
        "PyReportFragment",
        "PyIntersections",
        "PyIntersectionMode",
        "PyIntersectionPart",
        "PyOverlapAmount",
    ];

    for class_name in bedder_classes {
        let class = &bedder_module.getattr(class_name)?;
        builtins.setattr(class_name, class)?;
    }

    Ok(())
}

impl<'py> CompiledPython<'py> {
    /// Create a new compiled Python function
    ///
    /// If snippet is true, the code will be wrapped in a function called `bedder`
    /// and the function will be returned. Otherwise, the code will be executed directly.
    /// It must then be a function.
    pub fn new(py: Python<'py>, f_string_code: &str, snippet: bool) -> PyResult<Self> {
        let module = if snippet {
            let wrapped_code = wrap_python_code(f_string_code);
            log::info!("wrapped_code: {}", wrapped_code);
            let code = CString::new(wrapped_code)?;
            PyModule::from_code(py, &code, c_str!("user_code"), c_str!("user_code"))?
        } else {
            let code = CString::new(f_string_code)?;
            PyModule::from_code(py, &code, c_str!("user_code"), c_str!("user_code"))?
        };

        let f = module.getattr(FN_NAME)?.extract()?;

        Ok(CompiledPython {
            _code: f_string_code.to_string(),
            _module: module,
            f,
        })
    }

    pub fn eval(&self, intersections: PyIntersections) -> PyResult<String> {
        let result = self.f.call1((intersections,))?;
        if let Ok(result) = result.downcast_exact::<PyString>() {
            Ok(result.to_string())
        } else if let Ok(s) = result
            .str()
            .and_then(|py_str| py_str.to_str().map(|s| s.to_owned()))
        {
            Ok(s)
        } else {
            Err(PyTypeError::new_err("Result is not a string"))
        }
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn bedder_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBedRecord>()?;
    m.add_class::<PyReportFragment>()?;
    m.add_class::<PyReport>()?;
    m.add_class::<PyPosition>()?;
    m.add_class::<PyIntersections>()?;
    m.add_class::<PyIntersectionMode>()?;
    m.add_class::<PyIntersectionPart>()?;
    m.add_class::<PyOverlapAmount>()?;

    Ok(())
}
