#def bedder(fragment):
#    if fragment.a.start % 2 != 0: return "odd"
#    return "even"

def bedder_odd(fragment) -> str:
    """return odd if the start of the query interval is odd. otherwise even"""
    if fragment.a.start % 2 != 0: return "odd"
    return "even"

def bedder_n_overlapping(fragment) -> int:
    return len(fragment.b)


def bedder_total_b_overlap(fragment) -> int:
    """total bases of overlap in b"""
    return sum(b.stop - b.start for b in fragment.b)
