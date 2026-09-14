def bedder_repeat_sequence(fragment) -> str:
    """repeat sequence from UCSC simple repeats track"""
    result = []
    for bf in fragment.b:
        b = bf.bed()
        result.append(b.other_fields()[0])
    return "|".join(result)
