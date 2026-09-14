#!/usr/bin/env python3
"""Restore BED4 rows to AIList's coordinate-only, query-ordered output."""

from __future__ import annotations

import argparse
from pathlib import Path


def parse_bed(line: str) -> tuple[str, int, int]:
    fields = line.rstrip("\n").split("\t")
    return fields[0], int(fields[1]), int(fields[2])


def result_records(path: Path):
    with path.open() as handle:
        for line in handle:
            fields = line.split()
            if len(fields) == 5 and fields[0].isdigit() and fields[1].endswith(":"):
                yield fields[1][:-1], int(fields[2]), int(fields[3])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("query", type=Path)
    parser.add_argument("ailist_output", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    with args.query.open() as query_handle, args.output.open("w") as output_handle:
        query_line = next(query_handle, None)
        for result_coord in result_records(args.ailist_output):
            while query_line is not None and parse_bed(query_line) != result_coord:
                query_line = next(query_handle, None)
            if query_line is None:
                raise ValueError(f"AIList coordinate is absent or out of order: {result_coord}")
            output_handle.write(query_line)
            query_line = next(query_handle, None)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
