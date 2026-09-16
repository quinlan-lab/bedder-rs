import itertools
import json
import random
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace

from repeat_run_effect import bedder_repeat_run, effect, longest_run
from run import oracle_run


class RepeatTests(unittest.TestCase):
    def test_frozen_hg002_end_to_end(self):
        here = Path(__file__).resolve().parent
        binary = here.parents[1] / 'target/release/bedder'
        if not binary.exists():
            self.skipTest('build target/release/bedder to run the HG002 integration test')
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'results'
            subprocess.run([sys.executable, str(here / 'run.py'), '--output', str(output)],
                           check=True, capture_output=True, text=True)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual([summary[k] for k in ['validated_pairs', 'unique_variants',
                             'run_increased', 'run_decreased', 'run_unchanged']],
                             [561, 551, 48, 225, 288])
            provenance = json.loads((output / 'provenance.json').read_text())
            self.assertTrue(provenance['embedded_python'])

    def test_interruption_removed(self):
        self.assertEqual(effect('CAGCAGCAACAGCAG', 'CAG', 8, 'A', 'G'), (2, 5, 3))

    def test_interruption_created(self):
        self.assertEqual(effect('CAG' * 5, 'CAG', 8, 'G', 'A'), (5, 2, -3))

    def test_boundary_and_phase(self):
        self.assertEqual(effect('AGCAGCAGC', 'CAG', 0, 'A', 'T'), (3, 2, -1))
        self.assertEqual(longest_run('CAGCAGCA', 'CAG'), 2)
        self.assertEqual(longest_run('CTGCTG', 'CAG'), 0)

    def test_bad_reference_and_non_snv(self):
        for args in [('CAG', 'CAG', 0, 'T', 'A'), ('CAG', 'CAG', 0, 'C', 'CA'),
                     ('CAG', 'CAG', 0, 'C', 'C'), ('CNG', 'CAG', 0, 'C', 'A'),
                     ('CAG', '', 0, 'C', 'A'), ('CAG', 'C.G', 0, 'C', 'A')]:
            with self.assertRaises(ValueError):
                effect(*args)

    def test_multiple_repeats_rejected_for_number_one_info(self):
        fragment = SimpleNamespace(a=SimpleNamespace(vcf=lambda: SimpleNamespace(ALT=['A'])),
                                   b=[None, None])
        with self.assertRaisesRegex(ValueError, 'one repeat per record'):
            bedder_repeat_run(fragment)

    def test_seeded_snv_effects_against_oracle(self):
        rng = random.Random(20260915)
        for _ in range(500):
            motif = ''.join(rng.choices('ACGT', k=rng.randint(2, 6)))
            sequence = list(motif * rng.randint(2, 15))
            for _ in range(rng.randint(0, 6)):
                sequence[rng.randrange(len(sequence))] = rng.choice('ACGT')
            sequence = ''.join(sequence)
            offset = rng.randrange(len(sequence))
            ref = sequence[offset]
            alt = rng.choice([base for base in 'ACGT' if base != ref])
            alternate = sequence[:offset] + alt + sequence[offset + 1:]
            before, after = oracle_run(sequence, motif), oracle_run(alternate, motif)
            self.assertEqual(effect(sequence, motif, offset, ref, alt),
                             (before, after, after - before))
            self.assertEqual(effect(alternate, motif, offset, alt, ref),
                             (after, before, before - after))

    def test_overlapping_regex_starts_hg002_regression(self):
        sequence = 'AAATAATAAATAAATAAATACATAAATAAATAAATAAATA'
        self.assertEqual(effect(sequence, 'AAAT', sequence.index('C'), 'C', 'A'), (4, 9, 5))

    def test_regex_against_independent_exhaustive_oracle(self):
        for n in range(8):
            for bases in itertools.product('AC', repeat=n):
                sequence = ''.join(bases)
                for motif in ['AC', 'AAC', 'ACC', 'ACAC']:
                    self.assertEqual(longest_run(sequence, motif), oracle_run(sequence, motif))


if __name__ == '__main__':
    unittest.main()
