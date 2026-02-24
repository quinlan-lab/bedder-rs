use std::fs;
use std::process::Command;

#[test]
fn test_a_piece_whole_prints_full_a_per_overlap() {
    let a_path = "tests/temp_a_piece_whole_long_a.bed";
    let b_path = "tests/temp_a_piece_whole_long_b.bed";

    // Use a valid BED5 score and BED6 strand so we can assert an "extra" field (foo) is preserved.
    fs::write(a_path, "chr1\t10\t20\tA1\t0\t+\tfoo\n").unwrap();
    fs::write(b_path, "chr1\t11\t12\tB1\nchr1\t13\t15\tB2\n").unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "intersect",
            "-a",
            a_path,
            "-b",
            b_path,
            "-g",
            "tests/hg38.small.fai",
            "--a-piece",
            "whole",
        ])
        .output()
        .expect("Failed to execute bedder");

    let _ = fs::remove_file(a_path);
    let _ = fs::remove_file(b_path);

    assert!(
        output.status.success(),
        "Command failed.\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();

    assert_eq!(
        data_lines.len(),
        2,
        "Expected 2 intersections, got:\n{}",
        stdout
    );

    assert!(
        data_lines[0].contains("\t10\t20\tA1\t0\t+\tfoo\tchr1\t11\t12"),
        "Expected full A with first B overlap, got: {}",
        data_lines[0]
    );
    assert!(
        data_lines[1].contains("\t10\t20\tA1\t0\t+\tfoo\tchr1\t13\t15"),
        "Expected full A with second B overlap, got: {}",
        data_lines[1]
    );
}
