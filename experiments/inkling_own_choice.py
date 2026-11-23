"""Own-choice river experiment. Default is an offline plan; inference requires --execute."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import random
import sqlite3
import statistics
import time
from urllib.request import ProxyHandler, build_opener

from inkling_overnight import Study, atomic, best_action, cards, interval, utc
from inkling_own_choice_extra import ExtraStudies, graded

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = ROOT / 'examples/12-inkling-own-choice/protocol.json'
CONDITIONS = ['caller_to_folder', 'folder_to_caller', 'stable_caller', 'stable_folder']


def validate(p):
    assert p['model'] == 'inkling-2-mini'
    assert p['endpoint'] == 'http://127.0.0.1:9932/v1/chat/completions'
    assert p['temperature'] == 0 and 0 < p['max_hours'] <= 13
    assert p['hands'] >= p['switch_range'][1] > p['switch_range'][0] > 0
    assert p['note_every'] > 0 and p['count_window'] > 0
    assert p['probe_after_unobserved_hands'] > 0
    assert 0 < p['freeze_after'] < p['switch_range'][0]
    assert p['freeze_after'] % p['note_every'] == 0
    assert len({m['name'] for m in p['modules']}) == len(p['modules'])
    assert [m['name'] for m in p['modules']] == ['core', 'replication', 'scarce', 'gradual']
    registered = []
    for m in p['modules']:
        assert m['seeds'] > 0 and m['seeds'] % 4 == 0
        assert 0 < m['strong_probability'] < 1
        assert set(m['arms']) <= {'model', 'calculator', 'counts', 'both', 'model_probe', 'calculator_probe', 'model_frozen', 'calculator_frozen'}
        assert len(set(m['arms'])) == len(m['arms'])
        registered.extend(range(m['seed_start'], m['seed_start'] + m['seeds']))
    memory = p['memory_study']
    assert memory['identities'] > 0 and memory['identities'] % 4 == 0
    assert memory['sessions'] >= 2 and memory['hands_per_session'] % p['note_every'] == 0
    assert set(memory['arms']) == {'carry', 'reset'} and len(memory['arms']) == 2
    assert 0 < memory['strong_probability'] < 1
    registered.extend(range(memory['seed_start'], memory['seed_start'] + memory['identities']))
    exam = p['exam']
    assert exam['heldout_every'] >= 2
    assert set(exam['variants']) == {'numeric_label', 'numeric_cards', 'rule_label', 'payoffs_label'}
    assert len(exam['variants']) == 4
    assert exam['pots'] and exam['bet_ratios'] and exam['threshold_margins']
    assert all(pot > 0 for pot in exam['pots']) and all(r > 0 for r in exam['bet_ratios'])
    assert all(0 < ratio / (1+ratio) + margin < 1 for ratio in exam['bet_ratios'] for margin in exam['threshold_margins'])
    groups = len(exam['pots']) * len(exam['bet_ratios']) * len(exam['threshold_margins'])
    registered.extend(range(exam['seed_start'], exam['seed_start'] + groups))
    baseline = p['code_baselines']
    assert baseline['seeds'] > 0 and baseline['seeds'] % 4 == 0
    assert set(baseline['policies']) == {'last5', 'last20', 'last60', 'cumulative', 'decay'}
    assert len(baseline['policies']) == 5
    assert 0 < baseline['decay_per_observed_bet'] < 1
    registered.extend(range(baseline['seed_start'], baseline['seed_start'] + baseline['seeds']))
    assert len(set(registered)) == len(registered)


def plan(p):
    validate(p)
    return {'state': 'prepared; no inference started', 'maximum_hours': p['max_hours'],
            'modules': [{**m, 'hands_total': m['seeds'] * len(m['arms']) * p['hands']} for m in p['modules']],
            'memory_study': p['memory_study'],
            'exam_questions': groups_in_exam(p) * 2 * len(p['exam']['variants']),
            'code_baselines': p['code_baselines'],
            'execution_order': 'core, replication, exam, memory, scarce, code baselines, optional gradual',
            'primary': p['primary'], 'launch_requires': '--execute'}


def groups_in_exam(p):
    return len(p['exam']['pots']) * len(p['exam']['bet_ratios']) * len(p['exam']['threshold_margins'])


def schedule(p, module, seed):
    rng = random.Random(seed)
    condition = CONDITIONS[(seed - module['seed_start']) % 4]
    switch = rng.randint(*p['switch_range'])
    low, high = (.35, .65) if module['name'] == 'gradual' else (.2, .8)
    before, after = {'caller_to_folder': (low, high), 'folder_to_caller': (high, low),
                     'stable_caller': (low, low), 'stable_folder': (high, high)}[condition]
    hands = []
    for h in range(1, p['hands'] + 1):
        strong = rng.random() < module['strong_probability']
        if module['name'] == 'gradual':
            fraction = min(1, max(0, (h - switch + 15) / 30))
        else:
            fraction = int(h > switch)
        hands.append({'hand': h, 'strong': strong, 'deal': cards(rng, strong),
                      'p': before + (after - before) * fraction, 'uniform': rng.random()})
    return condition, switch, hands


def count_estimate(history, window):
    responses = [h['opponent_response'] for h in history if h['your_action'] == 'bet'][-window:]
    return 100 * (responses.count('fold') + 1) / (len(responses) + 2)


class OwnChoice(ExtraStudies, Study):
    # Reuse the archived request/cache implementation, not its old registration or runner.
    def __init__(self, directory, output, protocol):
        validate(protocol)
        self.directory, self.output = Path(directory), Path(output)
        self.protocol = protocol
        self.directory.mkdir(parents=True, exist_ok=True)
        self.output.mkdir(parents=True, exist_ok=True)
        locked = self.directory / 'protocol.json'
        if locked.exists() and json.loads(locked.read_text()) != protocol:
            raise ValueError('Protocol changed; use a new directory')
        atomic(locked, protocol)
        atomic(self.output / 'protocol.json', protocol)
        atomic(self.output / 'registered-seeds.json', [
            {'module': m['name'], 'seed': s, 'condition': CONDITIONS[(s-m['seed_start']) % 4]}
            for m in protocol['modules'] for s in range(m['seed_start'], m['seed_start'] + m['seeds'])])
        atomic(self.output / 'extra-registration.json', {
            'memory_identities': list(range(protocol['memory_study']['seed_start'], protocol['memory_study']['seed_start'] + protocol['memory_study']['identities'])),
            'exam_groups': [{'group': i, 'seed': protocol['exam']['seed_start'] + i,
                             'split': 'heldout' if i % protocol['exam']['heldout_every'] == 0 else 'training_candidate'} for i in range(groups_in_exam(protocol))],
            'code_seeds': list(range(protocol['code_baselines']['seed_start'], protocol['code_baselines']['seed_start'] + protocol['code_baselines']['seeds'])),
            'sequential_games': 'Evaluation only. Training candidates come only from exam development groups.'})
        self.http = build_opener(ProxyHandler({}))
        self.db = sqlite3.connect(self.directory / 'state.sqlite3')
        self.db.execute('pragma journal_mode=WAL')
        self.db.execute('pragma synchronous=FULL')
        self.db.execute('create table if not exists requests (key text primary key, body text, reply text, value text, error text, seconds real, created text)')
        self.db.execute('create table if not exists records (key text primary key, kind text, payload text)')
        self.db.commit()
        started = self.directory / 'started.json'
        if not started.exists():
            atomic(started, {'started': utc(), 'unix': time.time()})
        self.started = json.loads(started.read_text())['unix']
        self.deadline = self.started + protocol['max_hours'] * 3600
        self.stage, self.current, self.new_calls = 'initializing', '', 0
        self.analysis_cache = {}

    def episode(self, module, seed, arm):
        condition, switch, deals = schedule(self.protocol, module, seed)
        prefix = f'{module["name"]}/{seed}/{arm}'
        history, notes, estimate, last_response = [], '', 50., 0
        uses_counts = arm in ('counts', 'both')
        calculator = arm in ('calculator', 'calculator_probe', 'calculator_frozen', 'both')
        for hand in deals:
            if time.time() >= self.deadline:
                raise TimeoutError('13-hour budget reached; partial work saved')
            h = hand['hand']
            key = f'{prefix}/hand/{h:03}'
            row = self.done(key)
            if row is None:
                self.current = key
                current_estimate = count_estimate(history, self.protocol['count_window']) if uses_counts else estimate
                action_error = None
                evidence = {'estimated_opponent_fold_percent': current_estimate}
                visible = {**hand['deal'], 'pot_bb': 10., 'bet_bb': 10., **evidence,
                           'your_hand': 'three of a kind; beats Seat 1' if hand['strong'] else 'unpaired high card; loses to Seat 1'}
                if calculator:
                    proposed = best_action(current_estimate / 100, hand['strong'])
                else:
                    proposed, action_error, visible = self.choose(hand['deal'], hand['strong'], evidence)
                probe = (arm.endswith('_probe') and proposed == 'check' and not hand['strong']
                         and h - last_response >= self.protocol['probe_after_unobserved_hands'])
                action = 'bet' if probe else proposed
                response = ('fold' if hand['uniform'] < hand['p'] else 'call') if action == 'bet' else None
                observed = {'hand': h, 'your_action': action, 'opponent_response': response}
                row = {'module': module['name'], 'seed': seed, 'arm': arm, 'condition': condition,
                       'switch': switch, **hand, 'estimate': current_estimate,
                       'proposed_action': proposed, 'probe': probe, 'action': action,
                       'action_error': action_error, 'observed': observed,
                       'visible': visible, 'model_chooses_bet': not calculator, 'split': 'heldout',
                       **graded(action, hand['p'], hand['strong'])}
                self.record(key, 'hand', row)
            # Only observed actions/responses cross into the model's evidence.
            history.append(row['observed'])
            if row['action'] == 'bet':
                last_response = h
            update_allowed = not arm.endswith('_frozen') or h <= self.protocol['freeze_after']
            if h % self.protocol['note_every'] == 0 and h < self.protocol['hands'] and not uses_counts and update_allowed:
                note_key = f'{prefix}/note/{h:03}'
                checkpoint = self.done(note_key)
                if checkpoint is None:
                    updated, note_error = self.note(notes, history[-self.protocol['note_every']:])
                    value, raw, read_error = self.read({'notes': updated}, estimate)
                    checkpoint = {'module': module['name'], 'seed': seed, 'arm': arm, 'hand': h,
                                  'notes': updated, 'estimate': value, 'raw_read': raw,
                                  'note_error': note_error, 'read_error': read_error}
                    self.record(note_key, 'note', checkpoint)
                notes, estimate = checkpoint['notes'], checkpoint['estimate']
        self.record(prefix + '/complete', 'episode', {'module': module['name'], 'seed': seed, 'arm': arm})

    def write_report(self, state='running', error=None):
        hands = self.records('hand')
        episodes = {(r['module'], r['seed'], r['arm']) for r in self.records('episode')}
        grouped = defaultdict(list)
        for row in hands:
            grouped[row['module'], row['seed'], row['arm']].append(row)
        modules = {}
        for m in self.protocol['modules']:
            seeds = [s for s in range(m['seed_start'], m['seed_start'] + m['seeds'])
                     if all((m['name'], s, a) in episodes for a in m['arms'])]
            if self.analysis_cache.get(m['name'], {}).get('complete_seeds') == len(seeds):
                modules[m['name']] = self.analysis_cache[m['name']]
                continue
            rows = [r for s in seeds for a in m['arms'] for r in grouped[m['name'], s, a]]
            means = {(s, a): statistics.mean(r['regret_bb'] for r in grouped[m['name'], s, a])
                     for s in seeds for a in m['arms']}
            comparisons = [('model', 'calculator'), ('model', 'counts'), ('model', 'both'),
                           ('calculator', 'both'), ('counts', 'both')] if 'both' in m['arms'] else [
                ('model', 'model_probe'), ('calculator', 'calculator_probe')]
            metrics = {}
            for a in m['arms']:
                selected = [r for r in rows if r['arm'] == a]
                changed = [r for r in selected if r['condition'] in CONDITIONS[:2] and r['hand'] > r['switch']]
                metrics[a] = {'mean_regret_bb': statistics.mean(r['regret_bb'] for r in selected) if selected else None,
                              'bets': sum(r['action'] == 'bet' for r in selected),
                              'probes': sum(r['probe'] for r in selected),
                              'post_switch_regret_bb': statistics.mean(r['regret_bb'] for r in changed) if changed else None,
                              'post_switch_read_error_points': statistics.mean(abs(r['estimate'] - 100*r['p']) for r in changed) if changed else None,
                              'early_post_switch_regret_bb': self.segment(changed, 1, 20),
                              'late_post_switch_regret_bb': self.segment(changed, 21, 1000)}
            modules[m['name']] = {'complete_seeds': len(seeds), 'planned_seeds': m['seeds'],
                'arms': metrics, 'comparisons': {f'{a}_minus_{b}': interval([means[s, a] - means[s, b] for s in seeds]) for a, b in comparisons}}
            if 'model_frozen' in m['arms']:
                learning = {}
                for fresh in ['model', 'calculator']:
                    values = defaultdict(list)
                    for seed in seeds:
                        pair = {a: [r for r in grouped[m['name'], seed, a] if r['hand'] > r['switch']] for a in (fresh, fresh + '_frozen')}
                        condition = pair[fresh][0]['condition']
                        saving = statistics.mean(r['regret_bb'] for r in pair[fresh + '_frozen']) - statistics.mean(r['regret_bb'] for r in pair[fresh])
                        values[condition].append(saving)
                    learning[fresh] = {'changed_opponents': interval(values[CONDITIONS[0]] + values[CONDITIONS[1]]),
                                       'by_condition': {c: interval(v) for c, v in values.items()},
                                       'meaning': 'Frozen minus fresh post-switch regret; positive favors updates. Descriptive, whole-seed intervals.'}
                modules[m['name']]['learning'] = learning
            self.analysis_cache[m['name']] = modules[m['name']]
        success = all(modules[n]['complete_seeds'] == modules[n]['planned_seeds'] and
                      modules[n]['comparisons']['model_minus_calculator']['interval_95'] is not None and
                      modules[n]['comparisons']['model_minus_calculator']['interval_95'][0] > 0
                      for n in ('core', 'replication'))
        extras = self.extras_report()
        report = {'state': state, 'updated': utc(), 'error': error, 'modules': modules, 'extras': extras,
                  'success_criterion_met': success, 'saved_hands_including_partial': len(hands),
                  'failure_counts': {'actions': sum(bool(r['action_error']) for r in hands),
                                     'notes': sum(bool(r['note_error']) for r in self.records('note')),
                                     'reads': sum(bool(r['read_error']) for r in self.records('note'))},
                  'unit': 'Whole paired seed; only complete arm sets enter comparisons.'}
        atomic(self.output / 'analysis.json', report)
        for kind in ['hand', 'note', 'episode', 'module']:
            atomic(self.output / (kind + '.json'), self.records(kind))
        lines = ['# Inkling own-choice study', '', f'Status: {state}.', '',
                 'A positive saving means the helper gave up fewer chips per decision.',
                 'Intervals use whole games. Partial games are saved but excluded from comparisons.', '']
        admissions = {r['name']: r['admitted'] for r in self.records('module')}
        for name, data in modules.items():
            lines.append(f'- {name}: {data["complete_seeds"]}/{data["planned_seeds"]} complete matched games.')
            if admissions.get(name) is False:
                lines.append('  - Skipped by the predeclared time rule.')
            for pair, value in data['comparisons'].items():
                if value['mean'] is not None:
                    lines.append(f'  - {pair}: {value["mean"]:.3f} bb saved per decision; 95% interval {value["interval_95"]}.')
            for name, value in data.get('learning', {}).items():
                result = value['changed_opponents']
                if result['mean'] is not None:
                    lines.append(f'  - Updating versus freezing the {name} read after a change: {result["mean"]:.3f} bb saved; interval {result["interval_95"]} (descriptive).')
        lines += ['', f'Across-session memory: {extras["memory"]["complete_identities"]}/{extras["memory"]["planned_identities"]} complete identities; reset minus carry: {extras["memory"]["reset_minus_carry_bb"]}.',
                  f'Mistake exam: {extras["exam_questions_saved"]}/{extras["exam_questions_planned"]} saved questions.',
                  f'Code-only memory comparison: {extras["code_baselines"]["complete_seeds"]}/{extras["code_baselines"]["planned_seeds"]} complete seeds.',
                  'The format/card-reading exam and kept-memory results are in extra-analysis.json. Only exam development groups enter training-candidates.json.']
        lines += ['', f'Predeclared helper-benefit criterion met: {success}.',
                  'This tests local river decisions with supplied hand strength. It does not prove general poker learning.',
                  'The both arm uses only code. Probe costs are included. No weights were trained.']
        if error:
            lines += ['', f'Stopped: {error}']
        atomic(self.output / 'summary.md', '\n'.join(lines) + '\n')
        if success:
            recommendation = ('Use the fixed betting calculator next. It helped in both the main test and the fresh repeat. '
                              'This is evidence for a useful helper; it does not mean Inkling learned to do the calculation itself.')
        elif any(modules[n]['complete_seeds'] < modules[n]['planned_seeds'] for n in ('core', 'replication')):
            recommendation = 'The main comparison is unfinished. Keep the saved data, but wait for complete matched games before choosing a winner.'
        else:
            recommendation = 'The calculator benefit did not pass the predeclared test and fresh repeat. Do not treat a favorable side result as proof; inspect the intervals before deciding whether more games are worthwhile.'
        takeaways = ['# What this test says', '', f'Status: {state}.', '', recommendation, '']
        if success:
            memory = [modules[n]['comparisons']['calculator_minus_both'] for n in ('core', 'replication')]
            if all(v['interval_95'] and v['interval_95'][0] > 0 for v in memory):
                takeaways += ['Reliable recent counts also helped after the calculator was added in both samples. That makes better memory the next candidate. This comparison is descriptive, not the primary test.', '']
            else:
                takeaways += ['Extra benefit from recent counts after adding the calculator remains uncertain. Read the saved intervals before prioritizing more memory work.', '']
        takeaways += ['Fine-tuning was not tested. These games can locate mistakes to investigate, but cannot show that training would fix them.', '',
                     'The extra tests compare updated versus frozen reads, kept versus erased memory across sessions, and four ways to present a betting question. All are descriptive; incomplete sections remain unresolved. See extra-analysis.json.', '',
                     'The scarce-evidence test asks whether paying for occasional test bets helps overall. Their losses count. Gradual changes are an optional stress test. The code-only baseline does not demonstrate model learning.', '',
                     'Detailed numbers: [summary](summary.md). All hands, including partial games and failures, are saved.']
        mem = extras['memory']
        memory_result = mem['reset_minus_carry_bb']
        if memory_result['mean'] is not None:
            takeaways += ['', f'Keeping memory saved {memory_result["mean"]:.3f} bb per decision in later sessions, with a 95% interval of {memory_result["interval_95"]}, across {mem["complete_identities"]}/{mem["planned_identities"]} complete opponents. Positive favors keeping memory. Check the separate stable/changing opponent results before relying on this average.']
        exam_pairs = extras['exam']['heldout']['paired_groups']
        labels = {'numeric_label_minus_rule_label': 'Adding a simple betting rule',
                  'numeric_label_minus_payoffs_label': 'Supplying the calculated chip values',
                  'numeric_cards_minus_numeric_label': 'Supplying the hand strength'}
        for pair, value in exam_pairs['comparisons'].items():
            if value['mean'] is not None:
                takeaways += ['', f'{labels[pair]} saved {value["mean"]:.3f} bb per exam question; 95% interval {value["interval_95"]}. This uses {exam_pairs["complete_groups"]} complete held-out situation groups and does not establish a benefit in sequential play.']
        atomic(self.output / 'takeaways.md', '\n'.join(takeaways) + '\n')
        self.progress(state, error)
        return report

    @staticmethod
    def segment(rows, first, last):
        values = [r['regret_bb'] for r in rows if first <= r['hand'] - r['switch'] <= last]
        return statistics.mean(values) if values else None

    def run(self):
        try:
            self.preflight()
            for m in self.protocol['modules']:
                if m['name'] == 'scarce':
                    self.exam()
                    self.memory_study()
                if m['name'] == 'gradual':
                    self.code_baselines()
                self.stage = m['name']
                admission_key = 'module/' + m['name']
                admission = self.done(admission_key)
                if admission is None:
                    admitted = not m['optional'] or (self.deadline - time.time()) / 3600 >= m['admit_remaining_hours']
                    admission = {'name': m['name'], 'admitted': admitted, 'remaining_hours': (self.deadline - time.time()) / 3600}
                    self.record(admission_key, 'module', admission)
                if not admission['admitted']:
                    continue
                for seed in range(m['seed_start'], m['seed_start'] + m['seeds']):
                    # Rotate order, so slowdowns/deadlines do not always favor the same arm.
                    offset = (seed - m['seed_start']) % len(m['arms'])
                    for arm in m['arms'][offset:] + m['arms'][:offset]:
                        if not self.done(f'{m["name"]}/{seed}/{arm}/complete'):
                            self.episode(m, seed, arm)
                    self.write_report()
            return self.write_report('complete')
        except Exception as exc:
            self.write_report('incomplete', f'{type(exc).__name__}: {exc}')
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path, nargs='?', default=ROOT / 'runs/inkling-own-choice-v1')
    parser.add_argument('output', type=Path, nargs='?', default=ROOT / 'results/12-inkling-own-choice')
    parser.add_argument('--protocol', type=Path, default=PROTOCOL)
    parser.add_argument('--execute', action='store_true', help='Explicitly authorize inference for this invocation')
    parser.add_argument('--report-only', action='store_true')
    args = parser.parse_args()
    p = json.loads(args.protocol.read_text())
    if not args.execute:
        print(json.dumps(plan(p), indent=2))
        return
    study = OwnChoice(args.directory, args.output, p)
    try:
        if args.report_only:
            study.write_report('report-only')
        else:
            study.run()
    finally:
        study.db.close()


if __name__ == '__main__':
    main()
