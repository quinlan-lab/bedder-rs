#[cfg(test)]
mod tests {
    use crate::bedder_bed::BedRecord;
    use crate::bedder_vcf::BedderRecord;
    use crate::column::Value;
    use crate::hts_format::Format as BedderFormat;
    use crate::intersection::{Intersection, Intersections};
    use crate::position::Position;
    use crate::py::{CompiledExpr, CompiledMapPython, CompiledPython, PyReportFragment};
    use crate::report_options::ReportOptions;
    use crate::writer::{InputHeader, Writer};
    use parking_lot::Mutex;
    use pyo3::exceptions::PyRuntimeError;
    use pyo3::exceptions::PyValueError;
    use pyo3::types::PyModuleMethods;
    use pyo3::types::{PyDict, PyDictMethods};
    use pyo3::Py;
    use pyo3::PyResult;
    use pyo3::Python;
    use rust_htslib::bcf::header::Header;
    use rust_htslib::bcf::Read;
    use rust_htslib::bcf::{Format, Reader, Writer as BCFWriter};
    use std::ffi::CString;
    use std::fs::File as StdFile;
    use std::io::Read as _;
    use std::sync::{Arc, Once};
    use tempfile::NamedTempFile;

    fn ensure_python_initialized() {
        static INIT: Once = Once::new();
        INIT.call_once(|| Python::initialize());
    }

