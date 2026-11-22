"""Check grading, independent card labels, recovery, and the experiment loop offline."""
import itertools
import json
from pathlib import Path
import random
import tempfile
import unittest
from unittest.mock import patch

import inkling_overnight as experiment
import inkling_takeaways as briefing


def five_rank(cards):
    ranks = [experiment.RANKS.index(c[0]) + 2 for c in cards]
    groups = sorted(((ranks.count(r), r) for r in set(ranks)), reverse=True)
    flush = len({c[1] for c in cards}) == 1
    values = set(ranks) | ({1} if 14 in ranks else set())
    run = max((low + 4 for low in range(1, 11) if set(range(low, low + 5)) <= values), default=0)
    if flush and run:
        return (8, run)
    if groups[0][0] == 4:
        return (7, groups[0][1], groups[1][1])
    if [g[0] for g in groups] == [3, 2]:
        return (6, groups[0][1], groups[1][1])
    if flush:
        return (5, *sorted(ranks, reverse=True))
    if run:
        return (4, run)
    if groups[0][0] == 3:
        return (3, groups[0][1], *sorted((r for r in ranks if r != groups[0][1]), reverse=True))
    pairs = sorted((rank for count, rank in groups if count == 2), reverse=True)
    singles = sorted((rank for count, rank in groups if count == 1), reverse=True)
    return (2, *pairs, *singles) if len(pairs) == 2 else (1, *pairs, *singles) if pairs else (0, *singles)


def seven_rank(cards):
    return max(five_rank(hand) for hand in itertools.combinations(cards, 5))


class Tests(unittest.TestCase):
    def test_generated_labels_against_independent_poker_evaluator(self):
        for high, strong in itertools.product([10, 11, 12, 13, 14], [False, True]):
            for seed in range(50):
                deal = experiment.cards(random.Random(seed), strong, high)
                visible = deal['board'] + deal['hole']
                top = experiment.RANKS[high - 2]
                pair = next(top + s for s in experiment.SUITS if top + s not in visible)
                kicker = next(r + s for r in experiment.RANKS if r not in [c[0] for c in deal['board']]
                              for s in experiment.SUITS if r + s not in visible)
                hero = seven_rank(visible)
                opponent = seven_rank(deal['board'] + [pair, kicker])
                self.assertEqual(opponent[0], 1)
                self.assertEqual(hero[0], 3 if strong else 0)
                self.assertEqual(hero > opponent, strong)

    def test_payoffs_and_thresholds(self):
        self.assertAlmostEqual(experiment.payoff('bet', .8, False), 1)
        self.assertAlmostEqual(experiment.loss('check', .8, False), 6)
        self.assertAlmostEqual(experiment.loss('bet', .2, False), 6)
        for pot, bet in itertools.product([5., 10., 40.], [2., 5., 20.]):
            threshold = bet / (pot + bet)
            self.assertEqual(experiment.best_action(threshold - .001, False, pot, bet), 'check')
            self.assertEqual(experiment.best_action(threshold + .001, False, pot, bet), 'bet')
        self.assertEqual(experiment.percent(8.333333), 8.3)

    def test_disk_cache_failure_recovery_and_protocol_guard(self):
        class Response:
            def __enter__(self):
                return self
            def __exit__(self, *args):
                pass
            def read(self):
                return json.dumps({'model': 'inkling-2-mini', 'choices': [
                    {'finish_reason': 'length', 'message': {'content': ''}}]}).encode()
        class HTTP:
            def __init__(self):
                self.calls = 0
            def open(self, *args, **kwargs):
                self.calls += 1
                return Response()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            study = experiment.Study(root / 'run', root / 'out')
            http = HTTP()
            study.http = http
            self.assertEqual(study.note('old evidence', [{'hand': 1}])[0], 'old evidence')
            self.assertEqual(http.calls, 1)
            resumed = experiment.Study(root / 'run', root / 'out')
            resumed.http = http
            self.assertEqual(resumed.note('old evidence', [{'hand': 1}])[0], 'old evidence')
            self.assertEqual(http.calls, 1)
            self.assertEqual(resumed.choose({}, False, {})[0], 'check')
            self.assertEqual(resumed.read({})[0], 50.)
            changed = dict(study.protocol, version=999)
            with self.assertRaises(AssertionError):
                experiment.Study(root / 'run', root / 'out', changed)
            study.db.close()
            resumed.db.close()

    def test_full_offline_loop_report_resume_and_split(self):
        class Perfect(experiment.Study):
            def preflight(self):
                pass
            def ask(self, system, observation, field, options=None):
                if field == 'notes':
                    responses = [h['opponent_response'] for h in observation['completed_hands'] if h['your_action'] == 'bet']
                    return json.dumps(responses), None
                if field == 'fold_percent':
                    if 'private_notebook' in observation:
                        responses = json.loads(observation['private_notebook'] or '[]')
                    elif 'completed_hands' in observation:
                        responses = [h['opponent_response'] for h in observation['completed_hands'][-20:]]
                    elif 'responses_in_order' in observation:
                        responses = observation['responses_in_order']
                    else:
                        return 100 * observation['folds'] / observation['bets_faced'], None
                    return (100 * responses.count('fold') / len(responses) if responses else 50.), None
                return 'check', None
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            protocol = dict(json.loads(experiment.PROTOCOL.read_text()), histories_per_condition=1, online_pairs=2)
            study = Perfect(root / 'run', root / 'out', protocol)
            with patch.object(experiment, 'interval', lambda values, repetitions=0: {
                'n': len(values), 'mean': sum(values) / len(values) if values else None, 'interval_95': None}):
                study.run()
                analysis = json.loads((root / 'out/analysis.json').read_text())
                self.assertEqual(analysis['state'], 'complete')
                self.assertEqual(analysis['complete_histories'], 4)
                self.assertEqual(analysis['completed_online_pairs'], 2)
                self.assertEqual(len(study.records('controlled_probe')), 4 * 6 * 4 * 10)
                self.assertEqual(len(study.records('count_diagnostic')), 240)
                self.assertEqual(len(study.records('bet_diagnostic')), 960)
                heldout = [r for r in study.records('bet_diagnostic') if r['split'] == 'heldout']
                self.assertEqual({r['strong'] for r in heldout}, {True, False})
                self.assertEqual({max(experiment.RANKS.index(c[0]) + 2 for c in r['visible']['board']) for r in heldout}, {10, 11})
                old_counts = list(study.db.execute('select kind,count(*) from records group by kind'))
                study.run()
                self.assertEqual(old_counts, list(study.db.execute('select kind,count(*) from records group by kind')))
                self.assertTrue((root / 'out/summary.md').exists())
                text = briefing.render(analysis, root / 'out')
                self.assertIn('Fine-tuning remains untested', text)
                self.assertIn('paired own-choice games', text)
                self.assertNotIn('study is still running', text)
                provisional = dict(analysis, state='incomplete', error='test interruption')
                text = briefing.render(provisional, root / 'out')
                self.assertIn('stopped before completion', text)
                self.assertIn('test interruption', text)
            study.db.close()


if __name__ == '__main__':
    unittest.main()
