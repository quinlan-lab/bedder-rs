use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{self, PyFunction};
use std::collections::HashMap;

use crate::column::{Number, Type, Value};
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
    inner: Arc<Mutex<Position>>,
}

#[pymethods]
impl PyBedRecord {
    #[getter]
    /// Get the chromosome name
    fn chrom(&self) -> PyResult<String> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .chrom()
            .to_string())
    }

    #[getter]
    /// Get the start position (0-based)
    fn start(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .start())
    }

    #[getter]
    /// Get the end position (exclusive)
    fn stop(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .stop())
    }

    #[getter]
    /// Get the name field if present
    fn name(&self) -> PyResult<Option<String>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.name().map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    #[setter]
    /// Set the name field
    fn set_name(&mut self, name: &str) -> PyResult<()> {
        if let Position::Bed(b) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            b.0.set_name(name.to_string());
        }
        Ok(())
    }

    #[getter]
    /// Get the score field if present
    fn score(&self) -> PyResult<Option<f64>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.score())
        } else {
            Ok(None)
        }
    }

    #[setter]
    /// Set the score field
    fn set_score(&mut self, score: f64) -> PyResult<()> {
        if let Position::Bed(b) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            b.0.set_score(score);
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    /// Get any additional fields beyond the standard BED fields
    fn other_fields(&self) -> PyResult<Vec<String>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.other_fields().iter().map(|f| f.to_string()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// index into the other_fields
    fn __getitem__(&self, index: usize) -> PyResult<String> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.other_fields()[index].to_string())
        } else {
            Err(PyIndexError::new_err("Index out of bounds"))
        }
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
struct PyReportFragmentIter {
    report_fragment: crate::report::ReportFragment,
    index: usize,
}

#[pymethods]
impl PyReportFragmentIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyPosition> {
        if slf.index >= slf.report_fragment.b.len() {
            return None;
        }
        let position = PyPosition {
            inner: slf.report_fragment.b[slf.index].clone(),
        };
        slf.index += 1;
        Some(position)
    }
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyReportOptions {
    inner: Arc<ReportOptions>,
}

