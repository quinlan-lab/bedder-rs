use std::process::Command;

/// Run `cargo run -- map` with the given extra args, return stdout lines.
fn run_map(args: &[&str]) -> Vec<String> {
    let mut cmd_args = vec![
        "run",
        "--",
        "map",
        "-a",
        "tests/map_a.bed",
        "-b",
        "tests/map_b.bed",
        "-g",
        "tests/hg38.small.fai",
    ];
    cmd_args.extend_from_slice(args);

    let output = Command::new("cargo")
        .args(&cmd_args)
        .output()
        .expect("failed to execute bedder map");

    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn run_map_output(a_path: &str, b_path: &str, args: &[&str]) -> std::process::Output {
    let mut cmd_args = vec![
        "run",
        "--",
        "map",
        "-a",
        a_path,
        "-b",
        b_path,
        "-g",
        "tests/hg38.small.fai",
    ];
    cmd_args.extend_from_slice(args);
    Command::new("cargo")
        .args(&cmd_args)
        .output()
        .expect("failed to execute bedder map")
}

fn stdout_lines(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

#[test]
fn test_map_default_sum() {
    // All 3 B intervals overlap geneA (5+7+3=15), 1 overlaps geneB (4)
    let lines = run_map(&[]);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4");
}

#[test]
fn test_map_count() {
    let lines = run_map(&["-O", "count"]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t1");
}

#[test]
fn test_map_multiple_ops() {
    let lines = run_map(&["-c", "5", "-O", "sum,mean,count"]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t5\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t4\t1");
}

#[test]
fn test_map_numeric_name_column_four() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b_numeric_name.bed",
        &["-c", "4", "-O", "sum,mean,count"],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t5\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t4\t1");
}

#[test]
fn test_map_name_match() {
    // geneA: only geneA-named B (5+3=8), geneB: only geneB-named B (4)
    let lines = run_map(&["-n"]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t8");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4");
}

#[test]
fn test_map_group_by_b() {
    // geneA overlaps: geneA(5+3=8), geneB(7). geneB overlaps: geneB(4)
    let lines = run_map(&["-G"]);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\tgeneA\t8");
    assert_eq!(lines[1], "chr1\t100\t200\tgeneA\t10\tgeneB\t7");
    assert_eq!(lines[2], "chr1\t300\t400\tgeneB\t20\tgeneB\t4");
}

#[test]
fn test_map_group_by_b_with_name_match() {
    // Group by B name, but only keep groups matching A's name
    let lines = run_map(&["-G", "-n"]);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\tgeneA\t8");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\tgeneB\t4");
}

#[test]
fn test_map_group_by_b_with_multiple_ops() {
    let lines = run_map(&["-G", "-O", "sum,count"]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\tgeneA\t8\t2");
    assert_eq!(lines[1], "chr1\t100\t200\tgeneA\t10\tgeneB\t7\t1");
    assert_eq!(lines[2], "chr1\t300\t400\tgeneB\t20\tgeneB\t4\t1");
}

#[test]
fn test_map_b_no_name_with_name_match() {
    // B has no name column: name-match should exclude all B (no name != geneA/geneB)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "map",
            "-a",
            "tests/map_a.bed",
            "-b",
            "tests/map_b_noname.bed",
            "-g",
            "tests/hg38.small.fai",
            "-n",
            "-O",
            "count",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t0");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t0");
}

#[test]
fn test_map_zero_overlaps() {
    // geneC at chr1:500-600 has no overlapping B intervals
    let output = run_map_output(
        "tests/map_a_nohit.bed",
        "tests/map_b.bed",
        &["-O", "sum,count"],
    );

    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let lines = stdout_lines(&output);

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t3");
    assert_eq!(lines[1], "chr1\t500\t600\tgeneC\t30\t.\t0");
}

#[test]
fn test_map_a_noname_with_name_match() {
    // A has no name column; with -n, A name is treated as "."
    // B intervals have names (geneA, geneB), so none match "." → all excluded
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "map",
            "-a",
            "tests/map_a_noname.bed",
            "-b",
            "tests/map_b.bed",
            "-g",
            "tests/hg38.small.fai",
            "-n",
            "-O",
            "sum,count",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    assert_eq!(lines.len(), 2);
    // A name is "." which doesn't match any B name → empty aggregates
    assert_eq!(lines[0], "chr1\t100\t200\t.\t0");
    assert_eq!(lines[1], "chr1\t300\t400\t.\t0");
}

#[test]
fn test_map_python_operation_only() {
    let lines = run_map(&["--python", "tests/map_ops.py", "-O", "py:sum_plus_one"]);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t16");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t5");
}

#[test]
fn test_map_python_mixed_with_builtins() {
    let lines = run_map(&[
        "--python",
        "tests/map_ops.py",
        "-c",
        "5",
        "-O",
        "sum,py:sum_plus_one,count",
    ]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t16\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t5\t1");
}

#[test]
fn test_map_python_column_extractor_bed() {
    let lines = run_map(&[
        "--python",
        "tests/map_ops.py",
        "-c",
        "py:bed_score",
        "-O",
        "sum,mean,count",
    ]);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t5\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t4\t1");
}

#[test]
fn test_map_python_column_extractor_vcf_dp() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.vcf",
        &[
            "--python",
            "tests/map_ops.py",
            "-c",
            "py:vcf_dp",
            "-O",
            "sum,mean,count",
        ],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t5\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t4\t1");
}

#[test]
fn test_map_python_column_extractor_vcf_missing_info_skips_value() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b_missing_dp.vcf",
        &[
            "--python",
            "tests/map_ops.py",
            "-c",
            "py:vcf_dp",
            "-O",
            "sum,mean,count",
        ],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    // sum/mean skip the missing DP at chr1:161, while count still counts overlaps.
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t15\t5\t4");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t4\t4\t1");
}

