# cargo run -- -a tests/a.bed -b tests/b.bed -g tests/hg38.small.fai -c 'start:string:start:1:py:tests/test_ab.py'
def bedder(intersection):
    a_mode = PyIntersectionMode.default() # report the full interval like -v in bedtools
    b_part = PyIntersectionPart.inverse() # report the part of the b-intervals that do not overlap the a-interval
    b_part = PyIntersectionPart.whole() # report the the whole b-interval 
    a_requirements = PyOverlapAmount.fraction(0.5) # require at least 50% of the a_interval to be covered
    b_requirements = PyOverlapAmount.bases(1)
    report = intersection.report(a_mode, None, None, b_part, a_requirements, b_requirements)
    result = []
    for ov in report:
        #if ov.a.stop - ov.a.start < 5_000: continue
        line = [f"{ov.a.chrom}\t{ov.a.start}\t{ov.a.stop}"]
        for b in ov.b:
            line.append(f"{b.start}\t{b.stop}")
        result.append("\t".join(line))
    return "\n".join(result)
