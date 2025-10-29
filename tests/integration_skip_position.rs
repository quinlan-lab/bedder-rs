use std::fs;
use std::process::Command;

/// Integration tests for calculate_skip_position function
/// These tests verify that the skip position optimization works correctly in end-to-end scenarios

#[test]
fn test_skip_position_integration_overlap_only() {
    // Test basic overlap scenario where skip optimization should occur
    // Query intervals have large gaps from database intervals
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "intersect",
            "-a",
            "tests/skip_position_test_query.bed",
            "-b",
            "tests/skip_position_test_db.bed",
            "-g",
            "tests/skip_position_test_fai",
        ])
        .output()
        .expect("Failed to execute bedder");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    // Should succeed and produce intersections
    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        stderr
    );

    // The output should contain specific intersections
    // chr1:1000-2000 should not overlap with any db intervals (no skip needed, gap too small)
    // chr1:100000-110000 should not overlap with any db intervals but should skip due to large gap
    // chr1:500000-510000 should not overlap with any db intervals but should skip
    // chr2:10000-20000 should not overlap with any db intervals but should skip due to large gap
    // chr2:200000-210000 should not overlap with any db intervals but should skip

    // Since we're looking for overlaps and there are no actual overlaps in this test data,
    // the output should be empty or contain only headers
    let lines: Vec<&str> = stdout.lines().collect();

    // Filter out any header or empty lines
    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .cloned()
        .collect();

    // No overlaps should be found
    assert_eq!(
        data_lines.len(),
        0,
        "Expected no overlaps but found: {:?}",
        data_lines
    );
}

#[test]
fn test_skip_position_integration_with_max_distance() {
    // Test with max_distance parameter to verify skip optimization with distance constraints
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "closest",
            "-a",
            "tests/skip_position_test_query.bed",
            "-b",
            "tests/skip_position_test_db.bed",
            "-g",
            "tests/skip_position_test_fai",
            "-d",
            "50000", // max_distance of 50kb
        ])
        .output()
        .expect("Failed to execute bedder");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        stderr
    );

    // With max_distance = 50000:
    // chr1:1000-2000 should find chr1:5000-6000 (distance = 3000 < 50000)
    // chr1:100000-110000 should find chr1:300000-310000 (distance = 190000 > 50000, not found)
    // chr2:10000-20000 should find chr2:50000-60000 (distance = 30000 < 50000)

    let lines: Vec<&str> = stdout.lines().collect();
    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .cloned()
        .collect();

    // Should find some intervals within max_distance
    assert!(
        data_lines.len() > 0,
        "Expected to find some intervals within max_distance"
    );

    // Verify specific expected matches
    let has_chr1_query1 = data_lines
        .iter()
        .any(|line| line.contains("1000") && line.contains("2000"));
    let has_chr2_query4 = data_lines
        .iter()
        .any(|line| line.contains("10000") && line.contains("20000"));

    assert!(
        has_chr1_query1 || has_chr2_query4,
        "Expected to find at least one of the close matches"
    );
}

#[test]
fn test_skip_position_integration_cross_chromosome() {
    // Create test data that spans multiple chromosomes to test cross-chromosome skipping

    // First create a more complex test case
    let query_content = "chr1\t1000\t2000\tquery1\nchr3\t10000\t20000\tquery2\n";
    let db_content = "chr2\t5000\t6000\tdb1\nchr3\t15000\t16000\tdb2\n";

    fs::write("tests/temp_query_cross_chrom.bed", query_content).unwrap();
    fs::write("tests/temp_db_cross_chrom.bed", db_content).unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "intersect",
            "-a",
            "tests/temp_query_cross_chrom.bed",
            "-b",
            "tests/temp_db_cross_chrom.bed",
            "-g",
            "tests/skip_position_test_fai",
        ])
        .output()
        .expect("Failed to execute bedder");

    // Clean up temp files
    let _ = fs::remove_file("tests/temp_query_cross_chrom.bed");
    let _ = fs::remove_file("tests/temp_db_cross_chrom.bed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        stderr
    );

    // Should find the overlap between chr3:10000-20000 and chr3:15000-16000
    let lines: Vec<&str> = stdout.lines().collect();
    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .cloned()
        .collect();

    // Should find exactly one overlap
    assert_eq!(
        data_lines.len(),
        1,
        "Expected exactly one overlap but found: {:?}",
        data_lines
    );

    let overlap_line = data_lines[0];
    assert!(overlap_line.contains("chr3"), "Expected overlap on chr3");
    assert!(
        overlap_line.contains("10000") && overlap_line.contains("20000"),
        "Expected query interval"
    );
}

#[test]
fn test_skip_position_performance_large_gaps() {
    // Create a test that specifically triggers the skip optimization
    // Large gaps between intervals should cause skipping and improve performance

    let query_content = "chr1\t1000\t2000\tquery1\nchr1\t1000000\t1001000\tquery2\n";
    let db_content = "chr1\t500000\t500100\tdb1\nchr1\t2000000\t2000100\tdb2\n";

    fs::write("tests/temp_query_large_gaps.bed", query_content).unwrap();
    fs::write("tests/temp_db_large_gaps.bed", db_content).unwrap();

    let start = std::time::Instant::now();

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "intersect",
            "-a",
            "tests/temp_query_large_gaps.bed",
            "-b",
            "tests/temp_db_large_gaps.bed",
            "-g",
            "tests/skip_position_test_fai",
        ])
        .output()
        .expect("Failed to execute bedder");

    let duration = start.elapsed();

    // Clean up temp files
    let _ = fs::remove_file("tests/temp_query_large_gaps.bed");
    let _ = fs::remove_file("tests/temp_db_large_gaps.bed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);
    println!("Execution time: {:?}", duration);

    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        stderr
    );

    // The test should complete relatively quickly due to skip optimization
    // This is more of a smoke test to ensure the functionality works
    assert!(
        duration.as_secs() < 200,
        "Test took too long, skip optimization may not be working"
    );

    // No overlaps expected in this test
    let lines: Vec<&str> = stdout.lines().collect();
    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .cloned()
        .collect();

    assert_eq!(
        data_lines.len(),
        0,
        "Expected no overlaps but found: {:?}",
        data_lines
    );
}

#[test]
fn test_skip_position_with_closest_intervals() {
    // Test n_closest parameter - skip optimization should be disabled
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "closest",
            "-a",
            "tests/skip_position_test_query.bed",
            "-b",
            "tests/skip_position_test_db.bed",
            "-g",
            "tests/skip_position_test_fai",
            "-n",
            "2", // Find 2 closest intervals
        ])
        .output()
        .expect("Failed to execute bedder");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        output.status.success(),
        "Command failed with stderr: {}",
        stderr
    );

    // With n_closest, should find closest intervals even across large gaps
    let lines: Vec<&str> = stdout.lines().collect();
    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .cloned()
        .collect();

    // Should find multiple closest intervals
    assert!(data_lines.len() > 0, "Expected to find closest intervals");
}