#[test]
fn test_map_python_column_extractor_vcf_multivalue_info() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.vcf",
        &[
            "--python",
            "tests/map_ops.py",
            "-c",
            "py:vcf_af_first",
            "-O",
            "sum,mean,count",
        ],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    let first: Vec<&str> = lines[0].split('\t').collect();
    assert_eq!(first.len(), 8);
    assert_eq!(&first[..5], ["chr1", "100", "200", "geneA", "10"]);
    let sum1 = first[5]
        .parse::<f64>()
        .expect("first AF sum should parse as float");
    let mean1 = first[6]
        .parse::<f64>()
        .expect("first AF mean should parse as float");
    assert!((sum1 - 0.6).abs() < 1e-6, "unexpected AF sum: {}", sum1);
    assert!((mean1 - 0.2).abs() < 1e-6, "unexpected AF mean: {}", mean1);
    assert_eq!(first[7], "3");

    let second: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(second.len(), 8);
    assert_eq!(&second[..5], ["chr1", "300", "400", "geneB", "20"]);
    let sum2 = second[5]
        .parse::<f64>()
        .expect("second AF sum should parse as float");
    let mean2 = second[6]
        .parse::<f64>()
        .expect("second AF mean should parse as float");
    assert!((sum2 - 0.4).abs() < 1e-6, "unexpected AF sum: {}", sum2);
    assert!((mean2 - 0.4).abs() < 1e-6, "unexpected AF mean: {}", mean2);
    assert_eq!(second[7], "1");
}

#[test]
fn test_map_group_by_b_python_operation() {
    let lines = run_map(&[
        "-G",
        "--python",
        "tests/map_ops.py",
        "-O",
        "py:sum_plus_one",
    ]);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\tgeneA\t9");
    assert_eq!(lines[1], "chr1\t100\t200\tgeneA\t10\tgeneB\t8");
    assert_eq!(lines[2], "chr1\t300\t400\tgeneB\t20\tgeneB\t5");
}

#[test]
fn test_map_python_empty_overlap_behavior() {
    let output = run_map_output(
        "tests/map_a_nohit.bed",
        "tests/map_b.bed",
        &["--python", "tests/map_ops.py", "-O", "py:empty_marker"],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t3");
    assert_eq!(lines[1], "chr1\t500\t600\tgeneC\t30\tEMPTY");
}

#[test]
fn test_map_python_bool_output_contract() {
    let output = run_map_output(
        "tests/map_a_nohit.bed",
        "tests/map_b.bed",
        &["--python", "tests/map_ops.py", "-O", "py:has_values"],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t1");
    assert_eq!(lines[1], "chr1\t500\t600\tgeneC\t30\t0");
}

#[test]
fn test_map_python_operation_requires_python_file() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &["-O", "py:sum_plus_one"],
    );
    assert!(
        !output.status.success(),
        "bedder map should fail without --python"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("python operations/extractors require --python <file>"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_map_python_column_extractor_requires_python_file() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &["-c", "py:bed_score"],
    );
    assert!(
        !output.status.success(),
        "bedder map should fail without --python"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("python operations/extractors require --python <file>"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_map_python_missing_function_error() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &["--python", "tests/map_ops.py", "-O", "py:not_defined"],
    );
    assert!(
        !output.status.success(),
        "bedder map should fail for missing function"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("failed to compile python operation 'py:not_defined'"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_map_python_column_extractor_missing_function_error() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &["--python", "tests/map_ops.py", "-c", "py:not_defined"],
    );
    assert!(
        !output.status.success(),
        "bedder map should fail for missing extractor function"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("failed to compile python column extractor 'py:not_defined'"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_map_python_column_extractor_non_numeric_error() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &["--python", "tests/map_ops.py", "-c", "py:bad_numeric"],
    );
    assert!(
        !output.status.success(),
        "bedder map should fail for non-numeric extractor return"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("returned non-numeric value"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_map_count_does_not_invoke_python_extractor() {
    let output = run_map_output(
        "tests/map_a.bed",
        "tests/map_b.bed",
        &[
            "--python",
            "tests/map_ops.py",
            "-c",
            "py:bad_numeric",
            "-O",
            "count",
        ],
    );
    assert!(
        output.status.success(),
        "bedder map failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = stdout_lines(&output);
    assert_eq!(lines[0], "chr1\t100\t200\tgeneA\t10\t3");
    assert_eq!(lines[1], "chr1\t300\t400\tgeneB\t20\t1");
}

#[test]
fn test_map_column_zero_error() {
    // -c 0 should be rejected (columns are 1-indexed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "map",
            "-a",
            "tests/map_a.bed",
            "-b",
            "tests/map_b.bed",
            "-g",
            "tests/hg38.small.fai",
            "-c",
            "0",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(
        !output.status.success(),
        "bedder map should have failed with -c 0"
    );
}
