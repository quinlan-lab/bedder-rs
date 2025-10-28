use pyo3::exceptions::{PyIndexError, PyKeyError, PyTypeError, PyValueError};
use pyo3::types::{self, PyFunction};
use pyo3::IntoPyObject;
use pyo3::{prelude::*, IntoPyObjectExt};

use std::collections::HashMap;
use std::fmt;

use crate::column::{Number, Type, Value};
use crate::position::Position;
use crate::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};
use rust_htslib as htslib;

// Wrapper for simplebed::BedRecord
/// A Python wrapper for a BED record.
///
/// Attributes:
///     chrom (str): The chromosome name
///     start (int): The start position (0-based)
///     stop (int): The end position (exclusive)
///     name (str, optional): The name field if present
///     score (float, optional): The score field if present
///
/// # Example
/// ```python
/// bed_record = position.bed()
/// if bed_record is not None:
///     print(bed_record.chrom, bed_record.start, bed_record.stop)
///     bed_record.set_name("example")
/// ```
#[pyclass]
#[derive(Clone, Debug)] // Added Debug for easier inspection
pub struct PyBedRecord {
    inner: Arc<Mutex<Position>>,
}

#[pymethods]
impl PyBedRecord {
    #[getter]
    /// Get the chromosome name.
    ///
    /// # Example
    /// ```python
    /// chrom = bed_record.chrom
    /// ```
    fn chrom(&self) -> PyResult<String> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .chrom()
            .to_string())
    }

    #[getter]
    /// Get the start position (0-based).
    ///
    /// # Example
    /// ```python
    /// start = bed_record.start
    /// ```
    fn start(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .start())
    }

    #[getter]
    /// Get the end position (exclusive).
    ///
    /// # Example
    /// ```python
    /// stop = bed_record.stop
    /// ```
    fn stop(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .stop())
    }

    #[getter]
    /// Get the name field if present.
    ///
    /// # Example
    /// ```python
    /// label = bed_record.name
    /// ```
    fn name(&self) -> PyResult<Option<String>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.name().map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    #[setter]
    /// Set the name field.
    ///
    /// # Example
    /// ```python
    /// bed_record.name = "example"
    /// ```
    fn set_name(&mut self, name: &str) -> PyResult<()> {
        if let Position::Bed(b) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            b.0.set_name(name.to_string());
        }
        Ok(())
    }

    #[getter]
    /// Get the score field if present.
    ///
    /// # Example
    /// ```python
    /// score = bed_record.score
    /// ```
    fn score(&self) -> PyResult<Option<f64>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.score())
        } else {
            Ok(None)
        }
    }

    #[setter]
    /// Set the score field.
    ///
    /// # Example
    /// ```python
    /// bed_record.score = 42.0
    /// ```
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

    /// Get any additional fields beyond the standard BED fields.
    ///
    /// # Example
    /// ```python
    /// extra = bed_record.other_fields()
    /// for value in extra:
    ///     print(value)
    /// ```
    fn other_fields(&self) -> PyResult<Vec<String>> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.other_fields().iter().map(|f| f.to_string()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Index into the `other_fields` list.
    ///
    /// # Example
    /// ```python
    /// value = bed_record[0]
    /// ```
    fn __getitem__(&self, index: usize) -> PyResult<String> {
        if let Position::Bed(b) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(b.0.other_fields()[index].to_string())
        } else {
            Err(PyIndexError::new_err("Index out of bounds"))
        }
    }
}

/// A Python wrapper for a VCF record.
///
/// Attributes:
///     chrom (str): The chromosome name
///     pos (int): The position (0-based)
///
/// # Example
/// ```python
/// vcf_record = position.vcf()
/// if vcf_record is not None:
///     print(vcf_record.chrom, vcf_record.pos)
///     dp = vcf_record.info("DP")
///     if dp is not None:
///         print("depth:", dp)
/// ```
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyVcfRecord {
    inner: Arc<Mutex<Position>>,
}

impl PyVcfRecord {
    pub fn new(inner: Arc<Mutex<Position>>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVcfRecord {
    #[getter]
    /// Get the chromosome name.
    ///
    /// # Example
    /// ```python
    /// chrom = vcf_record.chrom
    /// ```
    fn chrom(&self) -> PyResult<String> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .chrom()
            .to_string())
    }

