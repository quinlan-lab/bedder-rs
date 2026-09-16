"""Run the SD annotation and validate every result against original UCSC rows."""
import argparse
from bisect import bisect_right
from collections import Counter
import csv
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

HERE = Path(__file__).resolve().parent
FIELDS = ['sd_best', 'sd_best_interchrom', 'sd_count', 'sd_count_interchrom']


def sha(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def records(path):
    with open(path) as stream:
        for line in stream:
            if not line.startswith('#'):
                yield line.rstrip().split('\t')


def record_key(fields):
    return tuple(fields[:7] + [tuple(sorted(fields[7].split(';')))] + fields[8:])


def oracle_best(hits):
    if not hits:
        return '.', 0
    maximum = max(Fraction(row['fracMatch']) for row in hits)
    tied = [row for row in hits if Fraction(row['fracMatch']) == maximum]
    row = sorted(tied, key=lambda r: r['row_id'])[0]
    return (f'{row["fracMatch"]}|{row["chrom"]}:{row["chromStart"]}-{row["chromEnd"]}|'
            f'{row["otherChrom"]}:{row["otherStart"]}-{row["otherEnd"]}|{row["strand"]}|{row["row_id"]}'), len(tied)


def validate(data, output):
    with (data / 'source-rows.tsv').open() as stream:
        sources = list(csv.DictReader(stream, delimiter='\t'))
    beds = list(records(data / 'segdups.bed'))
    if len(beds) != len(sources):
        raise ValueError('source/projection row count mismatch')
    for bed, row in zip(beds, sources):
        projection = [row['chrom'], row['chromStart'], row['chromEnd'], bed[3], '0', row['strand'],
                      'fracMatch=' + row['fracMatch'], row['otherChrom'], row['otherStart'], row['otherEnd'], row['uid']]
        if projection != bed:
            raise ValueError('BED projection differs from original UCSC fields')
        row['row_id'] = bed[3]
    starts = [int(row['chromStart']) for row in sources]
    max_length = max(int(r['chromEnd'])-int(r['chromStart']) for r in sources)
    expected = Counter()
    table = []
    summary = Counter()
    for f in records(data / 'variants.vcf'):
        pos = int(f[1]) - 1
        candidates = sources[bisect_right(starts, pos-max_length):bisect_right(starts, pos)]
        hits = [r for r in candidates if r['chrom'] == f[0] and int(r['chromStart']) <= pos < int(r['chromEnd'])]
        inter = [r for r in hits if r['chrom'] != r['otherChrom']]
        best, ties = oracle_best(hits)
        best_inter, inter_ties = oracle_best(inter)
        values = (best, best_inter, str(len(hits)), str(len(inter)))
        expected[(record_key(f), values)] += 1
        table.append(f[:2] + f[3:5] + list(values))
        summary['variants'] += 1
        summary['with_sd'] += bool(hits)
        summary['with_interchrom_sd'] += bool(inter)
        summary['with_intra_only'] += bool(hits) and not inter
        summary['different_best_when_interchrom_only'] += bool(inter) and best != best_inter
        summary['tied_for_best'] += ties > 1
        summary['tied_for_best_interchrom'] += inter_ties > 1
    actual = Counter()
    for f in records(output / 'annotated.vcf'):
        info = dict(v.split('=', 1) if '=' in v else (v, '') for v in f[7].split(';'))
        values = tuple(info.pop(name) for name in FIELDS)
        f[7] = ';'.join(k + ('=' + v if v else '') for k, v in info.items()) or '.'
        actual[(record_key(f), values)] += 1
    if not expected or actual != expected:
        raise ValueError(f'validation failed: missing={sum((expected-actual).values())}, '
                         f'extra={sum((actual-expected).values())}')
    with (output / 'best-matches.tsv').open('w') as stream:
        writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
        writer.writerow(['chrom', 'pos_1based', 'ref', 'alt'] + FIELDS)
        writer.writerows(table)
    summary = dict(summary, validation='all VCF records and four annotations match original-row oracle')
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bedder', type=Path, default=HERE.parents[1] / 'target/release/bedder')
    parser.add_argument('--data', type=Path, default=HERE / 'data')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data, output, binary = args.data.resolve(), args.output.resolve(), args.bedder.resolve()
    lock = json.loads((data / 'inputs.lock.json').read_text())
    for name, digest in lock['files'].items():
        if sha(data / name) != digest:
            raise ValueError(f'input checksum mismatch: {name}')
    output.mkdir(parents=True, exist_ok=False)
    command = [str(binary), 'intersect', '-a', str(data / 'variants.vcf'),
               '-b', str(data / 'segdups.bed'), '-g', str(data / 'genome.tsv'),
               '--a-piece', 'whole-wide', '--b-piece', 'whole-wide', '-r', '0', '-R', '0',
               '--python', str(HERE / 'segdup_best_match.py'), '-o', str(output / 'annotated.vcf')]
    for name in FIELDS:
        command.extend(['-c', 'py:' + name])
    env = dict(os.environ, PYTHONHASHSEED='0', LC_ALL='C')
    subprocess.run(command, check=True, env=env)
    header = (output / 'annotated.vcf').read_text().split('#CHROM', 1)[0]
    for name in FIELDS:
        kind = 'Integer' if name.startswith('sd_count') else 'String'
        if f'##INFO=<ID={name},Number=1,Type={kind},' not in header:
            raise ValueError('invalid INFO declaration: ' + name)
    summary = validate(data, output)
    probe = output / 'python-version.py'
    probe.write_text('import sys\nprint(sys.version, flush=True)\n')
    embedded = subprocess.check_output([str(binary), 'intersect', '-a', str(data / 'variants.vcf'),
        '-b', str(data / 'segdups.bed'), '-g', str(data / 'genome.tsv'), '--python', str(probe),
        '-o', os.devnull], text=True, env=env).strip()
    probe.unlink()
    repo = HERE.parents[1]
    provenance = {'command': command, 'inputs': lock, 'binary_sha256': sha(binary),
                  'bedder_version': subprocess.check_output([str(binary), '--version'], text=True).strip(),
                  'driver_python': sys.version, 'embedded_python': embedded, 'platform': platform.platform(),
                  'git_revision': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip(),
                  'scripts': {p.name: sha(p) for p in HERE.glob('*.py')},
                  'outputs': {p.name: sha(p) for p in output.iterdir() if p.is_file()}}
    (output / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
