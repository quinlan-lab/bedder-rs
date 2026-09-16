"""Freeze HG002 chr19 PASS SNVs overlapping primary-chromosome UCSC SDs."""
import argparse
from bisect import bisect_right
import gzip
import hashlib
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent


def sha(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['vcf', 'segdups', 'schema', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--freeze', action='store_true', help='create a new snapshot; default verifies existing lock')
    args = parser.parse_args()
    sources = {name: {'name': getattr(args, name).name, 'sha256': sha(getattr(args, name))}
               for name in ['vcf', 'segdups', 'schema']}
    expected = None if args.freeze else json.loads((HERE / 'data/inputs.lock.json').read_text())
    if expected and sources != expected['sources']:
        raise ValueError('original sources differ from frozen lock; use bundled offline data')
    columns = re.findall(r'^  `([^`]+)`', args.schema.read_text(), re.M)
    primary = {f'chr{i}' for i in range(1, 23)} | {'chrX', 'chrY'}
    rows = []
    counts = {'chr19_sd_rows': 0, 'primary_partner_sd_rows': 0,
              'chr19_variant_records': 0, 'eligible_snvs': 0, 'retained_snvs': 0}
    with gzip.open(args.segdups, 'rt') as stream:
        for line_number, line in enumerate(stream, 1):
            fields = line.rstrip().split('\t')
            if len(fields) != len(columns):
                raise ValueError('SD table does not match frozen SQL schema')
            row = dict(zip(columns, fields))
            if row['chrom'] != 'chr19':
                continue
            counts['chr19_sd_rows'] += 1
            if row['otherChrom'] not in primary:
                continue
            counts['primary_partner_sd_rows'] += 1
            start, end = int(row['chromStart']), int(row['chromEnd'])
            if not 0 <= start < end or not 0 <= int(row['otherStart']) < int(row['otherEnd']):
                raise ValueError('invalid SD interval')
            name = f'sd_{row["uid"]}_{line_number}'
            bed = ['chr19', str(start), str(end), name, '0', row['strand'], 'fracMatch=' + row['fracMatch'],
                   row['otherChrom'], row['otherStart'], row['otherEnd'], row['uid']]
            rows.append((start, end, name, bed, line))
    rows.sort()
    starts = [row[0] for row in rows]
    max_length = max(row[1] - row[0] for row in rows)
    args.output.mkdir(parents=True, exist_ok=False)
    used = set()
    with gzip.open(args.vcf, 'rt') as source, (args.output / 'variants.vcf').open('w') as dest:
        for line in source:
            if line.startswith('#'):
                dest.write(line)
                if line.startswith('#CHROM') and line.rstrip().split('\t')[9:] != ['HG002']:
                    raise ValueError('expected single-sample HG002 VCF')
                continue
            f = line.rstrip().split('\t')
            if f[0] != 'chr19':
                continue
            counts['chr19_variant_records'] += 1
            if f[6] != 'PASS' or len(f[3]) != 1 or len(f[4]) != 1 or set(f[3] + f[4]) - set('ACGT'):
                continue
            gt = dict(zip(f[8].split(':'), f[9].split(':'))).get('GT', '.')
            if '1' not in gt.replace('|', '/').split('/'):
                continue
            counts['eligible_snvs'] += 1
            pos = int(f[1]) - 1
            hits = [r for r in rows[bisect_right(starts, pos-max_length):bisect_right(starts, pos)]
                    if r[0] <= pos < r[1]]
            if hits:
                dest.write(line)
                counts['retained_snvs'] += 1
                used.update(r[2] for r in hits)
    with (args.output / 'segdups.bed').open('w') as bed, (args.output / 'source-rows.tsv').open('w') as raw:
        raw.write('\t'.join(columns) + '\n')
        for _, _, name, fields, original in rows:
            if name in used:
                bed.write('\t'.join(fields) + '\n')
                raw.write(original)
    (args.output / 'genome.tsv').write_text('chr19\t58617616\n')
    (args.output / 'genomicSuperDups.sql').write_text(args.schema.read_text())
    counts['retained_sd_rows'] = len(used)
    files = {p.name: sha(p) for p in sorted(args.output.iterdir())}
    lock = {'sources': sources, 'counts': counts, 'files': files}
    (args.output / 'inputs.lock.json').write_text(json.dumps(lock, indent=2) + '\n')
    if expected and lock != expected:
        raise ValueError('rebuilt snapshot differs from frozen inputs')
    print(json.dumps(counts, indent=2))


if __name__ == '__main__':
    main()
