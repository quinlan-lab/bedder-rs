"""Run bedder and independently validate every variant/repeat annotation."""
import argparse
from collections import Counter
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

HERE = Path(__file__).resolve().parent


def sha(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def oracle_run(sequence, motif):
    """Independent exhaustive start-position scan, without regex or callback imports."""
    best = 0
    k = len(motif)
    phases = {motif[i:] + motif[:i] for i in range(k)}
    for start in range(len(sequence) - k + 1):
        unit = sequence[start:start + k]
        if unit not in phases:
            continue
        end = start
        while sequence[end:end + k] == unit:
            end += k
        best = max(best, (end - start) // k)
    return best


def records(path):
    with open(path) as stream:
        for line in stream:
            if not line.startswith('#'):
                yield line.rstrip().split('\t')


def record_key(fields):
    return tuple(fields[:7] + [tuple(sorted(fields[7].split(';')))] + fields[8:])


def validate(data, output):
    repeats = list(records(data / 'repeats.bed'))
    expected = Counter()
    table = []
    for fields in records(data / 'variants.vcf'):
        chrom, pos, _, ref, alt = fields[:5]
        pos0 = int(pos) - 1
        for bchrom, start, end, name, motif, seq in repeats:
            if bchrom != chrom or not int(start) <= pos0 < int(end):
                continue
            offset = pos0 - int(start)
            if seq[offset] != ref:
                raise ValueError('oracle REF mismatch')
            alternate = seq[:offset] + alt + seq[offset + 1:]
            before, after = oracle_run(seq, motif), oracle_run(alternate, motif)
            annotation = f'{name}|{motif}|{before}|{after}|{after - before}'
            expected[(record_key(fields), annotation)] += 1
            table.append([chrom, pos, ref, alt, name, start, end, motif, before, after,
                          after - before, seq, alternate])
    actual = Counter()
    for fields in records(output / 'annotated.vcf'):
        infos = fields[7].split(';')
        annotations = [value.split('=', 1)[1] for value in infos if value.startswith('repeat_run=')]
        if len(annotations) != 1:
            raise ValueError('missing or repeated repeat_run INFO')
        fields[7] = ';'.join(value for value in infos if not value.startswith('repeat_run=')) or '.'
        actual[(record_key(fields), annotations[0])] += 1
    if not expected or actual != expected:
        raise ValueError(f'validation failed: missing={sum((expected-actual).values())}, '
                         f'extra={sum((actual-expected).values())}')
    header = ['chrom', 'pos_1based', 'ref', 'alt', 'repeat_id', 'start_0based', 'end_exclusive',
              'motif', 'ref_copies', 'alt_copies', 'delta_copies', 'ref_sequence', 'alt_sequence']
    with (output / 'effects.tsv').open('w') as stream:
        writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
        writer.writerow(header)
        writer.writerows(table)
    summary = {'validated_pairs': sum(expected.values()),
               'unique_variants': len({key[0] for key in expected}),
               'run_increased': sum(row[10] > 0 for row in table),
               'run_decreased': sum(row[10] < 0 for row in table),
               'run_unchanged': sum(row[10] == 0 for row in table),
               'validation': 'all annotations and original VCF record fields match independent oracle'}
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
               '-b', str(data / 'repeats.bed'), '-g', str(data / 'genome.tsv'),
               '--a-piece', 'whole', '--b-piece', 'whole',
               '--python', str(HERE / 'repeat_run_effect.py'), '-c', 'py:repeat_run',
               '-o', str(output / 'annotated.vcf')]
    env = dict(os.environ, PYTHONHASHSEED='0', LC_ALL='C')
    subprocess.run(command, check=True, env=env)
    header = (output / 'annotated.vcf').read_text().split('#CHROM', 1)[0]
    if '##INFO=<ID=repeat_run,Number=1,Type=String,' not in header:
        raise ValueError('missing or incorrect annotation INFO declaration')
    summary = validate(data, output)
    version = subprocess.check_output([str(binary), '--version'], text=True).strip()
    embedded_python = subprocess.check_output(
        [str(binary), 'intersect', '-a', str(data / 'variants.vcf'), '-b', str(data / 'repeats.bed'),
         '-g', str(data / 'genome.tsv'), '--python', str(HERE / 'python-version.py'),
         '-o', os.devnull], text=True, env=env).strip()
    provenance = {'command': command, 'bedder_version': version, 'bedder_sha256': sha(binary),
                  'driver_python': sys.version, 'embedded_python': embedded_python,
                  'platform': platform.platform(), 'inputs': lock,
                  'scripts': {p.name: sha(p) for p in HERE.glob('*.py')},
                  'outputs': {p.name: sha(p) for p in output.iterdir() if p.is_file()}}
    repo = HERE.parents[1]
    provenance['git_revision'] = subprocess.check_output(
        ['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    provenance['tracked_changes'] = subprocess.check_output(
        ['git', '-C', str(repo), 'diff', 'HEAD', '--', 'src', 'Cargo.toml', 'build.rs'], text=True)
    provenance['cargo_lock_sha256'] = sha(repo / 'Cargo.lock') if (repo / 'Cargo.lock').exists() else None
    (output / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
