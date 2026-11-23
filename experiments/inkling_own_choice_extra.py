"""Additional registered studies, invoked only by the explicitly executed worker.

All observations and hidden grading data stay separate. No work runs on import.
"""
from collections import defaultdict
import itertools
import random
import statistics
import time

from inkling_overnight import RULES, atomic, best_action, cards, interval, loss, payoff

RULE = (' For a hand that loses at showdown, bet when the fold probability is greater '
        'than bet_bb / (pot_bb + bet_bb); below that threshold check. '
        'With a hand that beats the opponent, betting earns at least as much as checking.')
MEMORY_CONDITIONS = ['stable_caller', 'stable_folder', 'caller_to_folder', 'folder_to_caller']


def graded(action, p, strong, pot=10., bet=10.):
    return {'best_action': best_action(p, strong, pot, bet),
            'action_values_bb': {a: payoff(a, p, strong, pot, bet) for a in ('check', 'bet')},
            'expected_bb': payoff(action, p, strong, pot, bet), 'regret_bb': loss(action, p, strong, pot, bet)}


def policy_estimate(history, policy, decay=.95):
    responses = [h['opponent_response'] for h in history if h['your_action'] == 'bet']
    if policy.startswith('last'):
        responses = responses[-int(policy[4:]):]
    if policy == 'decay':
        weights = [decay ** i for i in reversed(range(len(responses)))]
        return 100 * (1 + sum(w for w, r in zip(weights, responses) if r == 'fold')) / (2 + sum(weights))
    return 100 * (1 + responses.count('fold')) / (2 + len(responses))


