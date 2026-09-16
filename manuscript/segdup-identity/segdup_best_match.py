"""Select the highest-fracMatch SD record, retaining its coordinates and partner.

Use --a-piece whole-wide --b-piece whole-wide to supply all overlaps together.
B is BED6 plus fracMatch=<value>, otherChrom, otherStart, otherEnd, and UCSC uid.
The label preserves precision through bedder's numeric-field formatting.
"""
from decimal import Decimal


def best_match(fragment, interchromosomal=False):
    candidates = []
    for overlap in fragment.b:
        b = overlap.bed()
        strand, identity, partner, start, end, uid = b.other_fields()
        identity = identity.split('=', 1)[1]
        score = Decimal(identity)
        if not score.is_finite() or not 0 <= score <= 1:
            raise ValueError('fracMatch must be a finite fraction between 0 and 1')
        if interchromosomal and b.chrom == partner:
            continue
        value = f'{identity}|{b.chrom}:{b.start}-{b.stop}|{partner}:{start}-{end}|{strand}|{b.name}'
        candidates.append((-score, b.name, value))  # Highest identity; ties by row ID.
    return min(candidates)[2] if candidates else '.'


def bedder_sd_best(fragment) -> str:
    """Maximum-fracMatch SD: identity|local interval|partner interval|strand|row ID; BED coordinates."""
    return best_match(fragment)


def bedder_sd_best_interchrom(fragment) -> str:
    """Maximum-fracMatch interchromosomal SD: identity|local interval|partner interval|strand|row ID."""
    return best_match(fragment, interchromosomal=True)


def bedder_sd_count(fragment) -> int:
    """Number of overlapping SD alignment rows; not a copy-number estimate."""
    return len(fragment.b)


def bedder_sd_count_interchrom(fragment) -> int:
    """Number of overlapping SD alignment rows whose partner is on another chromosome."""
    return sum(b.bed().chrom != b.bed().other_fields()[2] for b in fragment.b)