    #[getter]
    /// Get the position (0-based).
    ///
    /// # Example
    /// ```python
    /// pos = vcf_record.pos
    /// ```
    fn pos(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .start())
    }

    /// Get an INFO field by key, returning the best Python representation.
    ///
    /// # Example
    /// ```python
    /// mq = vcf_record.info("MQ")
    /// ```
    fn info(&self, py: Python, key: &str) -> PyResult<Option<Py<PyAny>>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let header = v.record.header();
            let info_type = header
                .info_type(key.as_bytes())
                .map_err(|e| PyKeyError::new_err(format!("Invalid info key: {}", e)))?;
            let (tag_type, tag_length) = info_type;
            let mut info = v.record.info(key.as_bytes());
            match tag_type {
                htslib::bcf::header::TagType::Flag => match info.flag() {
                    Ok(b) => {
                        let bound_bool = b.into_py_any(py)?;
                        Ok(Some(bound_bool))
                    }
                    Err(e) => Err(PyValueError::new_err(format!("Invalid info key: {}", e))),
                },
                htslib::bcf::header::TagType::Integer => match info.integer() {
                    Ok(Some(values)) => match tag_length {
                        htslib::bcf::header::TagLength::Fixed(1) => {
                            Ok(Some(values[0].into_py_any(py)?))
                        }
                        _ => Ok(Some(values.to_vec().into_pyobject(py)?.unbind())),
                    },
                    Ok(None) => Ok(None),
                    Err(e) => Err(PyValueError::new_err(format!("Invalid info key: {}", e))),
                },
                htslib::bcf::header::TagType::Float => match info.float() {
                    Ok(Some(values)) => match tag_length {
                        htslib::bcf::header::TagLength::Fixed(1) => {
                            Ok(Some(values[0].into_py_any(py)?))
                        }
                        _ => Ok(Some(values.to_vec().into_pyobject(py)?.unbind())),
                    },
                    Ok(None) => Ok(None),
                    Err(e) => Err(PyValueError::new_err(format!("Invalid info key: {}", e))),
                },
                htslib::bcf::header::TagType::String => match info.string() {
                    Ok(Some(values)) => Ok(Some(values.to_vec().into_pyobject(py)?.unbind())),
                    Ok(None) => Ok(None),
                    Err(e) => Err(PyValueError::new_err(format!("Invalid info key: {}", e))),
                },
            }
        } else {
            Ok(None)
        }
    }

    /// Get a FORMAT field by key, returning per-sample values.
    ///
    /// # Example
    /// ```python
    /// gq = vcf_record.format("GQ")
    /// ```
    fn format(&self, py: Python, key: &str) -> PyResult<Option<Py<PyAny>>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let header = v.record.header();

            let info_type = header
                .format_type(key.as_bytes())
                .map_err(|e| PyKeyError::new_err(format!("Invalid format key: {}", e)))?;
            let (tag_type, _tag_length) = info_type;
            let fmt = v.record.format(key.as_bytes());
            match tag_type {
                htslib::bcf::header::TagType::Float => match fmt.float() {
                    Ok(b) => Ok(Some(b.concat().into_pyobject(py)?.unbind())),
                    Err(e) => Err(PyValueError::new_err(format!("Invalid format key: {}", e))),
                },
                htslib::bcf::header::TagType::Integer => match fmt.integer() {
                    Ok(b) => Ok(Some(b.concat().into_pyobject(py)?.unbind())),
                    Err(e) => Err(PyValueError::new_err(format!("Invalid format key: {}", e))),
                },
                _ => unimplemented!("Format flag not implemented for {:?}", tag_type),
            }
        } else {
            Ok(None)
        }
    }

    /// Set an INFO field, converting Python values into typed VCF entries.
    ///
    /// # Example
    /// ```python
    /// vcf_record.set_info("DP", 35)
    /// ```
    fn set_info(&mut self, key: &str, value: Py<PyAny>) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            Python::attach(|py| {
                let header = v.record.header();

                let (tag_type, _tag_length) = match header.info_type(key.as_bytes()) {
                    Ok(it) => it,
                    Err(e) => {
                        return Err(PyValueError::new_err(format!(
                            "INFO tag '{}' not found in header or other error: {}",
                            key, e
                        )))
                    }
                };

                match tag_type {
                    htslib::bcf::header::TagType::Flag => {
                        let val: bool = value.extract(py).map_err(|_| {
                            PyTypeError::new_err(format!(
                                "Expected bool for Flag INFO field '{}'",
                                key
                            ))
                        })?;

                        if val {
                            // Set flag to true. push_info_flag (via bcf_update_info) should overwrite if present.
                            v.record.push_info_flag(key.as_bytes()).map_err(|e| {
                                PyValueError::new_err(format!(
                                    "Failed to set flag INFO '{}' to true: {}",
                                    key, e
                                ))
                            })?;
                        } else {
                            // Set flag to false: remove the tag.
                            match v.record.clear_info_flag(key.as_bytes()) {
                                Ok(_) => {}                                              // Successfully removed
                                Err(htslib::errors::Error::BcfUndefinedTag { .. }) => {} // Tag was not present, which is fine for "false"
                                Err(e) => {
                                    return Err(PyValueError::new_err(format!(
                                        "Failed to set flag INFO '{}' to false (remove): {}",
                                        key, e
                                    )))
                                }
                            }
                        }
                    }
                    htslib::bcf::header::TagType::Integer => {
                        if let Ok(val_i32) = value.extract::<i32>(py) {
                            v.record
                                .push_info_integer(key.as_bytes(), &[val_i32])
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set integer INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else if let Ok(vals_i32) = value.extract::<Vec<i32>>(py) {
                            v.record
                                .push_info_integer(key.as_bytes(), &vals_i32)
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set integer array INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else {
                            return Err(PyTypeError::new_err(format!(
                                "Expected int or list of ints for Integer INFO field '{}'",
                                key
                            )));
                        }
                    }
                    htslib::bcf::header::TagType::Float => {
                        if let Ok(val_f32) = value.extract::<f32>(py) {
                            v.record
                                .push_info_float(key.as_bytes(), &[val_f32])
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set float INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else if let Ok(vals_f32) = value.extract::<Vec<f32>>(py) {
                            v.record
                                .push_info_float(key.as_bytes(), &vals_f32)
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set float array INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else {
                            return Err(PyTypeError::new_err(format!(
                                "Expected float or list of floats for Float INFO field '{}'",
                                key
                            )));
                        }
                    }
                    htslib::bcf::header::TagType::String => {
                        if let Ok(val_str) = value.extract::<String>(py) {
                            let val_bytes = val_str.as_bytes();
                            v.record
                                .push_info_string(key.as_bytes(), &[val_bytes])
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set string INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else if let Ok(vals_str) = value.extract::<Vec<String>>(py) {
                            let vals_bytes: Vec<&[u8]> =
                                vals_str.iter().map(|s| s.as_bytes()).collect();
                            v.record
                                .push_info_string(key.as_bytes(), &vals_bytes)
                                .map_err(|e| {
                                    PyValueError::new_err(format!(
                                        "Failed to set string array INFO '{}': {}",
                                        key, e
                                    ))
                                })?;
                        } else {
                            return Err(PyTypeError::new_err(format!(
                                "Expected string or list of strings for String INFO field '{}'",
                                key
                            )));
                        }
                    } // htslib::bcf::header::TagType::Character is another possibility,
                      // but not explicitly requested. If needed, it would be similar to String.
                }
                Ok(()) // Return Ok for the Python::with_gil closure
            })?; // Propagate PyResult from the closure
        }
        Ok(())
    }

    #[getter]
    /// Get the QUAL value if present.
    ///
    /// # Example
    /// ```python
    /// qual = vcf_record.qual
    /// ```
    fn qual(&self) -> PyResult<Option<f32>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            Ok(Some(v.record.qual()))
        } else {
            Ok(None)
        }
    }

    #[allow(non_snake_case)]
    #[getter]
    /// Get the reference allele.
    ///
    /// # Example
    /// ```python
    /// ref_allele = vcf_record.REF
    /// ```
    fn REF(&self) -> PyResult<Option<String>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let alleles = v.record.alleles();
            Ok(Some(String::from_utf8_lossy(alleles[0]).to_string()))
        } else {
            Ok(None)
        }
    }

    #[allow(non_snake_case)]
    #[setter]
    /// Update the reference allele.
    ///
    /// # Example
    /// ```python
    /// vcf_record.REF = "C"
    /// ```
    fn set_REF(&mut self, ref_allele: &str) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            let mut alleles = vec![ref_allele.as_bytes()];
            let c_alleles = v
                .record
                .alleles()
                .iter()
                .skip(1)
                .map(|&a| a.to_owned())
                .collect::<Vec<_>>();
            alleles.extend(c_alleles.iter().map(|a| &a[..]));
            v.record
                .set_alleles(&alleles)
                .map_err(|e| PyValueError::new_err(format!("Invalid ref: {}", e)))?;
        }
        Ok(())
    }
    #[allow(non_snake_case)]
    #[getter]
    /// Get all alternate alleles.
    ///
    /// # Example
    /// ```python
    /// alts = vcf_record.ALT
    /// ```
    fn ALT(&self) -> PyResult<Vec<String>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let alleles = v.record.alleles();
            if alleles.len() > 1 {
                let alt_alleles: Vec<String> = alleles[1..]
                    .iter()
                    .map(|a| String::from_utf8_lossy(a).to_string())
                    .collect();
                Ok(alt_alleles)
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    #[allow(non_snake_case)]
    #[setter]
    /// Replace alternate alleles.
    ///
    /// # Example
    /// ```python
    /// vcf_record.ALT = ["T", "G"]
    /// ```
    fn set_ALT(&mut self, alt: Vec<String>) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            let ref_allele = v.record.alleles()[0].to_owned();
            let mut alleles = vec![&ref_allele[..]];
            alleles.extend(alt.iter().map(|a| a.as_bytes()));
            v.record
                .set_alleles(alleles.as_slice())
                .map_err(|e| PyValueError::new_err(format!("Invalid alt: {}", e)))?;
        }
        Ok(())
    }

    #[getter]
    /// Get currently set FILTER values.
    ///
    /// # Example
    /// ```python
    /// filters = vcf_record.filters
    /// ```
    fn filters(&self) -> PyResult<Vec<String>> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let filters = v.record.filters();
            let mut filter_list = Vec::new();
            for filter in filters {
                let filter_name = v.record.header().id_to_name(filter);
                filter_list.push(String::from_utf8_lossy(&filter_name).to_string());
            }
            Ok(filter_list)
        } else {
            Ok(Vec::new())
        }
    }

    #[setter]
    /// Replace FILTER values with the provided list.
    ///
    /// # Example
    /// ```python
    /// vcf_record.filters = ["PASS"]
    /// ```
    fn set_filters(&mut self, filters: Vec<String>) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            let filter_bytes: Vec<_> = filters.iter().map(|f| f.as_bytes()).collect();
            v.record
                .set_filters(&filter_bytes)
                .map_err(|e| PyValueError::new_err(format!("Invalid filters: {}", e)))?;
        }
        Ok(())
    }

    #[setter]
    /// Set a single FILTER value.
    ///
    /// # Example
    /// ```python
    /// vcf_record.set_filter("LowQual")
    /// ```
    fn set_filter(&mut self, filter: &str) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            v.record
                .set_filters(&[filter.as_bytes()])
                .map_err(|e| PyValueError::new_err(format!("Invalid filter: {}", e)))?;
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    /// Get the record identifier.
    ///
    /// # Example
    /// ```python
    /// identifier = vcf_record.id
    /// ```
    fn id(&self) -> PyResult<String> {
        if let Position::Vcf(v) = &*self.inner.try_lock().expect("failed to lock interval") {
            let id = v.record.id();
            Ok(String::from_utf8_lossy(&id).to_string())
        } else {
            Ok(String::new())
        }
    }

    #[setter]
    /// Set the record identifier.
    ///
    /// # Example
    /// ```python
    /// vcf_record.id = "rs123"
    /// ```
    fn set_id(&mut self, id: &str) -> PyResult<()> {
        if let Position::Vcf(v) = &mut *self.inner.try_lock().expect("failed to lock interval") {
            v.record
                .set_id(id.as_bytes())
                .map_err(|e| PyValueError::new_err(format!("Invalid id: {}", e)))?;
        }
        Ok(())
    }
}

