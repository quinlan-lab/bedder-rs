"""Effect of one SNV on the longest pure tandem-repeat run.

Inputs are a biallelic VCF and BED4 plus forward-strand motif and reference
sequence. Use --a-piece whole --b-piece whole: each output VCF record carries
one variant/repeat result. Other variants in the repeat are not applied.
"""
import re
from functools import lru_cache


@lru_cache(maxsize=128)
def rotations(motif):
    """Equivalent forward-strand phases; do not add reverse complements."""
    if not motif or set(motif) - set('ACGT'):
        raise ValueError('requires a nonempty uppercase A/C/G/T motif')
    return tuple(sorted({motif[i:] + motif[:i] for i in range(len(motif))}))


@lru_cache(maxsize=8192)
def longest_run(sequence, motif):
    """Maximum number of complete, consecutive motif copies, over all phases."""
    # Lookahead retains overlapping starts: a shorter earlier match must not
    # hide a longer run beginning within it (e.g. the AAAT HG002 regression).
    return max((len(m.group(1)) // len(motif)
                for phase in rotations(motif)
                for m in re.finditer('(?=((?:' + phase + ')+))', sequence)), default=0)


def effect(sequence, motif, offset, ref, alt):
    if not (len(ref) == len(alt) == 1 and ref in 'ACGT' and alt in 'ACGT' and ref != alt):
        raise ValueError('requires a biallelic A/C/G/T SNV')
    if set(sequence) - set('ACGT'):
        raise ValueError('requires an uppercase A/C/G/T reference sequence')
    if not 0 <= offset < len(sequence) or sequence[offset] != ref:
        raise ValueError('VCF REF does not match reference repeat sequence')
    alternate = sequence[:offset] + alt + sequence[offset + 1:]
    before = longest_run(sequence, motif)
    after = longest_run(alternate, motif)
    return before, after, after - before


def bedder_repeat_run(fragment) -> str:
    """Repeat ID|motif|reference pure copies|alternate pure copies|delta; single-SNV effect."""
    variant = fragment.a.vcf()
    if variant is None:
        raise ValueError('requires VCF query records')
    if len(variant.ALT) != 1:
        raise ValueError('requires biallelic input')
    if len(fragment.b) != 1:
        raise ValueError('requires one repeat per record: use --a-piece whole --b-piece whole')
    repeat = fragment.b[0].bed()
    if repeat is None:
        raise ValueError('requires BED repeat records')
    motif, sequence = repeat.other_fields()
    if len(sequence) != repeat.stop - repeat.start:
        raise ValueError('repeat sequence length does not match BED interval')
    before, after, delta = effect(sequence, motif, variant.pos - repeat.start,
                                  variant.REF, variant.ALT[0])
    return f'{repeat.name}|{motif}|{before}|{after}|{delta}'