    fn create_test_intersection() -> Intersections {
        // Create a base interval
        let base = BedRecord::new("chr1", 100, 200, None, None, vec![]);
        let base_pos = Position::Bed(base);

        // Create an overlapping interval
        let overlap = BedRecord::new("chr1", 150, 250, None, None, vec![]);
        let overlap_pos = Position::Bed(overlap);

        // Create intersections with proper Intersection struct
        Intersections {
            base_interval: Arc::new(Mutex::new(base_pos)),
            overlapping: vec![Intersection {
                interval: Arc::new(Mutex::new(overlap_pos)),
                id: 0,
            }],
            cached_report: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn test_simple_snippet() {
        ensure_python_initialized();
        Python::attach(|py| -> PyResult<()> {
            let code = r#"
def bedder_test_func(fragment) -> str:
    chrom = fragment.a.chrom
    start = fragment.a.start
    return f"{chrom}:{start}"
            "#;

            let c_code = CString::new(code)
                .map_err(|_| PyRuntimeError::new_err("Failed to convert code to CString"))?;
            py.run(&c_code, None, None)?;
            let main_module = py.import("__main__")?;
            let globals = main_module.dict();

            let functions_map = crate::py::introspect_python_functions(py, globals)?;
            eprintln!("functions: {:?}", functions_map);

            let compiled = CompiledPython::new(py, "test_func", &functions_map)?;
            let intersections = create_test_intersection();
            let report_options = Arc::new(ReportOptions::default());
            let report = intersections.report(&report_options);
            for frag in report.iter() {
                let py_fragment = PyReportFragment::new(frag.clone());
                let result = compiled.eval(py_fragment).unwrap();
                assert_eq!(result, Value::String("chr1:100".to_string()));
            }
            Ok(())
        })
        .expect("Failed to run test");
    }

    #[test]
    fn test_compiled_map_python_value_conversions() {
        ensure_python_initialized();
        Python::attach(|py| -> PyResult<()> {
            let code = r#"
def bedder_map_count(values) -> int:
    return len(values)

def bedder_map_mean(values) -> float:
    if len(values) == 0:
        return 0.0
    return sum(values) / len(values)

def bedder_map_has(values) -> bool:
    return len(values) > 0

def bedder_map_join(values) -> str:
    return ",".join(str(int(v)) for v in values)
"#;
            let c_code = CString::new(code)?;
            py.run(&c_code, None, None)?;
            let globals = py.import("__main__")?.dict();
            let functions_map = crate::py::introspect_python_functions(py, globals)?;

            let count_fn = CompiledMapPython::new("map_count", &functions_map)?;
            let mean_fn = CompiledMapPython::new("map_mean", &functions_map)?;
            let has_fn = CompiledMapPython::new("map_has", &functions_map)?;
            let join_fn = CompiledMapPython::new("map_join", &functions_map)?;

            assert_eq!(count_fn.eval_values(&[1.0, 2.0, 3.0])?, "3");
            assert_eq!(mean_fn.eval_values(&[1.0, 2.0])?, "1.5");
            assert_eq!(mean_fn.eval_values(&[1.0, 3.0])?, "2");
            assert_eq!(has_fn.eval_values(&[1.0])?, "1");
            assert_eq!(has_fn.eval_values(&[])?, "0");
            assert_eq!(join_fn.eval_values(&[1.0, 2.0])?, "1,2");

            Ok(())
        })
        .expect("map python conversion test failed");
    }

    #[test]
    fn test_vcf_info() {
        ensure_python_initialized();
        Python::attach(|py| -> PyResult<()> {
            crate::py::initialize_python(py)?;

            let mut raw_header = Header::new();
            raw_header.push_record(b"##fileformat=VCFv4.2");
            raw_header.push_record(b"##contig=<ID=chr1,length=10000>");
            raw_header.push_record(b"##INFO=<ID=FLAG,Number=0,Type=Flag,Description=\"A flag\">");
            raw_header.push_record(b"##INFO=<ID=INT,Number=1,Type=Integer,Description=\"An int\">");
            raw_header.push_record(b"##INFO=<ID=INTS,Number=.,Type=Integer,Description=\"Ints\">");
            raw_header
                .push_record(b"##INFO=<ID=FLOAT,Number=1,Type=Float,Description=\"A float\">");
            raw_header
                .push_record(b"##INFO=<ID=STR,Number=1,Type=String,Description=\"A string\">");

            //let header_view = Arc::new(HeaderView::new(inner_ptr));

            let temp_file = NamedTempFile::new().expect("failed to create temp file");
            let writer = BCFWriter::from_path(temp_file.path(), &raw_header, true, Format::Vcf)
                .expect("failed to create writer");
            drop(writer);
            let vcf = Reader::from_path(temp_file.path()).expect("failed to open reader");

            let mut record = vcf.empty_record();
            let rid = vcf
                .header()
                .name2rid(b"chr1")
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            record.set_rid(Some(rid));
            record.set_pos(100);
            record
                .set_alleles(&[b"A", b"T"])
                .map_err(|e| PyValueError::new_err(e.to_string()))?;

            let bedder_record = BedderRecord::new(record);
            let position = Position::Vcf(Box::new(bedder_record));
            let py_vcf = crate::py::PyVcfRecord::new(Arc::new(Mutex::new(position)));

            let py_vcf_bound = Py::new(py, py_vcf).unwrap();
            let globals = PyDict::new(py);
            globals.set_item("vcf_record", py_vcf_bound)?;

            let code = r#"
assert vcf_record.chrom == "chr1"
assert vcf_record.pos == 100

assert vcf_record.info("INT") is None

# entries with Number=1 are returned as a single value
vcf_record.set_info("INT", 42)
assert vcf_record.info("INT") == 42

vcf_record.set_info("FLOAT", 3.14)
f = round(vcf_record.info("FLOAT"), 2)
assert f == round(3.14, 2), f

vcf_record.set_info("STR", "hello")
assert vcf_record.info("STR") == [b'hello']

vcf_record.set_info("FLAG", True)
assert vcf_record.info("FLAG") == True

#vcf_record.set_info("FLAG", False)
#assert vcf_record.info("FLAG") == False, vcf_record.info("FLAG")

vcf_record.set_info("INTS", [1,2,3])
assert vcf_record.info("INTS") == [1,2,3]
"#;
            let c_code = CString::new(code)?;
            py.run(&c_code, Some(&globals), None)?;

            Ok(())
        })
        .expect("test failed");
    }

    #[test]
    fn test_vcf_filters_returns_names() {
        ensure_python_initialized();
        Python::attach(|py| -> PyResult<()> {
            crate::py::initialize_python(py)?;

            let mut raw_header = Header::new();
            raw_header.push_record(b"##fileformat=VCFv4.2");
            raw_header.push_record(b"##contig=<ID=chr1,length=10000>");
            raw_header.push_record(b"##FILTER=<ID=q10,Description=\"Quality below 10\">");
            raw_header.push_record(b"##FILTER=<ID=lowdp,Description=\"Depth below 10\">");

            let temp_file = NamedTempFile::new().expect("failed to create temp file");
            let writer = BCFWriter::from_path(temp_file.path(), &raw_header, true, Format::Vcf)
                .expect("failed to create writer");
            drop(writer);

            let vcf = Reader::from_path(temp_file.path()).expect("failed to open reader");
            let mut record = vcf.empty_record();
            let rid = vcf
                .header()
                .name2rid(b"chr1")
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            record.set_rid(Some(rid));
            record.set_pos(123);
            record
                .set_alleles(&[b"A", b"T"])
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            record
                .push_filter("q10".as_bytes())
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            record
                .push_filter("lowdp".as_bytes())
                .map_err(|e| PyValueError::new_err(e.to_string()))?;

            let bedder_record = BedderRecord::new(record);
            let position = Position::Vcf(Box::new(bedder_record));
            let py_vcf = crate::py::PyVcfRecord::new(Arc::new(Mutex::new(position)));

            let py_vcf_bound = Py::new(py, py_vcf).unwrap();
            let globals = PyDict::new(py);
            globals.set_item("vcf_record", py_vcf_bound)?;

            let code = r#"
filters = sorted(vcf_record.filters)
assert filters == ["lowdp", "q10"], filters
"#;
            let c_code = CString::new(code)?;
            py.run(&c_code, Some(&globals), None)?;

            Ok(())
        })
        .expect("filters test failed");
    }

    #[test]
    fn test_writer_applies_filter() {
        ensure_python_initialized();
        Python::attach(|py| -> PyResult<()> {
            // Define two filter functions
            let code = r#"
def bedder_filter_true(fragment) -> bool:
    return True
def bedder_filter_false(fragment) -> bool:
    return False
"#;
            let c_code = CString::new(code)?;
            py.run(&c_code, None, None)?;

            // Compile simple boolean expressions directly
            let compiled_true = CompiledExpr::new(py, "True")?;
            let compiled_false = CompiledExpr::new(py, "False")?;

            // Prepare intersections and writer (BED)
            let mut intersections = create_test_intersection();
            let tmp1 = NamedTempFile::new().expect("tmp bed1");
            let mut writer1 = Writer::init(
                tmp1.path().to_str().unwrap(),
                Some(BedderFormat::Bed),
                None,
                InputHeader::None,
                &[],
            )
            .expect("init writer1");

            // Explicitly type the empty columns slice to satisfy ColumnReporter generic
            let cols: &[crate::column::Column<'_>] = &[];

            writer1
                .write(
                    &mut intersections,
                    Arc::new(ReportOptions::default()),
                    cols,
                    Some(&compiled_true),
                )
                .expect("write with true filter");

            // Ensure data is flushed to disk before reading
            drop(writer1);

            // Read file and ensure non-empty
            let mut s = String::new();
            StdFile::open(tmp1.path())
                .unwrap()
                .read_to_string(&mut s)
                .unwrap();
            assert!(
                !s.trim().is_empty(),
                "Output should not be empty when filter is true",
            );
            eprintln!("s: {}", s);

            // Now write with false filter to a new file
            let mut intersections2 = create_test_intersection();
            let tmp2 = NamedTempFile::new().expect("tmp bed2");
            let mut writer2 = Writer::init(
                tmp2.path().to_str().unwrap(),
                Some(BedderFormat::Bed),
                None,
                InputHeader::None,
                &[],
            )
            .expect("init writer2");

            writer2
                .write(
                    &mut intersections2,
                    Arc::new(ReportOptions::default()),
                    cols,
                    Some(&compiled_false),
                )
                .expect("write with false filter");

            // Ensure data is flushed to disk before reading
            drop(writer2);

            let mut s2 = String::new();
            StdFile::open(tmp2.path())
                .unwrap()
                .read_to_string(&mut s2)
                .unwrap();
            assert!(
                s2.trim().is_empty(),
                "Output should be empty when filter is false"
            );

            Ok(())
        })
        .expect("filter test failed");
    }

    /*
    #[test]
    fn test_full_function() {
        Python::with_gil(|py| {
            let code = r#"def bedder(intersection):
                overlaps = len(intersection.overlapping)
                return str(overlaps)
            "#;

            let compiled = CompiledPython::new(py, code, false).unwrap();
            let result = compiled.eval(create_test_intersection()).unwrap();
            assert_eq!(result, "1");
        });
    }

    #[test]
    fn test_invalid_python_code() {
        Python::with_gil(|py| {
            let code = "this is not valid python";
            let result = CompiledPython::new(py, code, true);
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_non_string_return() {
        Python::with_gil(|py| {
            let code = "return 42"; // Returns integer instead of string
            let compiled = CompiledPython::new(py, code, true).unwrap();
            let result = compiled.eval(create_test_intersection());
            assert!(result.is_ok(), "Should convert non-string to string");
            assert_eq!(result.unwrap(), "42");
        });
    }

    #[test]
    fn test_complex_snippet() {
        Python::with_gil(|py| {
            let code = r#"
            result = []
            for overlap in intersection.overlapping:
                result.append(f"{overlap.chrom}:{overlap.start}-{overlap.stop}")
            return ",".join(result)
            "#;

            let compiled = CompiledPython::new(py, code, true).unwrap();
            let result = compiled.eval(create_test_intersection()).unwrap();
            assert_eq!(result, "chr1:150-250");
        });
    }

    #[test]
    fn test_missing_return() {
        Python::with_gil(|py| {
            let code = r#"
            x = 42
            "#;

            let compiled = CompiledPython::new(py, code, true);
            assert!(compiled.is_err(), "Should fail when no value is returned");
            //let result = compiled.eval(create_test_intersection());
            //assert!(result.is_err(), "Should fail when no value is returned");
        });
    }
    */
}
