"""Offline tests only: no local model or server is contacted."""
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from inkling_own_choice import OwnChoice, PROTOCOL, count_estimate, plan, schedule
from inkling_overnight import loss
from inkling_own_choice_extra import policy_estimate


def small_protocol():
    p = json.loads(PROTOCOL.read_text())
    p.update(hands=8, note_every=2, switch_range=[3, 5], freeze_after=2, probe_after_unobserved_hands=3)
    for m in p['modules']:
        m['seeds'] = 4
        m['optional'] = False
    p['memory_study'].update(identities=4, sessions=2, hands_per_session=4)
    p['exam'].update(pots=[10], bet_ratios=[1], threshold_margins=[-.02, .02], heldout_every=2)
    p['code_baselines']['seeds'] = 4
    return p


class Fake(OwnChoice):
    calls = 0

    def ask(self, system, observation, field, options=None):
        self.calls += 1
        # Hidden grader fields must never reach model requests.
        serialized = json.dumps(observation)
        for hidden in ['"uniform"', '"p"', '"condition"', '"switch"', '"regret_bb"']:
            assert hidden not in serialized, hidden
        if field == 'notes':
            return json.dumps(observation['completed_hands'], sort_keys=True), None
        if field == 'fold_percent':
            return 25., None
        return ('bet' if observation.get('your_hand', '').startswith('three') else 'check'), None

    def preflight(self):
        pass


