#!/usr/bin/env python3
"""Independent half-open BED membership oracle using a sweep-line heap."""

from __future__ import annotations

import argparse
import bisect
from collections import defaultdict
from pathlib import Path
import sys


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", required=True, type=Path)
    parser.add_argument("--target", required=True, type=Path)
    parser.add_argument("--genome", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def genome_order(path: Path) -> dict[str, int]:
    order: dict[str, int] = {}
    with path.open() as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            chrom = line.split("\t", 1)[0]
            if chrom in order:
                raise ValueError(f"{path}:{line_number}: duplicate sequence {chrom}")
            order[chrom] = len(order)
    return order


def fields(path: Path, line_number: int, line: str) -> tuple[str, int, int]:
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 3:
        raise ValueError(f"{path}:{line_number}: expected at least BED3")
    chrom, start, end = parts[0], int(parts[1]), int(parts[2])
    if start < 0 or end <= start:
        raise ValueError(f"{path}:{line_number}: invalid half-open interval")
    return chrom, start, end


def load_targets(path: Path, order: dict[str, int]) -> dict[str, tuple[list[int], list[int]]]:
    targets: dict[str, list[tuple[int, int]]] = defaultdict(list)
    previous = (-1, -1)
    with path.open() as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip() or line.startswith("#"):
                continue
            chrom, start, end = fields(path, line_number, line)
            if chrom not in order:
                raise ValueError(f"{path}:{line_number}: {chrom} is absent from genome file")
            key = (order[chrom], start)
            if key < previous:
                raise ValueError(f"{path}:{line_number}: target is not genome-sorted")
            previous = key
            targets[chrom].append((start, end))
    index: dict[str, tuple[list[int], list[int]]] = {}
    for chrom, intervals in targets.items():
        starts: list[int] = []
        prefix_max_end: list[int] = []
        maximum_end = -1
        for start, end in intervals:
            starts.append(start)
            maximum_end = max(maximum_end, end)
            prefix_max_end.append(maximum_end)
        index[chrom] = starts, prefix_max_end
    return index


def main() -> int:
    args = parse_args()
    order = genome_order(args.genome)
    targets = load_targets(args.target, order)
    previous = (-1, -1)
    query_count = 0
    match_count = 0

    with args.query.open() as query_handle, args.output.open("w") as output_handle:
        for line_number, line in enumerate(query_handle, 1):
            if not line.strip() or line.startswith("#"):
                continue
            chrom, start, end = fields(args.query, line_number, line)
            if chrom not in order:
                raise ValueError(f"{args.query}:{line_number}: {chrom} is absent from genome file")
            key = (order[chrom], start)
            if key < previous:
                raise ValueError(f"{args.query}:{line_number}: query is not genome-sorted")
            previous = key
            query_count += 1

            starts, prefix_max_end = targets.get(chrom, ([], []))
            last_possible = bisect.bisect_left(starts, end) - 1
            if last_possible >= 0 and prefix_max_end[last_possible] > start:
                output_handle.write(line)
                match_count += 1

    print(f"oracle: {match_count} of {query_count} query records overlap", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
