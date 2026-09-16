"""Freeze the chr19 HG002 example from original files, preserving VCF records."""
import argparse
from bisect import bisect_right
import gzip
import hashlib
import json
from pathlib import Path


def sha(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def prepare(vcf, repeats, reference, output, source_lock):
    expected = json.loads(source_lock.read_text())
    for role, path in [('vcf', vcf), ('repeats', repeats), ('reference', reference)]:
        if sha(path) != expected['sources'][role]['sha256']:
            raise ValueError(f'{role} differs from frozen source SHA-256; use the bundled subset instead')
    output.mkdir(parents=True, exist_ok=False)
    with gzip.open(reference, 'rt') as stream:
        if next(stream).strip().split()[0] != '>chr19':
            raise ValueError('expected chr19 FASTA')
        sequence = ''.join(line.strip() for line in stream).upper()
    rows = []
    with gzip.open(repeats, 'rt') as stream:
        for number, line in enumerate(stream, 1):
            chrom, start, end, name, motif = line.rstrip().split('\t')
            if chrom != 'chr19' or not 2 <= len(motif) <= 6 or set(motif) - set('ACGT'):
                continue
            # Exclude reducible motifs so copy counts have an unambiguous unit.
            if any(len(motif) % k == 0 and motif == motif[:k] * (len(motif) // k)
                   for k in range(1, len(motif))):
                continue
            start, end = int(start), int(end)
            seq = sequence[start:end]
            if len(seq) != end - start or set(seq) - set('ACGT'):
                continue
            rows.append((start, end, f'trf_{number}', motif, seq))
    rows.sort()
    starts = [row[0] for row in rows]
    max_length = max(end - start for start, end, *_ in rows)
    used = set()
    counts = {'chr19_records': 0, 'eligible_snvs': 0, 'overlapping_snvs': 0,
              'eligible_repeat_intervals': len(rows)}
    with gzip.open(vcf, 'rt') as source, (output / 'variants.vcf').open('w') as dest:
        for line in source:
            if line.startswith('#'):
                dest.write(line)
                continue
            fields = line.rstrip().split('\t')
            if fields[0] != 'chr19':
                continue
            counts['chr19_records'] += 1
            ref, alt = fields[3:5]
            if fields[6] != 'PASS' or len(ref) != 1 or len(alt) != 1 or set(ref + alt) - set('ACGT'):
                continue
            gt = dict(zip(fields[8].split(':'), fields[9].split(':'))).get('GT', '.')
            if '1' not in gt.replace('|', '/').split('/'):
                continue
            counts['eligible_snvs'] += 1
            pos = int(fields[1]) - 1
            if sequence[pos] != ref:
                raise ValueError(f'REF mismatch at chr19:{pos + 1}')
            hits = [row for row in rows[bisect_right(starts, pos - max_length):bisect_right(starts, pos)]
                    if row[0] <= pos < row[1]]
            if hits:
                dest.write(line)
                used.update(row[2] for row in hits)
                counts['overlapping_snvs'] += 1
    with (output / 'repeats.bed').open('w') as stream:
        for start, end, name, motif, seq in rows:
            if name in used:
                stream.write(f'chr19\t{start}\t{end}\t{name}\t{motif}\t{seq}\n')
    (output / 'genome.tsv').write_text(f'chr19\t{len(sequence)}\n')
    counts['retained_repeat_intervals'] = len(used)
    lock = {'scope': 'chr19; PASS biallelic SNVs carrying ALT in HG002; primitive 2-6bp repeats',
            'counts': counts,
            'sources': {role: {'name': path.name, 'sha256': sha(path)}
                        for role, path in [('vcf', vcf), ('repeats', repeats), ('reference', reference)]},
            'files': {name: sha(output / name) for name in ['variants.vcf', 'repeats.bed', 'genome.tsv']}}
    (output / 'inputs.lock.json').write_text(json.dumps(lock, indent=2) + '\n')
    if lock['files'] != expected['files'] or counts != expected['counts']:
        raise ValueError('rebuilt subset differs from frozen input files or counts')
    print(json.dumps(counts, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['vcf', 'repeats', 'reference', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--source-lock', type=Path,
                        default=Path(__file__).resolve().parent / 'data/inputs.lock.json')
    args = parser.parse_args()
    prepare(args.vcf, args.repeats, args.reference, args.output, args.source_lock)
