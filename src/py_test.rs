#[cfg(test)]
mod tests {
    use crate::bedder_bed::BedRecord;
    use crate::column::{Number, Type, Value};
    use crate::intersection::{Intersection, Intersections};
    use crate::position::Position;
    use crate::py::{CompiledPython, PyReportFragment};
    use crate::report_options::ReportOptions;
    use parking_lot::Mutex;
    use pyo3::Python;
    use std::sync::Arc;

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
        Python::with_gil(|py| {
            let code = r#"
            def bedder(fragment):
                chrom = fragment.chrom
                start = fragment.start
                return f"{chrom}:{start}"
            "#;

            let compiled = CompiledPython::new(py, code, Type::String, Number::One).unwrap();
            let intersections = create_test_intersection();
            let report_options = Arc::new(ReportOptions::default());
            let report = intersections.report(&report_options);
            for frag in report.iter() {
                let py_fragment = PyReportFragment::new(frag.clone());
                let result = compiled.eval(py_fragment).unwrap();
                assert_eq!(result, Value::String("chr1:100".to_string()));
            }
        });
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