// Wrapper for bedder::report::ReportFragment
/// A fragment of a report containing intersection results.
///
/// Attributes:
///     a (Position, optional): The query interval
///     b (list[Position]): List of intervals that intersect with the query
///     id (int): Unique identifier for this fragment
///
/// # Example
/// ```python
/// for fragment in report:
///     anchor = fragment.a
///     for overlap in fragment:
///         print(overlap.chrom, overlap.start, overlap.stop)
/// ```
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
    /// Get the query interval if present.
    ///
    /// # Example
    /// ```python
    /// anchor = fragment.a
    /// if anchor is not None:
    ///     print(anchor.chrom)
    /// ```
    fn a(&self) -> PyResult<Option<PyPosition>> {
        match &self.inner.a {
            Some(pos) => Ok(Some(PyPosition { inner: pos.clone() })),
            None => Ok(None),
        }
    }

    #[getter]
    /// Get the list of intersecting intervals.
    ///
    /// # Example
    /// ```python
    /// overlaps = fragment.b
    /// for position in overlaps:
    ///     print(position.chrom)
    /// ```
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
    /// Get the unique identifier for this fragment.
    ///
    /// # Example
    /// ```python
    /// fragment_id = fragment.id
    /// ```
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
    /// Create a new empty Report.
    ///
    /// # Example
    /// ```python
    /// report = bedder.PyReport()
    /// ```
    fn new() -> Self {
        PyReport {
            inner: Arc::new(crate::report::Report::new(Vec::new())),
        }
    }

    /// Add a report fragment to the collection.
    ///
    /// # Example
    /// ```python
    /// report.add_fragment(fragment)
    /// ```
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

    /// Number of fragments in the report.
    ///
    /// # Example
    /// ```python
    /// size = len(report)
    /// ```
    fn __len__(&self) -> PyResult<usize> {
        Ok(self.inner.len())
    }

    /// Retrieve a fragment by index.
    ///
    /// # Example
    /// ```python
    /// first = report[0]
    /// ```
    fn __getitem__(&self, index: usize) -> PyResult<PyReportFragment> {
        Ok(PyReportFragment::from(self.inner[index].clone()))
    }

    /// Get count of overlaps for each query ID.
    ///
    /// # Example
    /// ```python
    /// overlap_counts = report.count_overlaps_by_id()
    /// ```
    fn count_overlaps_by_id(&self) -> PyResult<Vec<u64>> {
        Ok(self.inner.count_overlaps_by_id())
    }

    /// Get count of overlapping bases for each query ID.
    ///
    /// # Example
    /// ```python
    /// base_counts = report.count_bases_by_id()
    /// ```
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
///
/// # Example
/// ```python
/// fragment = report[0]
/// position = fragment.b[0] # or fragment.a
/// print(position.chrom, position.start, position.stop)
/// vcf_record = position.vcf()
/// print(vcf_record.REF)
/// ```
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyPosition {
    inner: Arc<Mutex<Position>>, // Use the trait object
}