#[pymethods]
impl PyReportOptions {
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

impl PyReportOptions {
    #[allow(dead_code)]
    pub(crate) fn new(report_options: Arc<ReportOptions>) -> Self {
        PyReportOptions {
            inner: report_options,
        }
    }
}

impl PyReportFragment {
    pub(crate) fn new(report_fragment: crate::report::ReportFragment) -> Self {
        PyReportFragment {
            inner: report_fragment,
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

    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<PyReportFragmentIter>> {
        let report_fragment = slf.inner.clone();
        let iter = PyReportFragmentIter {
            report_fragment,
            index: 0,
        };
        Py::new(slf.py(), iter)
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
    inner: Arc<crate::report::Report>,
}

#[pymethods]
impl PyReport {
    #[new]
    /// Create a new empty Report
    fn new() -> Self {
        PyReport {
            inner: Arc::new(crate::report::Report::new(Vec::new())),
        }
    }

    /// Add a report fragment to the collection
    fn add_fragment(&mut self, frag: PyReportFragment) -> PyResult<()> {
        let inner_frags = vec![frag.inner];
        self.inner = Arc::new(crate::report::Report::new(inner_frags));
        Ok(())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<PyReportIter>> {
        let inner = slf.inner.clone();
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
    inner: Arc<crate::report::Report>,
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

use parking_lot::Mutex;
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
    inner: Arc<Mutex<Position>>, // Use the trait object
}

#[pymethods]
impl PyPosition {
    /// Get the BED record if this position represents a BED interval
    fn bed(&self) -> PyResult<Option<PyBedRecord>> {
        let is_bed =
            if let Position::Bed(_) = *self.inner.try_lock().expect("failed to lock interval") {
                true
            } else {
                false
            };
        if is_bed {
            Ok(Some(PyBedRecord {
                inner: self.inner.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    #[getter]
    /// Get the chromosome name
    fn chrom(&self) -> PyResult<String> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .chrom()
            .to_string())
    }

    #[getter]
    /// Get the start position (0-based)
    fn start(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .start())
    }

    #[getter]
    /// Get the end position (exclusive)
    fn stop(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .stop())
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
    fn report(&mut self) -> PyResult<PyReport> {
        Ok(PyReport {
            inner: self.inner.report(&self.report_options),
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
    function_name: String,
    f: Bound<'py, PyFunction>,
    ftype: Type,
    number: Number,
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

#[derive(Debug)]
pub struct PythonFunction<'py> {
    name: String,
    return_type: String,
    pyfn: pyo3::Bound<'py, pyo3::types::PyFunction>,
    // description is from the docstring of the function
    description: String,
}

const BEDDER_PREFIX: &str = "bedder_";

/// Introspects the Python environment to find functions and their return type annotations.
pub fn introspect_python_functions<'py>(
    _py: Python<'py>,
    globals: pyo3::Bound<'py, pyo3::types::PyDict>,
) -> PyResult<HashMap<String, PythonFunction<'py>>> {
    let mut functions_map = HashMap::new();

    for (name, obj) in globals.iter() {
        let name_str = name.to_string();
        if !name_str.starts_with(BEDDER_PREFIX) {
            continue;
        }
        // Check if the object is a Python function
        if obj.is_instance_of::<pyo3::types::PyFunction>() {
            let pyfn = obj.downcast::<pyo3::types::PyFunction>()?;
            let mut return_type_str = "No return annotation".to_string();
            let mut description_str = "".to_string();
            if let Ok(annotations) = obj.getattr("__annotations__") {
                if let Ok(dict) = annotations.downcast::<pyo3::types::PyDict>() {
                    if let Some(return_type) = dict.get_item("return")? {
                        if let Ok(type_name) = return_type.getattr("__name__") {
                            return_type_str = format!("{}", type_name.to_string());
                        } else {
                            // Fallback to repr if __name__ is not available
                            return_type_str = format!("{}", return_type.repr()?);
                        }
                    }
                    if let Ok(description) = obj.getattr("__doc__") {
                        description_str = description.to_string();
                        // get first non-empty line of docstring
                        description_str = description_str
                            .split('\n')
                            .filter(|line| !line.is_empty())
                            .next()
                            .unwrap_or("")
                            .to_string();
                    }
                }
            }
            if !["str", "int", "float", "bool"].contains(&return_type_str.as_str()) {
                return Err(PyValueError::new_err(format!(
                    "Invalid return type '{}'. Expected str, int, float, or bool. Make sure the function has a return annotation.",
                    return_type_str
                )));
            }
            let name_str = name_str[BEDDER_PREFIX.len()..].to_string();
            functions_map.insert(
                name_str.clone(),
                PythonFunction {
                    name: name_str,
                    return_type: return_type_str,
                    pyfn: pyfn.clone(),
                    description: description_str,
                },
            );
        }
    }

    Ok(functions_map)
}

impl<'py> CompiledPython<'py> {
    /// Create a new compiled Python function
    pub fn new(
        _py: Python<'py>,
        fname: &str,
        functions: &HashMap<String, PythonFunction<'py>>,
    ) -> PyResult<Self> {
        let f = functions.get(fname).ok_or(PyValueError::new_err(format!(
            "Function '{}' not found in the provided functions map.",
            fname
        )))?;

        Ok(CompiledPython {
            function_name: fname.to_string(),
            f: f.pyfn.clone(),
            ftype: Type::try_from(f.return_type.as_str())
                .map_err(|e| PyValueError::new_err(format!("Invalid type: {}", e)))?,
            number: Number::One,
        })
    }

    #[inline]
    pub fn eval(&self, fragment: PyReportFragment) -> PyResult<Value> {
        let result = self.f.call1((fragment,))?;
        if self.number != Number::One && self.number != Number::Dot {
            unimplemented!("Multiple values not supported yet in python eval function");
        } else {
            match self.ftype {
                Type::Integer => result
                    .downcast_exact::<types::PyInt>()
                    .map(|py_int| Value::Int(py_int.extract::<i32>().unwrap()))
                    .map_err(|_| PyTypeError::new_err("Result is not an integer")),
                Type::Float => result
                    .downcast_exact::<types::PyFloat>()
                    .map(|py_float| Value::Float(py_float.extract::<f32>().unwrap()))
                    .map_err(|_| PyTypeError::new_err("Result is not a float")),
                Type::Character => result
                    .downcast_exact::<types::PyString>()
                    .map(|py_str| Value::String(py_str.to_str().unwrap().to_string()))
                    .map_err(|_| PyTypeError::new_err("Result is not a string")),
                Type::String => result
                    .downcast_exact::<types::PyString>()
                    .map(|py_str| Value::String(py_str.to_str().unwrap().to_string()))
                    .map_err(|_| PyTypeError::new_err("Result is not a string")),
                Type::Flag => result
                    .downcast_exact::<types::PyBool>()
                    .map(|py_bool| Value::Flag(py_bool.extract::<bool>().unwrap()))
                    .map_err(|_| PyTypeError::new_err("Result is not a boolean")),
            }
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
    m.add_class::<PyReportFragmentIter>()?;
    m.add_class::<PyReportIter>()?;

    Ok(())
}