class PreparedStudyTests(unittest.TestCase):
    def test_default_commands_do_not_create_runs_or_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            run, out = Path(tmp) / 'run', Path(tmp) / 'out'
            for script in ['inkling_own_choice.py', 'run_inkling_own_choice.py']:
                result = subprocess.run([sys.executable, '-B', str(Path(__file__).with_name(script)), str(run), str(out)],
                                        capture_output=True, text=True, check=True)
                offline = json.loads(result.stdout)
                self.assertEqual(offline['state'], 'prepared; no inference started')
                self.assertEqual(offline['exam_questions'], 512)
                self.assertEqual(offline['memory_study']['identities'], 12)
                self.assertFalse(run.exists())
                self.assertFalse(out.exists())

    def test_counts_use_only_observed_bets_and_window(self):
        history = [{'your_action': 'bet', 'opponent_response': 'fold'}] * 30
        history += [{'your_action': 'check', 'opponent_response': None}] * 100
        self.assertAlmostEqual(count_estimate(history, 20), 100*21/22)
        history += [{'your_action': 'bet', 'opponent_response': 'call'}] * 20
        self.assertAlmostEqual(count_estimate(history, 20), 100/22)
        self.assertEqual(count_estimate([], 20), 50)

    def test_exogenous_matching_and_policy(self):
        p = small_protocol()
        m = p['modules'][0]
        self.assertEqual(schedule(p, m, m['seed_start']), schedule(p, m, m['seed_start']))
        for i in range(4):
            condition, switch, hands = schedule(p, m, m['seed_start'] + i)
            self.assertEqual(len(hands), p['hands'])
            self.assertAlmostEqual(hands[0]['p'], .8 if i in (1, 3) else .2)
            self.assertAlmostEqual(hands[-1]['p'], .8 if i in (0, 3) else .2)

    def test_frozen_source_bundle_can_display_plan_without_starting(self):
        from run_inkling_own_choice import FILES, ROOT
        with tempfile.TemporaryDirectory() as tmp:
            snapshot = Path(tmp)/'source'
            snapshot.mkdir()
            for name in FILES:
                (snapshot/Path(name).name).write_bytes((ROOT/name).read_bytes())
            result = subprocess.run([sys.executable, '-B', str(snapshot/'inkling_own_choice.py'),
                str(Path(tmp)/'run'), str(Path(tmp)/'out'), '--protocol', str(snapshot/'protocol.json')],
                capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(result.stdout)['exam_questions'], 512)
            self.assertFalse((Path(tmp)/'run').exists())
            self.assertFalse((Path(tmp)/'out').exists())

    def test_full_fake_run_resume_and_all_probe_costs(self):
        with tempfile.TemporaryDirectory() as tmp, patch('urllib.request.OpenerDirector.open', side_effect=AssertionError('Network forbidden')):
            p = small_protocol()
            study = Fake(Path(tmp) / 'run', Path(tmp) / 'out', p)
            report = study.run()
            self.assertEqual(report['state'], 'complete')
            self.assertEqual(report['saved_hands_including_partial'], sum(m['seeds']*len(m['arms'])*p['hands'] for m in p['modules']))
            self.assertEqual(report['extras']['memory']['complete_identities'], 4)
            self.assertEqual(report['extras']['code_baselines']['complete_seeds'], 4)
            self.assertEqual(report['extras']['exam_questions_saved'], 16)
            self.assertEqual(report['extras']['exam']['heldout']['paired_groups']['complete_groups'], 1)
            candidates = json.loads((Path(tmp)/'out'/'training-candidates.json').read_text())
            self.assertEqual(len(candidates), 8)
            self.assertTrue(all(r['split'] == 'training_candidate' for r in candidates))
            calls = study.calls
            study.run()
            self.assertEqual(study.calls, calls)
            rows = study.records('hand')
            probes = [r for r in rows if r['probe']]
            self.assertTrue(probes)
            for r in probes:
                self.assertEqual(r['action'], 'bet')
                self.assertEqual(r['regret_bb'], loss('bet', r['p'], r['strong']))
            self.assertTrue(any(r['regret_bb'] > 0 for r in probes))
            # Same hidden outcomes; histories can diverge because actions differ.
            for module in p['modules']:
                subset = [r for r in rows if r['module'] == module['name'] and r['seed'] == module['seed_start'] and r['hand'] == 1]
                self.assertEqual(len({json.dumps(r['deal'], sort_keys=True) for r in subset}), 1)
                self.assertEqual(len({r['uniform'] for r in subset}), 1)
            changed = deepcopy(p)
            changed['tokens']['notes'] += 1
            with self.assertRaises(ValueError):
                Fake(Path(tmp) / 'run', Path(tmp) / 'out', changed)
            study.db.close()

    def test_frozen_reads_stop_updating_before_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = small_protocol()
            study = Fake(Path(tmp)/'run', Path(tmp)/'out', p)
            m = p['modules'][0]
            study.episode(m, m['seed_start'], 'model_frozen')
            self.assertEqual([r['hand'] for r in study.records('note')], [p['freeze_after']])
            self.assertTrue(all(r['estimate'] == 25 for r in study.records('hand') if r['hand'] > p['freeze_after']))
            study.db.close()

    def test_memory_carries_or_resets_and_conditions_change_at_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = small_protocol()
            study = Fake(Path(tmp)/'run', Path(tmp)/'out', p)
            study.memory_study()
            rows = study.records('memory_hand')
            for seed in range(p['memory_study']['seed_start'], p['memory_study']['seed_start']+4):
                carry = next(r for r in rows if r['seed'] == seed and r['arm'] == 'carry' and r['session'] == 2 and r['hand'] == 1)
                reset = next(r for r in rows if r['seed'] == seed and r['arm'] == 'reset' and r['session'] == 2 and r['hand'] == 1)
                self.assertEqual(carry['estimate'], 25)
                self.assertEqual(reset['estimate'], 50)
                self.assertEqual(carry['deal'], reset['deal'])
                first = next(r for r in rows if r['seed'] == seed and r['arm'] == 'carry' and r['session'] == 1 and r['hand'] == 1)
                self.assertEqual(first['p'] != carry['p'], carry['condition'] in ('caller_to_folder', 'folder_to_caller'))
            study.db.close()

    def test_code_baselines_get_only_their_own_observed_responses(self):
        history = [{'your_action': 'bet', 'opponent_response': 'fold'}]*30
        history += [{'your_action': 'bet', 'opponent_response': 'call'}]*6
        self.assertAlmostEqual(policy_estimate(history, 'last5'), 100/7)
        self.assertGreater(policy_estimate(history, 'cumulative'), policy_estimate(history, 'decay'))
        history += [{'your_action': 'check', 'opponent_response': None}]*100
        self.assertAlmostEqual(policy_estimate(history, 'last5'), 100/7)
        with tempfile.TemporaryDirectory() as tmp:
            p = small_protocol()
            study = Fake(Path(tmp)/'run', Path(tmp)/'out', p)
            with patch.object(study, 'ask', side_effect=AssertionError('Code-only study')):
                study.code_baselines()
            games = study.records('baseline_game')
            self.assertEqual(len(games), 20)
            for game in games:
                seen = []
                for row in game['trajectory']:
                    self.assertAlmostEqual(row['estimate'], policy_estimate(seen, game['policy']))
                    if row['action'] == 'check':
                        self.assertIsNone(row['response'])
                    else:
                        self.assertEqual(row['response'], 'fold' if row['uniform'] < row['p'] else 'call')
                    seen.append({'your_action': row['action'], 'opponent_response': row['response']})
            study.db.close()

    def test_programmed_baseline_never_calls_model_and_partial_is_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = small_protocol()
            study = Fake(Path(tmp) / 'run', Path(tmp) / 'out', p)
            m = p['modules'][0]
            with patch.object(study, 'ask', side_effect=AssertionError('No model in both')):
                study.episode(m, m['seed_start'], 'both')
            report = study.write_report('incomplete')
            self.assertEqual(report['modules']['core']['complete_seeds'], 0)
            self.assertEqual(report['saved_hands_including_partial'], 8)
            study.deadline = 0
            with self.assertRaises(TimeoutError):
                study.run()
            self.assertEqual(json.loads((Path(tmp)/'out'/'analysis.json').read_text())['state'], 'incomplete')
            study.db.close()

    def test_mid_game_resume_reconstructs_private_memory(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = small_protocol()
            m = p['modules'][0]
            run, out = Path(tmp)/'run', Path(tmp)/'out'
            study = Fake(run, out, p)
            original = study.choose
            calls = 0

            def interrupted(*args, **kwargs):
                nonlocal calls
                calls += 1
                if calls == 4:
                    raise ConnectionError('simulated interruption')
                return original(*args, **kwargs)

            with patch.object(study, 'choose', side_effect=interrupted):
                with self.assertRaises(ConnectionError):
                    study.episode(m, m['seed_start'], 'model')
            self.assertEqual(len(study.records('hand')), 3)
            study.db.close()
            resumed = Fake(run, out, p)
            resumed.episode(m, m['seed_start'], 'model')
            fresh = Fake(Path(tmp)/'fresh', Path(tmp)/'fresh-out', p)
            fresh.episode(m, m['seed_start'], 'model')
            self.assertEqual(resumed.records('hand'), fresh.records('hand'))
            self.assertEqual(resumed.records('note'), fresh.records('note'))
            resumed.db.close()
            fresh.db.close()


if __name__ == '__main__':
    unittest.main()