#[pymethods]
impl PyPosition {
    /// Get the BED record if this position represents a BED interval
    ///
    /// # Example
    /// ```python
    /// bed = position.bed()
    /// ```
    ///
    /// # Raises
    ///
    /// TypeError: If the position is not a BED record.
    fn bed(&self) -> PyResult<PyBedRecord> {
        let is_bed = matches!(
            *self
                .inner
                .try_lock()
                .expect("failed to lock interval in call to .bed()"),
            Position::Bed(_)
        );
        if is_bed {
            Ok(PyBedRecord {
                inner: self.inner.clone(),
            })
        } else {
            Err(PyTypeError::new_err("position is not a BED record"))
        }
    }

    /// get the vcf record if this position represents a vcf record
    ///
    /// # Example
    /// ```python
    /// vcf = position.vcf()
    /// ```
    ///
    /// # Raises
    ///
    /// TypeError: If the position is not a VCF record.
    fn vcf(&self) -> PyResult<PyVcfRecord> {
        let is_vcf = matches!(
            *self
                .inner
                .try_lock()
                .expect("failed to lock interval in call to .vcf()"),
            Position::Vcf(_)
        );
        if is_vcf {
            Ok(PyVcfRecord {
                inner: self.inner.clone(),
            })
        } else {
            Err(PyTypeError::new_err("position is not a VCF record"))
        }
    }