class ExtraStudies:
    def budget_check(self):
        if time.time() >= self.deadline:
            raise TimeoutError('Study budget reached; partial data saved')

    def exam(self):
        config = self.protocol['exam']
        self.stage = 'compact mistake exam'
        groups = itertools.product(config['pots'], config['bet_ratios'], config['threshold_margins'])
        for group, (pot, ratio, margin) in enumerate(groups):
            seed = config['seed_start'] + group
            split = 'heldout' if group % config['heldout_every'] == 0 else 'training_candidate'
            probability = ratio / (1 + ratio) + margin
            for strong in (False, True):
                rng = random.Random(seed * 2 + strong)
                deal = cards(rng, strong, rng.choice([10, 11] if split == 'heldout' else [12, 13]))
                for variant in config['variants']:
                    key = f'exam/{group:03}/{int(strong)}/{variant}'
                    if self.done(key):
                        continue
                    self.budget_check()
                    self.current = key
                    obs = {**deal, 'pot_bb': pot, 'bet_bb': pot * ratio,
                           'estimated_opponent_fold_percent': 100 * probability}
                    if variant != 'numeric_cards':
                        obs['your_hand'] = 'three of a kind; beats Seat 1' if strong else 'unpaired high card; loses to Seat 1'
                    if variant == 'payoffs_label':
                        obs['expected_net_bb'] = graded('check', probability, strong, pot, pot * ratio)['action_values_bb']
                    system = RULES + (RULE if variant == 'rule_label' else '')
                    action, error = self.ask(system, obs, 'action', ['check', 'bet'])
                    action = 'check' if error else action
                    self.record(key, 'exam', {'id': key, 'group': group, 'seed': seed, 'split': split,
                        'variant': variant, 'strong': strong, 'true_fold_probability': probability,
                        'system': system, 'visible': obs, 'action': action, 'error': error,
                        **graded(action, probability, strong, pot, pot * ratio)})
            if group % 8 == 7:
                self.write_report()
        self.record('extra/exam/complete', 'extra_complete', {'name': 'exam'})
        self.write_report()

    def memory_study(self):
        c = self.protocol['memory_study']
        self.stage = 'across-session memory'
        for index in range(c['identities']):
            seed = c['seed_start'] + index
            condition = MEMORY_CONDITIONS[index % 4]
            rng = random.Random(seed)
            # Shared exogenous cards and potential responses across carry/reset arms.
            deals = []
            for session in range(1, c['sessions'] + 1):
                high = condition == 'stable_folder' or (condition == 'folder_to_caller' and session == 1) or (condition == 'caller_to_folder' and session > 1)
                for hand in range(1, c['hands_per_session'] + 1):
                    strong = rng.random() < c['strong_probability']
                    deals.append({'session': session, 'hand': hand, 'strong': strong,
                                  'deal': cards(rng, strong), 'p': .8 if high else .2, 'uniform': rng.random()})
            arms = c['arms'][index % 2:] + c['arms'][:index % 2]
            for arm in arms:
                prefix = f'memory/{seed}/{arm}'
                if self.done(prefix + '/complete'):
                    continue
                notes, estimate, recent = '', 50., []
                for offset, deal in enumerate(deals):
                    self.budget_check()
                    h, session = deal['hand'], deal['session']
                    if h == 1 and arm == 'reset':
                        notes, estimate, recent = '', 50., []
                    key = f'{prefix}/{session}/{h:03}'
                    row = self.done(key)
                    if row is None:
                        self.current = key
                        action = best_action(estimate / 100, deal['strong'])
                        response = ('fold' if deal['uniform'] < deal['p'] else 'call') if action == 'bet' else None
                        observed = {'session': session, 'hand': h, 'your_action': action, 'opponent_response': response}
                        row = {'seed': seed, 'condition': condition, 'arm': arm, **deal, 'estimate': estimate,
                               'action': action, 'observed': observed, 'split': 'heldout',
                               **graded(action, deal['p'], deal['strong'])}
                        self.record(key, 'memory_hand', row)
                    recent.append(row['observed'])
                    # Carry needs a final notebook at session boundaries; reset does not.
                    needs_update = h % self.protocol['note_every'] == 0 and offset < len(deals) - 1
                    if h == c['hands_per_session'] and arm == 'reset':
                        needs_update = False
                    if needs_update:
                        note_key = key + '/note'
                        checkpoint = self.done(note_key)
                        if checkpoint is None:
                            updated, note_error = self.note(notes, recent[-self.protocol['note_every']:])
                            value, raw, read_error = self.read({'notes': updated}, estimate)
                            checkpoint = {'seed': seed, 'arm': arm, 'session': session, 'hand': h,
                                          'notes': updated, 'estimate': value, 'raw_read': raw,
                                          'note_error': note_error, 'read_error': read_error}
                            self.record(note_key, 'memory_note', checkpoint)
                        notes, estimate = checkpoint['notes'], checkpoint['estimate']
                self.record(prefix + '/complete', 'memory_episode', {'seed': seed, 'arm': arm})
            self.write_report()
        self.record('extra/memory/complete', 'extra_complete', {'name': 'memory'})

    def code_baselines(self):
        c = self.protocol['code_baselines']
        self.stage = 'code-only memory policies'
        for index in range(c['seeds']):
            seed = c['seed_start'] + index
            rng = random.Random(seed)
            switch = rng.randint(*self.protocol['switch_range'])
            before, after = [(.2, .8), (.8, .2), (.2, .2), (.8, .8)][index % 4]
            tape = [{'hand': h, 'strong': rng.random() < .25, 'uniform': rng.random(),
                     'p': before if h <= switch else after} for h in range(1, self.protocol['hands'] + 1)]
            for policy in c['policies']:
                key = f'baseline/{seed}/{policy}'
                if self.done(key):
                    continue
                self.budget_check()
                history, trajectory = [], []
                for deal in tape:
                    estimate = policy_estimate(history, policy, c['decay_per_observed_bet'])
                    action = best_action(estimate / 100, deal['strong'])
                    response = ('fold' if deal['uniform'] < deal['p'] else 'call') if action == 'bet' else None
                    history.append({'your_action': action, 'opponent_response': response})
                    trajectory.append({**deal, 'estimate': estimate, 'action': action, 'response': response,
                                       **graded(action, deal['p'], deal['strong'])})
                self.record(key, 'baseline_game', {'seed': seed, 'condition_index': index % 4,
                    'switch': switch, 'policy': policy, 'split': 'heldout', 'trajectory': trajectory,
                    'mean_regret_bb': statistics.mean(r['regret_bb'] for r in trajectory)})
        self.record('extra/baselines/complete', 'extra_complete', {'name': 'code_baselines'})
        self.write_report()

    def extras_report(self):
        kinds = ['exam', 'memory_hand', 'memory_note', 'memory_episode', 'baseline_game', 'extra_complete']
        counts = tuple(self.db.execute('select kind,count(*) from records where kind in (?,?,?,?,?,?) group by kind order by kind', kinds))
        if getattr(self, '_extra_counts', None) == counts:
            return self._extra_analysis
        exam = self.records('exam')
        memory = self.records('memory_hand')
        completed = {(r['seed'], r['arm']) for r in self.records('memory_episode')}
        c = self.protocol['memory_study']
        seeds = [s for s in range(c['seed_start'], c['seed_start'] + c['identities']) if all((s, a) in completed for a in c['arms'])]
        paired_memory = []
        by_condition = defaultdict(list)
        for seed in seeds:
            means = {a: statistics.mean(r['regret_bb'] for r in memory if r['seed'] == seed and r['arm'] == a and r['session'] > 1) for a in c['arms']}
            saving = means['reset'] - means['carry']
            paired_memory.append(saving)
            by_condition[MEMORY_CONDITIONS[(seed-c['seed_start']) % 4]].append(saving)
        baseline = self.records('baseline_game')
        policies = self.protocol['code_baselines']['policies']
        grouped = {(r['seed'], r['policy']): r['mean_regret_bb'] for r in baseline}
        complete_baselines = [s for s in sorted({s for s, _ in grouped}) if all((s, p) in grouped for p in policies)]
        exam_report = {}
        exam_groups = defaultdict(dict)
        for row in exam:
            exam_groups[row['split'], row['group']][row['variant'], row['strong']] = row
        for split in ['training_candidate', 'heldout']:
            exam_report[split] = {}
            for variant in self.protocol['exam']['variants']:
                rows = [r for r in exam if r['split'] == split and r['variant'] == variant]
                exam_report[split][variant] = {'questions': len(rows), 'errors': sum(bool(r['error']) for r in rows),
                    'zero_regret_fraction': statistics.mean(r['regret_bb'] < 1e-9 for r in rows) if rows else None,
                    'mean_regret_bb': statistics.mean(r['regret_bb'] for r in rows) if rows else None}
            complete_groups = [group for (part, _), group in exam_groups.items() if part == split and
                               all((v, strong) in group for v in self.protocol['exam']['variants'] for strong in (False, True))]
            pairs = [('numeric_label', 'rule_label'), ('numeric_label', 'payoffs_label'), ('numeric_cards', 'numeric_label')]
            exam_report[split]['paired_groups'] = {'complete_groups': len(complete_groups),
                'comparisons': {f'{a}_minus_{b}': interval([
                    statistics.mean(group[a, strong]['regret_bb'] - group[b, strong]['regret_bb'] for strong in (False, True))
                    for group in complete_groups]) for a, b in pairs}}
        exam_config = self.protocol['exam']
        result = {'exam': exam_report,
            'exam_questions_saved': len(exam),
            'exam_questions_planned': len(exam_config['pots'])*len(exam_config['bet_ratios'])*len(exam_config['threshold_margins'])*2*len(exam_config['variants']),
            'memory': {'complete_identities': len(seeds), 'planned_identities': c['identities'],
            'reset_minus_carry_bb': interval(paired_memory), 'by_condition': {k: interval(v) for k, v in by_condition.items()},
            'note_failures': sum(bool(r['note_error']) for r in self.records('memory_note')),
            'read_failures': sum(bool(r['read_error']) for r in self.records('memory_note'))},
            'code_baselines': {'complete_seeds': len(complete_baselines), 'planned_seeds': self.protocol['code_baselines']['seeds'],
                'last20_minus_policy_bb': {p: interval([grouped[s, 'last20'] - grouped[s, p] for s in complete_baselines]) for p in policies if p != 'last20'}},
            'completed_modules': [r['name'] for r in self.records('extra_complete')],
            'caveat': 'Extras are descriptive. Memory intervals resample identities; code intervals resample paired seeds. Exam formats of the same situation are not independent replications. No training performed.'}
        for kind in kinds:
            atomic(self.output / (kind + '.json'), self.records(kind))
        atomic(self.output / 'training-candidates.json', [r for r in exam if r['split'] == 'training_candidate'])
        atomic(self.output / 'extra-analysis.json', result)
        self._extra_counts, self._extra_analysis = counts, result
        return result
