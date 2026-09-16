use std::process::Command;

#[test]
fn python_docstring_becomes_vcf_info_description() {
    let temp = tempfile::tempdir().unwrap();
    let query = temp.path().join("query.vcf");
    let target = temp.path().join("target.bed");
    let genome = temp.path().join("genome.fai");
    let python = temp.path().join("columns.py");
    let output = temp.path().join("annotated.vcf");
    std::fs::write(
        &query,
        "##fileformat=VCFv4.2\n##contig=<ID=chr1,length=100>\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t11\t.\tA\tT\t.\tPASS\t.\n",
    )
    .unwrap();
    std::fs::write(&target, "chr1\t10\t20\trepeat\n").unwrap();
    std::fs::write(&genome, "chr1\t100\n").unwrap();
    std::fs::write(
        &python,
        "def bedder_example(fragment) -> str:\n    \"\"\"Explains the annotation.\n\n    Additional details.\n    \"\"\"\n    return 'value'\n\ndef bedder_no_doc(fragment) -> int:\n    return 1\n",
    )
    .unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_bedder"))
        .args(["intersect", "-a"])
        .arg(&query)
        .arg("-b")
        .arg(&target)
        .arg("-g")
        .arg(&genome)
        .arg("--python")
        .arg(&python)
        .args(["-c", "py:example", "-c", "py:no_doc", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let vcf = std::fs::read_to_string(output).unwrap();
    assert!(vcf.contains(
        "##INFO=<ID=example,Number=1,Type=String,Description=\"Explains the annotation.\">"
    ));
    assert!(vcf.contains("##INFO=<ID=no_doc,Number=1,Type=Integer,Description=\"no_doc\">"));
}

#[test]
fn test_intersect_python_expression_compile_error_is_reported() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "intersect",
            "-a",
            "tests/map_a.bed",
            "-b",
            "tests/map_b.bed",
            "-g",
            "tests/hg38.small.fai",
            "-c",
            "bad:Integer:bad:1:py:not_defined",
        ])
        .output()
        .expect("failed to execute bedder intersect");

    assert!(
        !output.status.success(),
        "bedder intersect should fail for missing python expression function"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to compile python expression 'py:not_defined'"),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("thread 'main' panicked"),
        "expected graceful error, got panic:\n{}",
        stderr
    );
}
