#!/usr/bin/env python3
"""Correctness-gated native VCF-versus-BED benchmark for bedder and BEDTools."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil
import statistics
import subprocess


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vcf", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--genome", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True, help="BED4 membership truth set")
    parser.add_argument("--bedder", type=Path, default=Path("bedder"))
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--output-root", type=Path, required=True)
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def expected_keys(path: Path) -> set[tuple[str, int, int]]:
    keys: set[tuple[str, int, int]] = set()
    with path.open() as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 3:
                raise ValueError(f"{path}:{line_number}: expected BED3")
            keys.add((fields[0], int(fields[1]), int(fields[2])))
    return keys


def vcf_keys(path: Path) -> list[tuple[str, int, int]]:
    """Extract VCF interval coordinates through bcftools, ignoring headers."""
    result = subprocess.run(
        ["bcftools", "query", "-f", "%CHROM\\t%POS0\\t%END\\n", str(path)],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"bcftools query failed for {path}: {result.stderr}")
    keys = []
    for line_number, line in enumerate(result.stdout.splitlines(), 1):
        fields = line.split("\t")
        if len(fields) != 3:
            raise ValueError(f"unexpected bcftools output at {path}:{line_number}: {line!r}")
        keys.append((fields[0], int(fields[1]), int(fields[2])))
    return keys


def run(tool: str, args: argparse.Namespace, output: Path, timing: Path) -> subprocess.CompletedProcess[str]:
    if tool == "bedder":
        command = [
            str(args.bedder), "intersect", "-a", str(args.vcf), "-b", str(args.target),
            "-g", str(args.genome), "-o", str(output),
        ]
    elif tool == "bedtools":
        command = [
            "bedtools", "intersect", "-sorted", "-header", "-a", str(args.vcf),
            "-b", str(args.target), "-g", str(args.genome),
        ]
    else:
        raise ValueError(tool)
    # Both tools materialize VCF output during the timed command. Do not buffer
    # BEDTools output in Python and write it after timing has stopped.
    with output.open("w") if tool == "bedtools" else open("/dev/null", "w") as handle:
        result = subprocess.run(
            ["/usr/bin/time", "-f", "%e\\t%U\\t%S\\t%M", "-o", str(timing), *command],
            stdout=handle,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    output.with_suffix(".stderr").write_text(result.stderr)
    return result


def parse_timing(path: Path) -> dict[str, float | int]:
    fields = path.read_text().strip().split("\t")
    if len(fields) != 4:
        raise ValueError(f"unexpected GNU time output: {fields!r}")
    return {
        "wall_seconds": float(fields[0]),
        "user_seconds": float(fields[1]),
        "system_seconds": float(fields[2]),
        "max_rss_kib": int(fields[3]),
    }


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def main() -> int:
    args = parse_args()
    if args.runs < 1 or args.warmups < 0:
        raise SystemExit("--runs must be positive and --warmups must be non-negative")
    root = args.output_root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    expected = expected_keys(args.expected)
    tools = ("bedder", "bedtools")
    bedder_path = shutil.which(str(args.bedder))
    if bedder_path is None:
        raise SystemExit(f"bedder binary is not executable: {args.bedder}")
    args.bedder = Path(bedder_path).resolve()
    manifest = {
        "vcf": {"path": str(args.vcf.resolve()), "sha256": sha256_file(args.vcf)},
        "target": {"path": str(args.target.resolve()), "sha256": sha256_file(args.target)},
        "genome": {"path": str(args.genome.resolve()), "sha256": sha256_file(args.genome)},
        "expected": {"path": str(args.expected.resolve()), "sha256": sha256_file(args.expected)},
        "runs": args.runs,
        "warmups": args.warmups,
        "bedder": {"path": str(args.bedder.resolve()), "sha256": sha256_file(args.bedder)},
    }
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    outputs: dict[str, list[tuple[str, int, int]]] = {}
    validation: dict[str, dict[str, object]] = {}
    for tool in tools:
        output = root / f"{tool}.validation.vcf"
        timing = root / f"{tool}.validation.time"
        result = run(tool, args, output, timing)
        keys = vcf_keys(output) if result.returncode == 0 else []
        outputs[tool] = keys
        unique = set(keys)
        passed = result.returncode == 0 and unique == expected
        validation[tool] = {
            "passed": passed,
            "returncode": result.returncode,
            "records": len(keys),
            "unique_query_intervals": len(unique),
            "expected_unique_query_intervals": len(expected),
            "stderr": result.stderr,
        }
        if not passed:
            raise RuntimeError(f"native VCF validation failed for {tool}: {validation[tool]}")
    if outputs["bedder"] != outputs["bedtools"]:
        raise RuntimeError("bedder and BEDTools native VCF outputs differ")
    (root / "validation.json").write_text(json.dumps(validation, indent=2) + "\n")

    rows: list[dict[str, object]] = []
    for warmup in range(args.warmups):
        for tool in tools:
            output = root / f"{tool}.warmup-{warmup + 1}.vcf"
            timing = root / f"{tool}.warmup-{warmup + 1}.time"
            result = run(tool, args, output, timing)
            if result.returncode or set(vcf_keys(output)) != expected:
                raise RuntimeError(f"native VCF warm-up failed for {tool}")

    for run_number in range(args.runs):
        order = tools[run_number % 2 :] + tools[: run_number % 2]
        for order_number, tool in enumerate(order, 1):
            output = root / f"{tool}.run-{run_number + 1}.vcf"
            timing = root / f"{tool}.run-{run_number + 1}.time"
            result = run(tool, args, output, timing)
            keys = vcf_keys(output) if result.returncode == 0 else []
            passed = result.returncode == 0 and keys == outputs[tool] and set(keys) == expected
            if not passed:
                raise RuntimeError(f"native VCF measured output failed for {tool}")
            rows.append({"tool": tool, "run": run_number + 1, "order": order_number, "correct": True, **parse_timing(timing)})

    with (root / "measurements.jsonl").open("w") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    with (root / "summary.tsv").open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["tool", "runs", "median_wall_seconds", "q1_wall_seconds", "q3_wall_seconds", "median_max_rss_kib", "all_correct"])
        for tool in tools:
            values = [row for row in rows if row["tool"] == tool]
            walls = [float(row["wall_seconds"]) for row in values]
            rss = [float(row["max_rss_kib"]) for row in values]
            writer.writerow([tool, len(values), f"{statistics.median(walls):.6f}", f"{percentile(walls, .25):.6f}", f"{percentile(walls, .75):.6f}", f"{statistics.median(rss):.0f}", True])
    print((root / "summary.tsv").read_text(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
