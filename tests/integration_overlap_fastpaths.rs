use std::fs;
use std::process::Command;

#[test]
fn overlap_output_preserves_modes_requirements_and_python_filter() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.bed");
    let b = temp.path().join("b.bed");
    let genome = temp.path().join("genome.fai");
    fs::write(&a, "chr1\t10\t20\tA1\nchr1\t30\t40\tA2\n").unwrap();
    fs::write(&b, "chr1\t12\t13\tB1\n").unwrap();
    fs::write(&genome, "chr1\t100\n").unwrap();

    let cases: &[(&[&str], &str)] = &[
        (&[], "chr1\t10\t20\tA1\n"),
        (&["-m", "not"], "chr1\t30\t40\tA2\n"),
        (
            &["-r", "0", "-R", "0"],
            "chr1\t10\t20\tA1\nchr1\t30\t40\tA2\n",
        ),
        (
            &["-r", "0%", "-R", "0%"],
            "chr1\t10\t20\tA1\nchr1\t30\t40\tA2\n",
        ),
        (&["-r", "50%"], ""),
        (&["-m", "piece", "-r", "0", "-R", "0"], "chr1\t10\t20\tA1\n"),
    ];
    for (flags, expected) in cases {
        for use_python in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_bedder"));
            command
                .arg("intersect")
                .arg("-a")
                .arg(&a)
                .arg("-b")
                .arg(&b)
                .arg("-g")
                .arg(&genome)
                .args(["-p", "whole-wide", "-P", "none"])
                .args(*flags);
            if use_python {
                command.args(["--filter", "True"]);
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                *expected,
                "flags={flags:?}, python={use_python}"
            );
        }
    }
}

#[test]
fn python_file_is_executed_without_columns_or_filter() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.bed");
    let b = temp.path().join("b.bed");
    let genome = temp.path().join("genome.fai");
    let python = temp.path().join("raise.py");
    fs::write(&a, "chr1\t10\t20\n").unwrap();
    fs::write(&b, "chr1\t12\t13\n").unwrap();
    fs::write(&genome, "chr1\t100\n").unwrap();
    fs::write(&python, "raise RuntimeError('python-file-was-executed')\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_bedder"))
        .arg("intersect")
        .arg("-a")
        .arg(&a)
        .arg("-b")
        .arg(&b)
        .arg("-g")
        .arg(&genome)
        .arg("--python")
        .arg(&python)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("python-file-was-executed"));
}