    #[getter]
    /// Get the chromosome name
    ///
    /// # Example
    /// ```python
    /// chrom = position.chrom
    /// ```
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
    ///
    /// # Example
    /// ```python
    /// start = position.start
    /// ```
    fn start(&self) -> PyResult<u64> {
        Ok(self
            .inner
            .try_lock()
            .expect("failed to lock interval")
            .start())
    }

    #[getter]
    /// Get the end position (exclusive)
    ///
    /// # Example
    /// ```python
    /// stop = position.stop
    /// ```
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
            inner: IntersectionPart::Piece,
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
    ///
    /// # Example
    /// ```python
    /// anchor = intersections.base_interval
    /// ```
    fn base_interval(&self) -> PyResult<PyPosition> {
        Ok(PyPosition {
            inner: self.inner.base_interval.clone(),
        })
    }
    /// Get the base interval
    ///
    /// # Example
    /// ```python
    /// anchor = intersections.a
    /// ```
    fn a(&self) -> PyResult<PyPosition> {
        Ok(PyPosition {
            inner: self.inner.base_interval.clone(),
        })
    }

    #[getter]
    /// Get the list of overlapping intervals
    ///
    /// # Example
    /// ```python
    /// for hit in intersections.overlapping:
    ///     print(hit.chrom)
    /// ```
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
    ///
    /// # Example
    /// ```python
    /// report = intersections.report()
    /// ```
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
/// TODO: also have an eval_mod function that modifies the fragment in place.
#[derive(Debug)]
pub struct CompiledPython<'py> {
    function_name: String,
    f: Bound<'py, PyFunction>,
    ftype: Type,
    number: Number,
}

