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


def _to_float_or_none(value):
    if value is None:
        return None
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, (bytes, bytearray)):
        try:
            return float(value.decode())
        except ValueError:
            return None
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return None
    return None


def bedder_bed_score(iv) -> float:
    """Extract BED score from a mapped interval."""
    b = iv.bed()
    if b is None:
        return None
    if b.score is None:
        return None
    return float(b.score)


def bedder_vcf_dp(iv) -> float:
    """Extract INFO/DP from a mapped VCF interval."""
    v = iv.vcf()
    if v is None:
        return None
    return _to_float_or_none(v.info("DP"))


def bedder_vcf_af_first(iv) -> float:
    """Extract the first INFO/AF entry from a mapped VCF interval."""
    v = iv.vcf()
    if v is None:
        return None
    af = v.info("AF")
    if af is None:
        return None
    if isinstance(af, list):
        if len(af) == 0:
            return None
        return _to_float_or_none(af[0])
    return _to_float_or_none(af)


def bedder_bad_numeric(iv) -> float:
    """Return a non-numeric value to test extractor runtime validation."""
    _ = iv.chrom
    return "not-a-number"
