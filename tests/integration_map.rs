use std::process::Command;

/// Run `cargo run -- map` with the given extra args, return stdout lines.
fn run_map(args: &[&str]) -> Vec<String> {
    let mut cmd_args = vec![
        "run", "--", "map",
        "-a", "tests/map_a.bed",
        "-b", "tests/map_b.bed",
        "-g", "tests/hg38.small.fai",
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
            "run", "--", "map",
            "-a", "tests/map_a.bed",
            "-b", "tests/map_b_noname.bed",
            "-g", "tests/hg38.small.fai",
            "-n", "-O", "count",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(output.status.success(), "bedder map failed:\n{}", String::from_utf8_lossy(&output.stderr));

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
    let output = Command::new("cargo")
        .args([
            "run", "--", "map",
            "-a", "tests/map_a_nohit.bed",
            "-b", "tests/map_b.bed",
            "-g", "tests/hg38.small.fai",
            "-O", "sum,count",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(output.status.success(), "bedder map failed:\n{}", String::from_utf8_lossy(&output.stderr));

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

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
            "run", "--", "map",
            "-a", "tests/map_a_noname.bed",
            "-b", "tests/map_b.bed",
            "-g", "tests/hg38.small.fai",
            "-n", "-O", "sum,count",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(output.status.success(), "bedder map failed:\n{}", String::from_utf8_lossy(&output.stderr));

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
fn test_map_column_zero_error() {
    // -c 0 should be rejected (columns are 1-indexed)
    let output = Command::new("cargo")
        .args([
            "run", "--", "map",
            "-a", "tests/map_a.bed",
            "-b", "tests/map_b.bed",
            "-g", "tests/hg38.small.fai",
            "-c", "0",
        ])
        .output()
        .expect("failed to execute bedder map");

    assert!(!output.status.success(), "bedder map should have failed with -c 0");
}
