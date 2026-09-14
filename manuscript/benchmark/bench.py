#!/usr/bin/env python3
"""Correctness-gated, modular interval-tool benchmark runner."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


BENCHMARK_DIR = Path(__file__).resolve().parent
DEFAULT_TOOLS = ("bedder", "bedtools", "bedtk", "bedops", "ailist", "coitrees")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", default="smoke", help="case_id from cases.tsv")
    parser.add_argument("--tools", default=",".join(DEFAULT_TOOLS))
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--output-root", type=Path, default=Path("results/smoke"))
    parser.add_argument("--list-cases", action="store_true")
    return parser.parse_args()


def load_cases() -> dict[str, dict[str, str]]:
    with (BENCHMARK_DIR / "cases.tsv").open(newline="") as handle:
        return {row["case_id"]: row for row in csv.DictReader(handle, delimiter="\t")}


def resolve_case(row: dict[str, str]) -> dict[str, Path | str]:
    resolved: dict[str, Path | str] = dict(row)
    for key in ("query", "target", "genome", "expected"):
        resolved[key] = (BENCHMARK_DIR / row[key]).resolve()
    return resolved


def canonical_records(path: Path) -> list[tuple[str, int, int, str]]:
    records: list[tuple[str, int, int, str]] = []
    with path.open() as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 4:
                raise ValueError(f"{path}:{line_number}: expected BED4, got {line.rstrip()!r}")
            records.append((fields[0], int(fields[1]), int(fields[2]), fields[3]))
    # Preserve duplicate query records: membership means once per input row,
    # not once per unique coordinate/name tuple.
    return sorted(records, key=lambda r: (r[0], r[1], r[2], r[3]))


def write_canonical(path: Path, records: list[tuple[str, int, int, str]]) -> None:
    with path.open("w") as handle:
        for chrom, start, end, name in records:
            handle.write(f"{chrom}\t{start}\t{end}\t{name}\n")


def prepare_case(case: dict[str, Path | str], prep_dir: Path) -> dict[str, float | int]:
    prep_dir.mkdir(parents=True, exist_ok=True)
    metrics: dict[str, float | int] = {}
    for label in ("query", "target"):
        destination = prep_dir / f"{label}.bedops.bed"
        started = time.monotonic()
        with destination.open("w") as output_handle:
            result = subprocess.run(
                ["sort-bed", str(case[label])],
                stdout=output_handle,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
            )
        metrics[f"bedops_{label}_sort_seconds"] = time.monotonic() - started
        if result.returncode != 0:
            raise RuntimeError(f"sort-bed failed for {label}: {result.stderr}")
    return metrics


def run_adapter(
    tool: str,
    case: dict[str, Path | str],
    output: Path,
    prep_dir: Path,
    timing_file: Path | None,
) -> subprocess.CompletedProcess[str]:
    adapter = BENCHMARK_DIR / "adapters" / f"{tool}.sh"
    if not adapter.exists():
        raise FileNotFoundError(f"missing adapter for {tool}: {adapter}")
    command = [
        str(adapter),
        str(case["query"]),
        str(case["target"]),
        str(case["genome"]),
        str(output),
    ]
    if timing_file is not None:
        command = [
            "/usr/bin/time",
            "-f",
            "%e\t%U\t%S\t%M",
            "-o",
            str(timing_file),
            *command,
        ]
    env = os.environ.copy()
    env["BENCH_PREP_DIR"] = str(prep_dir)
    return subprocess.run(command, env=env, capture_output=True, text=True, check=False)


def parse_timing(path: Path) -> dict[str, float | int]:
    fields = path.read_text().strip().split("\t")
    if len(fields) != 4:
        raise ValueError(f"unexpected GNU time output in {path}: {fields!r}")
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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def record_inputs(case: dict[str, Path | str], output_root: Path) -> None:
    with (output_root / "inputs.tsv").open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["role", "path", "bytes", "sha256"])
        for role in ("query", "target", "genome", "expected"):
            path = Path(case[role])
            writer.writerow([role, path, path.stat().st_size, sha256_file(path)])


def main() -> int:
    args = parse_args()
    cases = load_cases()
    if args.list_cases:
        for case_id, row in cases.items():
            print(f"{case_id}\t{row['description']}")
        return 0
    if args.case not in cases:
        raise SystemExit(f"unknown case {args.case!r}; choose from {', '.join(cases)}")
    if args.runs < 1 or args.warmups < 0:
        raise SystemExit("--runs must be positive and --warmups must be non-negative")

    tools = tuple(filter(None, (item.strip() for item in args.tools.split(","))))
    unknown = sorted(set(tools) - set(DEFAULT_TOOLS))
    if unknown:
        raise SystemExit(f"unknown tools: {', '.join(unknown)}")

    case = resolve_case(cases[args.case])
    output_root = args.output_root.resolve()
    raw_dir = output_root / "raw"
    output_dir = output_root / "outputs"
    prep_dir = output_root / "prepared" / args.case
    for directory in (raw_dir, output_dir, prep_dir):
        directory.mkdir(parents=True, exist_ok=True)

    expected = canonical_records(case["expected"])
    record_inputs(case, output_root)
    prep_metrics = prepare_case(case, prep_dir)
    (output_root / "preparation.json").write_text(json.dumps(prep_metrics, indent=2) + "\n")

    version_script = BENCHMARK_DIR / "scripts" / "record-versions.sh"
    subprocess.run([str(version_script), str(output_root / "versions.tsv")], check=True)

    validation: dict[str, dict[str, object]] = {}
    for tool in tools:
        raw_output = output_dir / f"{tool}.validation.raw.bed"
        result = run_adapter(tool, case, raw_output, prep_dir, None)
        observed = canonical_records(raw_output) if result.returncode == 0 else []
        passed = result.returncode == 0 and observed == expected
        canonical_path = output_dir / f"{tool}.canonical.bed"
        write_canonical(canonical_path, observed)
        validation[tool] = {
            "passed": passed,
            "returncode": result.returncode,
            "stderr": result.stderr,
            "expected_records": len(expected),
            "observed_records": len(observed),
        }
        if not passed:
            print(f"validation failed for {tool}: {validation[tool]}", file=sys.stderr)
    (output_root / "validation.json").write_text(json.dumps(validation, indent=2) + "\n")
    failed = [tool for tool in tools if not validation[tool]["passed"]]
    if failed:
        print(f"correctness gate failed: {', '.join(failed)}", file=sys.stderr)
        return 1

    for warmup in range(args.warmups):
        for tool in tools:
            warmup_output = output_dir / f"{tool}.warmup-{warmup + 1}.bed"
            result = run_adapter(tool, case, warmup_output, prep_dir, None)
            if result.returncode != 0 or canonical_records(warmup_output) != expected:
                raise RuntimeError(f"warm-up failed for {tool}: {result.stderr}")

    records: list[dict[str, object]] = []
    jsonl_path = raw_dir / "measurements.jsonl"
    with jsonl_path.open("w") as jsonl:
        for run_index in range(args.runs):
            offset = run_index % len(tools)
            run_order = tools[offset:] + tools[:offset]
            for order_index, tool in enumerate(run_order):
                raw_output = output_dir / f"{tool}.run-{run_index + 1}.bed"
                timing_file = raw_dir / f"{tool}.run-{run_index + 1}.time.tsv"
                result = run_adapter(tool, case, raw_output, prep_dir, timing_file)
                observed = canonical_records(raw_output) if result.returncode == 0 else []
                passed = result.returncode == 0 and observed == expected
                timing = parse_timing(timing_file)
                record: dict[str, object] = {
                    "case": args.case,
                    "tool": tool,
                    "run": run_index + 1,
                    "order": order_index + 1,
                    "correct": passed,
                    "returncode": result.returncode,
                    **timing,
                }
                records.append(record)
                jsonl.write(json.dumps(record, sort_keys=True) + "\n")
                jsonl.flush()
                if not passed:
                    raise RuntimeError(f"measured output failed correctness for {tool}: {result.stderr}")

    summary_path = output_root / "summary.tsv"
    with summary_path.open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(
            [
                "case",
                "tool",
                "runs",
                "median_wall_seconds",
                "q1_wall_seconds",
                "q3_wall_seconds",
                "median_max_rss_kib",
                "all_correct",
            ]
        )
        for tool in tools:
            tool_records = [record for record in records if record["tool"] == tool]
            walls = [float(record["wall_seconds"]) for record in tool_records]
            rss = [float(record["max_rss_kib"]) for record in tool_records]
            writer.writerow(
                [
                    args.case,
                    tool,
                    len(tool_records),
                    f"{statistics.median(walls):.6f}",
                    f"{percentile(walls, 0.25):.6f}",
                    f"{percentile(walls, 0.75):.6f}",
                    f"{statistics.median(rss):.0f}",
                    all(bool(record["correct"]) for record in tool_records),
                ]
            )

    print(summary_path.read_text(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
