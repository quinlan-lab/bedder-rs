import csv
import gzip
import itertools
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

from segdup_best_match import best_match, bedder_sd_count_interchrom
from run import sha

HERE = Path(__file__).resolve().parent
BINARY = HERE.parents[1] / 'target/release/bedder'


def overlap(name, score, partner='chr19', start=100, end=200):
    bed = SimpleNamespace(chrom='chr19', start=start, stop=end, name=name,
        other_fields=lambda: ['+', 'fracMatch=' + score, partner, '500', '600', '1'])
    return SimpleNamespace(bed=lambda: bed)


class SegdupTests(unittest.TestCase):
    def test_download_checksum_matches_manuscript_source(self):
        expected = json.loads((HERE / 'data/inputs.lock.json').read_text())['sources']['segdups']['sha256']
        digest, filename = (HERE / 'genomicSuperDups.sha256').read_text().split()
        self.assertEqual(digest, expected)
        self.assertEqual(filename, 'genomicSuperDups.txt.gz')

    @unittest.skipUnless(shutil.which('bedtools'), 'BEDtools is required for the quick-start pipeline')
    def test_quick_start_pipeline(self):
        blocks = re.findall(r'```bash\n(.*?)\n```', (HERE / 'README.md').read_text(), re.S)
        pipeline = next(block for block in blocks if 'gzip -dc genomicSuperDups.txt.gz |' in block)
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            (directory / 'hg38.fai').write_text('chr19\t1000\n')
            rows = []
            # UCSC columns: bin, chrom, start, end, name, score, strand,
            # otherChrom, otherStart, otherEnd, otherSize, uid, ..., fracMatch.
            for chrom, start, partner, uid in [('chr19', 20, 'chr2', 7),
                    ('chr19', 10, 'chr19_alt', 8), ('chr19', 10, 'chr19', 9),
                    ('chr19_alt', 10, 'chr2', 10)]:
                row = ['0'] * 30
                row[1:12] = [chrom, str(start), str(start + 5), 'unused', '0', '+',
                             partner, '500', '600', '100', str(uid)]
                row[26] = '0.999949'
                rows.append('\t'.join(row) + '\n')
            with gzip.open(directory / 'genomicSuperDups.txt.gz', 'wt') as stream:
                stream.writelines(rows)
            subprocess.run(['bash', '-euc', pipeline], cwd=directory, check=True,
                           capture_output=True, text=True)
            output = (directory / 'segdups.bed').read_text().splitlines()
            self.assertEqual(output, [
                'chr19\t10\t15\tsd_9_3\t0\t+\tfracMatch=0.999949\tchr19\t500\t600\t9',
                'chr19\t20\t25\tsd_7_1\t0\t+\tfracMatch=0.999949\tchr2\t500\t600\t7'])

    def test_selects_record_and_interchrom_filter(self):
        fragment = SimpleNamespace(b=[overlap('intra', '0.99999'), overlap('inter', '0.95', 'chr2')])
        self.assertTrue(best_match(fragment).endswith('|intra'))
        self.assertTrue(best_match(fragment, True).endswith('|inter'))
        self.assertEqual(bedder_sd_count_interchrom(fragment), 1)

    def test_identity_precision(self):
        fragment = SimpleNamespace(b=[overlap('a', '0.999941'), overlap('z', '0.999949')])
        self.assertTrue(best_match(fragment).endswith('|z'))

    def test_ties_do_not_depend_on_order(self):
        candidates = [overlap('z', '0.99'), overlap('a', '0.99'), overlap('lower', '0.98')]
        for order in itertools.permutations(candidates):
            self.assertTrue(best_match(SimpleNamespace(b=order)).endswith('|a'))

    def test_empty_and_intra_only(self):
        self.assertEqual(best_match(SimpleNamespace(b=[])), '.')
        self.assertEqual(best_match(SimpleNamespace(b=[overlap('intra', '1')]), True), '.')

    def test_invalid_identity(self):
        for score in ['NaN', 'Infinity', '-0.1', '90']:
            with self.assertRaises(ValueError):
                best_match(SimpleNamespace(b=[overlap('bad', score)]))

    @unittest.skipUnless(BINARY.exists(), 'build target/release/bedder for CLI tests')
    def test_frozen_hg002(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'results'
            subprocess.run([sys.executable, str(HERE / 'run.py'), '--output', str(output)],
                           check=True, capture_output=True, text=True)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual([summary[k] for k in ['variants', 'with_interchrom_sd',
                             'with_intra_only', 'different_best_when_interchrom_only']],
                             [6205, 902, 5303, 99])

    @unittest.skipUnless(BINARY.exists(), 'build target/release/bedder for CLI tests')
    def test_cli_boundaries_precision_ties_and_no_overlap(self):
        with tempfile.TemporaryDirectory() as tmp:
            data = Path(tmp) / 'data'
            data.mkdir()
            (data / 'genome.tsv').write_text('chr19\t1000\n')
            (data / 'variants.vcf').write_text(
                '##fileformat=VCFv4.2\n##contig=<ID=chr19,length=1000>\n'
                '#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n' +
                ''.join(f'chr19\t{pos}\t.\tA\tC\t.\tPASS\t.\n' for pos in [101, 121, 181, 201]))
            source_rows = []
            beds = []
            for name, start, end, score, partner in [
                    ('local', 100, 200, '0.999941', 'chr19'),
                    ('a', 120, 180, '0.999949', 'chr2'),
                    ('b', 120, 180, '0.999949', 'chr2')]:
                source_rows.append(dict(chrom='chr19', chromStart=str(start), chromEnd=str(end),
                    strand='+', fracMatch=score, otherChrom=partner, otherStart='500', otherEnd='600', uid='1'))
                beds.append(f'chr19\t{start}\t{end}\t{name}\t0\t+\tfracMatch={score}\t{partner}\t500\t600\t1\n')
            (data / 'segdups.bed').write_text(''.join(beds))
            with (data / 'source-rows.tsv').open('w') as stream:
                writer = csv.DictWriter(stream, fieldnames=list(source_rows[0]), delimiter='\t', lineterminator='\n')
                writer.writeheader()
                writer.writerows(source_rows)
            lock = {'files': {p.name: sha(p) for p in data.iterdir()}}
            (data / 'inputs.lock.json').write_text(json.dumps(lock))
            output = Path(tmp) / 'results'
            result = subprocess.run([sys.executable, str(HERE / 'run.py'), '--data', str(data), '--output', str(output)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            with (output / 'best-matches.tsv').open() as stream:
                rows = list(csv.DictReader(stream, delimiter='\t'))
            self.assertEqual(len(rows), 4)
            self.assertTrue(rows[1]['sd_best'].startswith('0.999949|'))
            self.assertTrue(rows[1]['sd_best'].endswith('|a'))
            self.assertEqual([r['sd_count'] for r in rows], ['1', '3', '1', '0'])
            self.assertEqual(rows[-1]['sd_best'], '.')
            self.assertEqual(rows[0]['sd_best_interchrom'], '.')


if __name__ == '__main__':
    unittest.main()
