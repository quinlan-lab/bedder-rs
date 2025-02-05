use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;
use simplebed::BedRecord as SimpleBedRecord; // Import directly
use std::ffi::CString;

use crate::position::Position; // Import Position

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
    inner: SimpleBedRecord,
}

#[pymethods]
impl PyBedRecord {
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
        Ok(self.inner.end())
    }

    #[getter]
    /// Get the name field if present
    fn name(&self) -> PyResult<Option<String>> {
        Ok(self.inner.name().map(|s| s.to_string()))
    }

    #[getter]
    /// Get the score field if present
    fn score(&self) -> PyResult<Option<f64>> {
        Ok(self.inner.score())
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
            .other_fields()
            .iter()
            .map(|f| f.to_string())
            .collect())
    }
}

impl From<SimpleBedRecord> for PyBedRecord {
    fn from(inner: SimpleBedRecord) -> Self {
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
    inner: Position, // Use the trait object
}

#[pymethods]
impl PyPosition {
    /// Get the BED record if this position represents a BED interval
    fn bed(&self) -> PyResult<Option<PyBedRecord>> {
        if let Position::Bed(b) = &self.inner {
            Ok(Some(PyBedRecord::from(b.0.clone())))
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

/// Format a report fragment using an f-string.
///
/// Args:
///     report (ReportFragment): The report fragment to format
///     f_string_code (str): The f-string code to use for formatting
///
/// Returns:
///     str: The formatted string
#[pyfunction]
fn py_run_f_string(
    py: Python<'_>,
    report: PyReportFragment,
    f_string_code: &str,
) -> PyResult<String> {
    let globals = PyDict::new(py);
    globals.set_item("report", report)?;

    let code = format!("f'{}'", f_string_code.replace("'", "\\'"));
    let code = CString::new(code).unwrap();
    let result = py.eval(code.as_ref(), Some(&globals), None)?;

    result.extract()
}

/// A Python module implemented in Rust.
#[pymodule]
fn bedder_py(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBedRecord>()?;
    m.add_class::<PyReportFragment>()?;
    m.add_class::<PyReport>()?;
    m.add_class::<PyPosition>()?;
    m.add_function(wrap_pyfunction!(py_run_f_string, py)?)?;

    Ok(())
}
