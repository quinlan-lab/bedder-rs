def bedder_sum_plus_one(values) -> float:
    """Sum the mapped values and add one to make expected outputs obvious."""
    return float(sum(values) + 1.0)


def bedder_empty_marker(values) -> str:
    """Return a marker for empty overlaps so no-overlap behavior is explicit."""
    if len(values) == 0:
        return "EMPTY"
    return str(len(values))


def bedder_has_values(values) -> bool:
    """Expose bool conversion contract in map output."""
    return len(values) > 0
