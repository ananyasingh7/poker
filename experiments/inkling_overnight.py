"""Resumable, local-only Inkling study. See examples/11-inkling-overnight/protocol.json.

Run: python3 -B experiments/inkling_overnight.py RUN_DIR RESULT_DIR
Resume with the same command. Each provider response is committed before use.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
from pathlib import Path
import random
import sqlite3
import statistics
import time
import traceback
from urllib.error import HTTPError, URLError
from urllib.request import ProxyHandler, Request, build_opener


ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = ROOT / 'examples/11-inkling-overnight/protocol.json'
RANKS = '23456789TJQKA'
SUITS = 'cdhs'
CONDITIONS = [('caller_to_folder', .2, .8), ('folder_to_caller', .8, .2),
              ('stable_caller', .2, .2), ('stable_folder', .8, .8)]
READ = ('Estimate Seat 1\'s CURRENT probability of folding to the next pot-sized river bet. '
        'Use only the supplied evidence. Calls and folds are responses to bets; a showdown '
        'after a check is not a response to a bet. Return fold_percent as a numeric percentage '
        'from 0 to 100. Consider whether recent evidence differs from older evidence.')
NOTE = ('Maintain a scouting notebook about the same opponent, Seat 1. Use only the previous '
        'notebook and listed completed hands. Record observed calls and folds, counts and '
        'when they occurred. Distinguish recent evidence from old evidence. A showdown '
        'after a check is not a call or fold. Keep observations only, no playing advice. '
        'Do not invent observations. At most 80 words. Return JSON with notes.')
RULES = ('Heads-up hold\'em river drill. You are Seat 0; Seat 1 has checked. '
         'The observation gives the pot and the only allowed bet size in bb. You may check '
         'or bet that amount. Seat 1 always holds exactly one pair of the board\'s highest '
         'rank, with no straight or flush. It only calls or folds to a bet. '
         'Maximize expected chips. Opponent fold chance is independent of your cards. '
         'Return JSON with action, either check or bet.')


def utc():
    return datetime.now(timezone.utc).isoformat()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def atomic(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + '.tmp')
    tmp.write_text(value if isinstance(value, str) else json.dumps(value, indent=2, allow_nan=False) + '\n')
    tmp.replace(path)


def straight(ranks):
    values = set(ranks)
    if 14 in values:
        values.add(1)
    return any(set(range(low, low + 5)) <= values for low in range(1, 11))


def cards(rng, strong, high=None):
    """Unpaired board, no four-to-a-straight; neither player can have a flush.

    Strong is a set below the board's highest rank. Air has seven distinct
    ranks without a straight. Opponent top pair is the known comparison hand.
    """
    high = high or rng.choice([10, 11])
    while True:
        board_ranks = sorted([high] + rng.sample(list(range(2, high)), 4), reverse=True)
        wheel = set(board_ranks) | ({1} if high == 14 else set())
        if all(len(wheel & set(range(low, low + 5))) < 4 for low in range(1, 11)):
            break
    suits = rng.sample(list(SUITS), 4)
    board = [RANKS[r - 2] + suits[i % 4] for i, r in enumerate(board_ranks)]
    if strong:
        index = rng.choice([1, 2, 3, 4])
        rank = board_ranks[index]
        free = [s for s in SUITS if RANKS[rank - 2] + s not in board]
        hole = [RANKS[rank - 2] + s for s in rng.sample(free, 2)]
    else:
        while True:
            ranks = rng.sample([r for r in range(2, 15) if r not in board_ranks], 2)
            if not straight(board_ranks + ranks):
                break
        hole = [RANKS[r - 2] + s for r, s in zip(ranks, rng.sample(list(SUITS), 2))]
    assert len(set(board + hole)) == 7
    return {'board': board, 'hole': hole}


def payoff(action, p, strong, pot=10.0, bet=10.0):
    # Net includes half the existing pot already contributed by hero.
    check = pot / 2 if strong else -pot / 2
    called = check + bet if strong else check - bet
    return check if action == 'check' else p * pot / 2 + (1 - p) * called


def best_action(p, strong, pot=10.0, bet=10.0):
    return max(['check', 'bet'], key=lambda a: (payoff(a, p, strong, pot, bet), a == 'bet'))


def loss(action, p, strong, pot=10.0, bet=10.0):
    return max(0.0, payoff(best_action(p, strong, pot, bet), p, strong, pot, bet)
               - payoff(action, p, strong, pot, bet))


def percent(value):
    # Canonical numeric formatting, never enum buckets. Preserve raw reads separately.
    return float(f'{value:.1f}')


def interval(values, repetitions=4000):
    if not values:
        return {'n': 0, 'mean': None, 'interval_95': None}
    if len(values) < 2:
        return {'n': len(values), 'mean': statistics.mean(values), 'interval_95': None}
    rng = random.Random(1100999)
    boot = sorted(statistics.mean(rng.choices(values, k=len(values))) for _ in range(repetitions))
    return {'n': len(values), 'mean': statistics.mean(values),
            'interval_95': [boot[int(.025 * repetitions)], boot[int(.975 * repetitions)]]}


class Study:
    def __init__(self, directory, output, protocol=None):
        self.directory, self.output = Path(directory), Path(output)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.output.mkdir(parents=True, exist_ok=True)
        self.protocol = protocol or json.loads(PROTOCOL.read_text())
        self.http = build_opener(ProxyHandler({}))
        self.db = sqlite3.connect(self.directory / 'state.sqlite3')
        self.db.execute('pragma journal_mode=WAL')
        self.db.execute('pragma synchronous=FULL')
        self.db.execute('create table if not exists requests (key text primary key, body text, reply text, value text, error text, seconds real, created text)')
        self.db.execute('create table if not exists records (key text primary key, kind text, payload text)')
        self.db.commit()
        locked = self.directory / 'protocol.json'
        if locked.exists():
            if json.loads(locked.read_text()) != self.protocol:
                self.db.close()
                raise AssertionError('Protocol changed; use a new run directory')
        else:
            atomic(locked, self.protocol)
        atomic(self.output / 'protocol.json', self.protocol)
        atomic(self.output / 'registered-seeds.json', [
            {'seed': 1140000 + block * 4 + i, 'condition': condition[0]}
            for block in range(self.protocol['histories_per_condition']) for i, condition in enumerate(CONDITIONS)])
        started = self.directory / 'started.json'
        if not started.exists():
            atomic(started, {'started': utc(), 'unix': time.time()})
        self.started = json.loads(started.read_text())['unix']
        self.deadline = self.started + self.protocol['max_hours'] * 3600
        self.stage = 'initializing'
        self.current = ''
        self.new_calls = 0

    def record(self, key, kind, payload):
        self.db.execute('insert or replace into records values (?,?,?)', (key, kind, canonical(payload)))
        self.db.commit()

    def records(self, kind):
        return [json.loads(r[0]) for r in self.db.execute('select payload from records where kind=? order by key', (kind,))]

    def done(self, key):
        row = self.db.execute('select payload from records where key=?', (key,)).fetchone()
        return json.loads(row[0]) if row else None

    def progress(self, state='running', error=None):
        counts = dict(self.db.execute('select kind,count(*) from records group by kind'))
        requests, failures = self.db.execute('select count(*),sum(error is not null) from requests').fetchone()
        value = {'state': state, 'updated': utc(), 'stage': self.stage, 'current': self.current,
                 'elapsed_hours': (time.time() - self.started) / 3600,
                 'requests': requests, 'failed_model_answers': failures or 0,
                 'completed': counts, 'error': error}
        atomic(self.directory / 'progress.json', value)
        atomic(self.output / 'progress.json', value)

    def ask(self, system, observation, field, options=None):
        schema = {'type': 'number', 'minimum': 0, 'maximum': 100} if field == 'fold_percent' else {'type': 'string'}
        if options:
            schema['enum'] = options
        body = {'model': 'inkling-2-mini', 'temperature': 0,
                'max_tokens': self.protocol['tokens'][field],
                'chat_template_kwargs': {'reasoning_effort': 'none'},
                'messages': [{'role': 'system', 'content': system},
                             {'role': 'user', 'content': canonical(observation)},
                             {'role': 'assistant', 'content': '{'}],
                'response_format': {'type': 'json_schema', 'json_schema': {'name': field, 'strict': True,
                    'schema': {'type': 'object', 'properties': {field: schema}, 'required': [field], 'additionalProperties': False}}}}
        encoded = canonical(body)
        key = hashlib.sha256(encoded.encode()).hexdigest()
        cached = self.db.execute('select value,error from requests where key=?', (key,)).fetchone()
        if cached:
            return json.loads(cached[0]), cached[1]
        reply = None
        began = time.monotonic()
        for attempt in range(6):
            if time.time() >= self.deadline:
                raise TimeoutError('Study time limit reached; all committed work is preserved')
            try:
                request = Request('http://127.0.0.1:9932/v1/chat/completions', encoded.encode(), {'Content-Type': 'application/json'})
                with self.http.open(request, timeout=180) as response:
                    reply = json.load(response)
                if 'choices' not in reply:
                    raise ValueError('Missing choices: ' + str(reply)[:300])
                break
            except HTTPError as exc:
                if exc.code not in [408, 429, 500, 502, 503, 504]:
                    raise
                self.progress('retrying', f'HTTP {exc.code}')
            except (URLError, TimeoutError, ConnectionError):
                self.progress('retrying', 'Local server unavailable')
            if attempt == 5:
                raise ConnectionError('Local server unavailable after six attempts')
            time.sleep(min(30, 2 ** attempt))
        assert reply['model'] == 'inkling-2-mini', 'Unexpected model alias'
        value, error = None, None
        choice = reply['choices'][0]
        try:
            if choice['finish_reason'] != 'stop':
                raise ValueError('finish_reason=' + choice['finish_reason'])
            value = json.loads(choice['message']['content'])[field]
            if field == 'fold_percent':
                if type(value) not in (int, float) or not math.isfinite(value) or not 0 <= value <= 100:
                    raise ValueError('Invalid numeric percentage')
            elif not isinstance(value, str) or (options and value not in options):
                raise ValueError('Invalid string response')
        except (ValueError, TypeError, KeyError) as exc:
            value, error = None, str(exc)
        self.db.execute('insert into requests values (?,?,?,?,?,?,?)',
                        (key, encoded, canonical(reply), canonical(value), error, time.monotonic() - began, utc()))
        self.db.commit()
        self.new_calls += 1
        self.progress()
        return value, error

    def note(self, prior, hands):
        value, error = self.ask(NOTE, {'prior_notebook': prior, 'completed_hands': hands}, 'notes')
        return (prior if error else value), error

    def read(self, evidence, previous=50.0):
        value, error = self.ask(READ, evidence, 'fold_percent')
        return (previous if error else percent(value)), value, error

    def choose(self, deal, strong, evidence, pot=10., bet=10., label=True):
        obs = {**deal, 'pot_bb': pot, 'bet_bb': bet, **evidence}
        if label:
            obs['your_hand'] = 'three of a kind; beats Seat 1' if strong else 'unpaired high card; loses to Seat 1'
        value, error = self.ask(RULES, obs, 'action', ['check', 'bet'])
        return ('check' if error else value), error, obs

    def preflight(self):
        self.stage = 'preflight'
        if self.done('preflight'):
            return
        for endpoint in ['props', 'v1/models']:
            with self.http.open('http://127.0.0.1:9932/' + endpoint, timeout=30) as response:
                atomic(self.directory / (endpoint.replace('/', '-') + '.json'), json.load(response))
        count_prompt = 'Return the observed fold percentage: folds divided by total bets, times 100. Numeric JSON only.'
        low, e1 = self.ask(count_prompt, {'calls': 22, 'folds': 2}, 'fold_percent')
        high, e2 = self.ask(count_prompt, {'calls': 2, 'folds': 22}, 'fold_percent')
        if e1 or e2 or not 5 <= low <= 15 or not 85 <= high <= 98:
            raise ValueError('Numeric preflight failed; main study not started')
        for i in range(4):
            self.note('', [{'hand': h + 1, 'your_action': 'bet', 'opponent_response': 'fold' if h % (i + 2) else 'call'} for h in range(20)])
        for strong in [False, True]:
            self.choose(cards(random.Random(1100001 + strong), strong, 14), strong,
                        {'estimated_opponent_fold_percent': 80.0})
        self.record('preflight', 'preflight', {'finished': utc(), 'numeric_low': low, 'numeric_high': high})
        self.write_report()

    def diagnostics(self):
        self.stage = 'basic skills'
        count_prompt = 'Calculate the observed fold percentage from the supplied evidence. Every response is to a bet. Return numeric fold_percent from 0 to 100.'
        index = 0
        for n, p, repeat in itertools.product([8, 24, 60, 120], [0, .05, .1, .2, .4, .5, .6, .8, .9, 1], range(2)):
            rng = random.Random(1110000 + index)
            responses = ['fold' if rng.random() < p else 'call' for _ in range(n)]
            folds = responses.count('fold')
            for variant in ['counts', 'history', 'counts_with_rate']:
                key = f'diagnostic/count/{index:03}/{variant}'
                if self.done(key):
                    continue
                self.current = key
                obs = {'folds': folds, 'calls': n - folds, 'bets_faced': n}
                if variant == 'history':
                    obs = {'responses_in_order': responses}
                if variant == 'counts_with_rate':
                    obs['observed_fold_percent'] = percent(100 * folds / n)
                value, error = self.ask(count_prompt, obs, 'fold_percent')
                self.record(key, 'count_diagnostic', {'id': key, 'split': 'development', 'variant': variant,
                            'visible': obs, 'target_percent': 100 * folds / n, 'answer': value, 'error': error})
            index += 1
        index = 0
        for pot, ratio, p, strong in itertools.product([5., 10., 20., 40.], [.25, .5, 1., 2.], [0, .2, .5, .8, 1], [False, True]):
            rng = random.Random(1120000 + index)
            # Every representation and label ablation stays in the same split.
            split = 'heldout' if (index // 2) % 4 == 0 else 'training_candidate'
            high = rng.choice([10, 11]) if split == 'heldout' else rng.choice([12, 13])
            deal = cards(rng, strong, high)
            bet = pot * ratio
            for label, variant in itertools.product([False, True], ['numeric', 'sentence', 'payoffs']):
                key = f'diagnostic/bet/{index:03}/{int(label)}/{variant}'
                if self.done(key):
                    continue
                self.current = key
                evidence = {'estimated_opponent_fold_percent': percent(100 * p)}
                if variant == 'sentence':
                    evidence = {'opponent_information': f'Seat 1 folds to {100*p:g} percent of these bets, independently of your cards.'}
                if variant == 'payoffs':
                    evidence['expected_net_bb'] = {a: payoff(a, p, strong, pot, bet) for a in ['check', 'bet']}
                action, error, obs = self.choose(deal, strong, evidence, pot, bet, label)
                self.record(key, 'bet_diagnostic', {'id': key, 'group': index // 2, 'split': split, 'variant': variant,
                            'hand_label_supplied': label, 'visible': obs, 'strong': strong, 'true_fold_probability': p,
                            'action': action, 'best_action': best_action(p, strong, pot, bet),
                            'loss_bb': loss(action, p, strong, pot, bet), 'error': error})
            index += 1
            if index % 16 == 0:
                self.write_report()

    def controlled(self):
        self.stage = 'controlled learning'
        for block in range(self.protocol['histories_per_condition']):
            order = list(enumerate(CONDITIONS))
            random.Random(1130000 + block).shuffle(order)
            for condition_id, (condition, before, after) in order:
                seed = 1140000 + block * 4 + condition_id
                prefix = f'controlled/{seed}'
                if self.done(prefix + '/complete'):
                    continue
                rng = random.Random(seed)
                tape = [{'hand': i + 1, 'your_action': 'bet',
                         'opponent_response': 'fold' if rng.random() < (before if i < 60 else after) else 'call'} for i in range(120)]
                self.record(prefix + '/tape', 'tape', {'seed': seed, 'condition': condition, 'before': before,
                            'after': after, 'split': 'heldout', 'visible_hands': tape})
                notebook, estimate, raw_estimate, frozen = '', 50., 50., 50.
                for t in range(20, 121, 20):
                    self.current = f'{prefix}/checkpoint/{t}'
                    previous = notebook
                    notebook, note_error = self.note(notebook, tape[t-20:t])
                    estimate, raw_note_read, read_error = self.read({'private_notebook': notebook}, estimate)
                    raw_estimate, raw_history_read, raw_error = self.read({'completed_hands': tape[:t]}, raw_estimate)
                    if t <= 60:
                        frozen = estimate
                    count_estimate = percent(100 * (sum(h['opponent_response'] == 'fold' for h in tape[t-20:t]) + 1) / 22)
                    p = before if t <= 60 else after
                    reads = {'notes': estimate, 'frozen': frozen, 'counts': count_estimate,
                             'raw_history': raw_estimate, 'true': percent(100 * p)}
                    self.record(self.current, 'checkpoint', {'seed': seed, 'condition': condition, 'hand': t,
                                'notebook_before': previous, 'notebook': notebook, 'reads': reads,
                                'raw_note_read': raw_note_read, 'raw_history_read': raw_history_read,
                                'true_fold_percent': 100 * p, 'note_error': note_error,
                                'read_error': read_error, 'raw_error': raw_error})
                    probe_rng = random.Random(seed * 1000 + t)
                    for i, strong in enumerate([False, True, False, True]):
                        deal = cards(probe_rng, strong)
                        contexts = {'direct_notes': {'private_notebook': notebook},
                                    'fresh_read': {'estimated_opponent_fold_percent': estimate},
                                    'frozen_read': {'estimated_opponent_fold_percent': frozen},
                                    'counts_read': {'estimated_opponent_fold_percent': count_estimate},
                                    'perfect_read': {'estimated_opponent_fold_percent': percent(100 * p)}}
                        for arm, evidence in contexts.items():
                            action, error, obs = self.choose(deal, strong, evidence)
                            self.record(f'{prefix}/probe/{t}/{i}/{arm}', 'controlled_probe', {
                                'seed': seed, 'condition': condition, 'hand': t, 'probe': i, 'arm': arm,
                                'split': 'heldout', 'visible': obs, 'strong': strong, 'true_fold_probability': p,
                                'action': action, 'loss_bb': loss(action, p, strong), 'error': error})
                        for read_name, value in reads.items():
                            action = best_action(value / 100, strong)
                            self.record(f'{prefix}/probe/{t}/{i}/calculator_{read_name}', 'controlled_probe', {
                                'seed': seed, 'condition': condition, 'hand': t, 'probe': i,
                                'arm': 'calculator_' + read_name, 'split': 'heldout',
                                'strong': strong, 'cards': deal, 'estimated_fold_percent': value,
                                'true_fold_probability': p, 'action': action,
                                'loss_bb': loss(action, p, strong), 'error': None})
                    self.progress()
                self.record(prefix + '/complete', 'controlled_complete', {'seed': seed, 'condition': condition})
                self.write_report()

    def online(self):
        self.stage = 'own-choice games'
        for episode in range(self.protocol['online_pairs']):
            seed = 1150000 + episode
            condition, before, after = CONDITIONS[episode % 2]
            for arm in ['direct_notes', 'split']:
                prefix = f'online/{seed}/{arm}'
                if self.done(prefix + '/complete'):
                    continue
                rng = random.Random(seed)
                switch = rng.randint(32, 48)
                schedule = [(cards(rng, i % 2 == 0), i % 2 == 0, rng.random()) for i in range(80)]
                notebook, estimate, completed = '', 50., []
                for i, (deal, strong, uniform) in enumerate(schedule, 1):
                    self.current = f'{prefix}/hand/{i}'
                    p = before if i <= switch else after
                    evidence = {'private_notebook': notebook} if arm == 'direct_notes' else {'estimated_opponent_fold_percent': estimate}
                    action, error, obs = self.choose(deal, strong, evidence)
                    response = ('fold' if uniform < p else 'call') if action == 'bet' else 'showdown'
                    completed.append({'hand': i, 'your_action': action, 'opponent_response': response})
                    self.record(self.current, 'online_hand', {'seed': seed, 'condition': condition,
                        'arm': arm, 'hand': i, 'switch_after': switch, 'split': 'heldout',
                        'visible': obs, 'strong': strong, 'true_fold_probability': p,
                        'action': action, 'response': response, 'loss_bb': loss(action, p, strong),
                        'estimated_fold_percent': estimate if arm == 'split' else None, 'error': error})
                    if i % 20 == 0:
                        previous = notebook
                        notebook, note_error = self.note(notebook, completed[-20:])
                        if arm == 'split':
                            estimate, raw, read_error = self.read({'private_notebook': notebook}, estimate)
                        else:
                            raw, read_error = None, None
                        self.record(f'{prefix}/notes/{i}', 'online_note', {'seed': seed, 'arm': arm, 'hand': i,
                            'previous': previous, 'notes': notebook, 'note_error': note_error,
                            'estimate': estimate if arm == 'split' else None, 'raw_read': raw, 'read_error': read_error})
                self.record(prefix + '/complete', 'online_complete', {'seed': seed, 'arm': arm})
                self.write_report()

    def analyze(self):
        complete = self.records('controlled_complete')
        completed = {r['seed'] for r in complete}
        all_probes = self.records('controlled_probe')
        probes = [r for r in all_probes if r['seed'] in completed and r['hand'] > 60]
        grouped = defaultdict(lambda: defaultdict(list))
        for r in probes:
            grouped[r['seed']][r['arm']].append(r['loss_bb'])
        per_seed = {seed: {arm: statistics.mean(v) for arm, v in arms.items()} for seed, arms in grouped.items()}
        conditions = {r['seed']: r['condition'] for r in complete}
        changed = [seed for seed in per_seed if conditions[seed] in ['caller_to_folder', 'folder_to_caller']]
        primary = interval([per_seed[s]['frozen_read'] - per_seed[s]['fresh_read'] for s in changed])
        arm_means = {arm: interval([v[arm] for v in per_seed.values()]) for arm in next(iter(per_seed.values()), {})}
        interventions = {}
        for name, old, new in [('fixed_calculator', 'fresh_read', 'calculator_notes'),
                               ('exact_recent_counts', 'calculator_notes', 'calculator_counts'),
                               ('raw_history_memory', 'calculator_notes', 'calculator_raw_history'),
                               ('counts_and_calculator', 'fresh_read', 'calculator_counts')]:
            interventions[name] = interval([v[old] - v[new] for v in per_seed.values()])
        directions = {name: interval([per_seed[s]['frozen_read'] - per_seed[s]['fresh_read']
                                     for s in changed if conditions[s] == name])
                      for name in ['caller_to_folder', 'folder_to_caller']}
        counts = self.records('count_diagnostic')
        bets = self.records('bet_diagnostic')
        diagnostics = {}
        for variant in ['counts', 'history', 'counts_with_rate']:
            rows = [r for r in counts if r['variant'] == variant]
            valid = [r for r in rows if not r['error']]
            diagnostics['reader_' + variant] = {'n': len(rows), 'failures': len(rows) - len(valid),
                'mean_absolute_percentage_error': statistics.mean(abs(r['answer'] - r['target_percent']) for r in valid) if valid else None}
        for label, variant in itertools.product([False, True], ['numeric', 'sentence', 'payoffs']):
            rows = [r for r in bets if r['variant'] == variant and r['hand_label_supplied'] == label]
            diagnostics[f'bet_{variant}_label_{label}'] = {'n': len(rows),
                'optimal_fraction': statistics.mean(r['loss_bb'] < 1e-8 and not r['error'] for r in rows) if rows else None,
                'mean_loss_bb': statistics.mean(r['loss_bb'] for r in rows) if rows else None,
                'failures': sum(bool(r['error']) for r in rows)}
        online_done = {(r['seed'], r['arm']) for r in self.records('online_complete')}
        online = [r for r in self.records('online_hand') if (r['seed'], r['arm']) in online_done]
        online_groups = defaultdict(lambda: defaultdict(list))
        for r in online:
            if r['hand'] > r['switch_after']:
                online_groups[r['seed']][r['arm']].append(r['loss_bb'])
        online_paired = [statistics.mean(v['direct_notes']) - statistics.mean(v['split'])
                         for v in online_groups.values() if set(v) == {'direct_notes', 'split'}]
        checkpoints = self.records('checkpoint')
        note_failures = sum(bool(r['note_error']) for r in checkpoints)
        note_failures += sum(bool(r['note_error']) for r in self.records('online_note'))
        return {'updated': utc(), 'complete_histories': len(completed), 'primary_fresh_vs_frozen': primary,
                'directions': directions, 'post_change_arm_losses': arm_means, 'interventions': interventions,
                'diagnostics': diagnostics, 'online_split_advantage': interval(online_paired),
                'note_failures': note_failures, 'per_history_post_change_loss': per_seed,
                'planned_histories': self.protocol['histories_per_condition'] * 4,
                'planned_online_pairs': self.protocol['online_pairs'],
                'completed_online_pairs': len(online_paired),
                'caveat': 'Intervals resample whole independent histories. Only fresh versus frozen is confirmatory; all intervention comparisons are descriptive. This is a river drill with supplied hand strength in the main experiment. No fine-tuning was performed.'}

    def write_report(self, state='running', error=None):
        data = self.analyze()
        data['state'] = state
        data['error'] = error
        atomic(self.output / 'analysis.json', data)
        def fmt(value):
            return 'pending' if value is None else f'{value:.3f}'
        def show(result):
            ci = result['interval_95']
            return fmt(result['mean']) + (f' [{fmt(ci[0])}, {fmt(ci[1])}]' if ci else ' [not enough histories]')
        primary = data['primary_fresh_vs_frozen']
        lines = ['# Inkling overnight results', '',
            f'Status: **{state}**. Updated {data["updated"]}.', '',
            f'Completed {data["complete_histories"]}/{data["planned_histories"]} controlled histories and '
            f'{data["completed_online_pairs"]}/{data["planned_online_pairs"]} paired own-choice games.', '',
            'Values below are avoidable bb lost per decision. Lower loss is better. Positive savings means the change helped.', '',
            '## Does it use new evidence?', '',
            f'Fresh reads saved **{show(primary)} bb per decision** against frozen reads after a switch. '
            'The interval is a 95% bootstrap interval over independent histories.', '']
        if state != 'complete':
            lines += ['**Provisional. The experiment is not complete; do not treat this as the final result.**', '']
        if primary['interval_95'] and data['complete_histories'] == data['planned_histories']:
            low, high = primary['interval_95']
            positive_directions = all(v['mean'] is not None and v['mean'] > 0 for v in data['directions'].values())
            conclusion = ('The predeclared learning test passed in this controlled setup.' if low > 0 and positive_directions
                          else 'The data does not meet the predeclared test for reliable adaptation.')
            lines += [conclusion, '']
        lines += ['| Switch | Savings from fresh reads |', '| --- | ---: |']
        lines += [f'| {k} | {show(v)} |' for k, v in data['directions'].items()]
        lines += ['', '## Memory, calculator or fine-tuning?', '',
                  '| Change | Chips saved per decision, with 95% interval |', '| --- | ---: |']
        lines += [f'| {name} | {show(value)} |' for name, value in data['interventions'].items()]
        lines += ['', 'These comparisons hold the other step fixed. The calculator uses the same model read; the memory comparison gives the same calculator a rate from exact recent counts.', '']
        if data['complete_histories']:
            math_effect = data['interventions']['fixed_calculator']
            memory_effect = data['interventions']['exact_recent_counts']
            def supported(v):
                return v['interval_95'] is not None and v['interval_95'][0] > 0
            if supported(math_effect) and supported(memory_effect):
                priority = 'Both the fixed calculator and exact-count memory helped. Use them together, then test on fresh histories before training.'
            elif supported(math_effect):
                priority = 'Prioritize the fixed betting calculator. It improved choices while using the same Inkling estimates.'
            elif supported(memory_effect):
                priority = 'Prioritize better memory: exact recent counts improved the estimates feeding the same calculator.'
            else:
                priority = 'Neither simple intervention has a clearly positive interval yet. Inspect the skill checks and collect a fresh replication before choosing training.'
            lines += [priority, '',
                '**Fine-tuning has not been tested here.** Residual mistakes in questions with supplied probabilities or supplied payoffs identify training candidates; they do not show that training would fix them.', '']
        lines += ['## Results by setup', '', '| Setup | Average loss per decision |', '| --- | ---: |']
        lines += [f'| {arm} | {show(value)} |' for arm, value in data['post_change_arm_losses'].items()]
        lines += ['', '## Basic skills', '', '| Check | Results |', '| --- | --- |']
        for name, value in data['diagnostics'].items():
            detail = (f'{value["n"]} questions; mean error {fmt(value["mean_absolute_percentage_error"])} percentage points'
                      if name.startswith('reader') else
                      f'{value["n"]} questions; correct {fmt(100 * value["optimal_fraction"] if value["optimal_fraction"] is not None else None)}%; mean loss {fmt(value["mean_loss_bb"])} bb')
            lines.append(f'| {name} | {detail}; {value["failures"]} failed answers |')
        lines += ['', '## Own-choice games', '',
            f'Separating the steps saved {show(data["online_split_advantage"])} bb per decision after a change. '
            'The two setups saw the same cards and random response draws, but their own bets determined what evidence they collected.', '',
            f'{data["note_failures"]} note writes failed and kept their previous note. Failed actions check; failed reads retain their previous estimate (50% initially). No model failures are silently dropped.', '',
            '## Limits', '', data['caveat'], '',
            'Exact scripted probabilities are used only for grading and explicitly labelled perfect-information controls. Probe results never enter notebooks. '
            'Main and own-choice histories are held out from training. The quiz groups keep all wording and hand-label variants in the same split.', '']
        if error:
            lines += ['## Run error', '', str(error), '']
        atomic(self.output / 'summary.md', '\n'.join(lines))
        self.progress(state, error)

    def export(self):
        for kind in ['count_diagnostic', 'bet_diagnostic', 'tape', 'checkpoint', 'controlled_probe', 'online_hand', 'online_note']:
            atomic(self.output / (kind + '.json'), self.records(kind))
        candidates = [r for r in self.records('bet_diagnostic') if r['split'] == 'training_candidate']
        atomic(self.output / 'training-candidates.json', candidates)
        atomic(self.output / 'protocol.json', self.protocol)

    def run(self):
        try:
            self.preflight()
            # Main experiment has priority. Quiz and own-choice games cannot consume its budget.
            self.controlled()
            self.diagnostics()
            self.online()
            self.stage, self.current = 'finished', ''
            self.export()
            self.write_report('complete')
            atomic(self.directory / 'complete.json', {'complete': True, 'finished': utc()})
        except BaseException as exc:
            self.export()
            self.write_report('incomplete', f'{type(exc).__name__}: {exc}')
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--report-only', action='store_true')
    parser.add_argument('--protocol', type=Path, default=PROTOCOL)
    args = parser.parse_args()
    study = Study(args.directory, args.output, json.loads(args.protocol.read_text()))
    if args.report_only:
        complete = (args.directory / 'complete.json').exists()
        study.export()
        study.write_report('complete' if complete else 'running')
    else:
        study.run()


if __name__ == '__main__':
    main()