// Add this function to initialize the Python environment
pub fn initialize_python(py: Python<'_>) -> PyResult<()> {
    // Register the bedder module in sys.modules (and alias as bedder_py)
    let bedder_module = PyModule::new(py, "bedder")?;
    bedder_py(&bedder_module)?;
    let sys_modules = py.import("sys")?.getattr("modules")?;
    sys_modules.set_item("bedder", &bedder_module)?;
    sys_modules.set_item("bedder_py", &bedder_module)?;

    // Import bedder classes into the __builtins__ module so they're globally available
    let builtins = py.import("builtins")?;
    let bedder_classes = [
        "PyBedRecord",
        "PyVcfRecord",
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

impl<'py> PythonFunction<'py> {
    pub fn return_type(&self) -> &str {
        &self.return_type
    }
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
                            .find(|line| !line.is_empty())
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

impl fmt::Display for CompiledPython<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CompiledPython(function_name: {}, ftype: {}, number: {})",
            self.function_name, self.ftype, self.number
        )
    }
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

    pub fn ftype(&self) -> &Type {
        &self.ftype
    }

    pub fn number(&self) -> &Number {
        &self.number
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

/// A compiled Python boolean expression (no wrapper function required)
#[derive(Debug)]
pub struct CompiledExpr<'py> {
    code: Bound<'py, types::PyAny>,
    globals: Bound<'py, types::PyDict>,
}

impl<'py> CompiledExpr<'py> {
    /// Compile a Python expression (mode='eval') once for reuse
    pub fn new(py: Python<'py>, expr: &str) -> PyResult<Self> {
        let builtins = py.import("builtins")?;
        let compile = builtins.getattr("compile")?;
        let code = compile.call1((expr, "<bedder-filter>", "eval"))?;
        let main_module = py.import("__main__")?;
        let globals = main_module.dict();
        Ok(Self { code, globals })
    }

    /// Evaluate the expression for a given fragment; returns boolean
    pub fn eval_bool(&self, fragment: PyReportFragment) -> PyResult<bool> {
        let py = self.code.py();
        let builtins = py.import("builtins")?;
        let eval_fn = builtins.getattr("eval")?;
        let locals = pyo3::types::PyDict::new(py);
        locals.set_item("r", fragment.clone())?;
        locals.set_item("fragment", fragment)?;
        let val = eval_fn.call1((self.code.clone(), self.globals.clone(), locals))?;
        val.extract::<bool>()
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn bedder_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBedRecord>()?;
    m.add_class::<PyVcfRecord>()?;
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

#[pymodule]
fn bedder(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;
    bedder_py(m)
}
