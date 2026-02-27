use std::process::Command;

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
