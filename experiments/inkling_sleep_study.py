"""Run the bounded local river study from a frozen copy of these sources."""
import argparse
from collections import Counter, defaultdict
import fcntl
import json
from pathlib import Path
import random
import signal
import statistics
import time

from inkling_bluff_practice import SYSTEM
from inkling_overnight import RULES, Study, atomic, best_action, canonical, cards, loss
from inkling_own_choice import OwnChoice

PROTOCOL = dict(model='inkling-2-mini', max_hours=8, histories_per_condition=0,
                tokens=dict(action=64, notes=512, fold_percent=128),
                hands=100, switch_range=[42, 58], note_every=20, count_window=20,
                freeze_after=40, blocks=48, arms=['model', 'model_frozen', 'both'],
                seed_ranges={'abrupt': 1410000, 'gradual': 1420000, 'reserved_games': 1430000},
                splits='Original quiz pots define train/dev/test. Static test inputs and 48 fresh game seeds reserved for a future model comparison. All played games are evaluation only.',
                metrics='Paired format switches and expected bb lost; compare fresh and frozen by complete seed after change. Report incompatible and failed answers.')
CARD_READ = 'Compare your river hand with the described opponent. Return action as win or lose if called.'


def variants(view, original=False):
    strong = view['if_called'] == 'you win'
    pot, bet = view['pot_bb'], view['bet_bb']
    rng = random.Random(f'{pot}/{bet}/{strong}')
    deal = cards(rng, strong)
    alternate = not original and rng.random() < .5
    label = 'three of a kind; beats Seat 1' if strong else 'unpaired high card; loses to Seat 1'
    if alternate:
        ranks = [c[0] for c in deal['board']]
        if strong:
            pair = rng.sample(ranks[1:], 2)
            label = 'two pair; beats Seat 1'
        else:
            rank = rng.choice([r for r in '23456789' if r not in ranks])
            pair = [rank, rank]
            label = 'one pair below top pair; loses to Seat 1'
        used = set(deal['board'])
        hole = []
        for r in pair:
            card = rng.choice([r+s for s in 'cdhs' if r+s not in used])
            used.add(card)
            hole.append(card)
        deal['hole'] = hole
    context = {**deal, 'pot_bb': pot, 'bet_bb': bet, 'estimated_opponent_fold_percent': view['fold_percent']}
    return {'plain': (SYSTEM, view), 'table': (RULES, {**context, 'your_hand': label}),
            'cards_only': (RULES, context),
            'hand_read': (CARD_READ, {**deal, 'opponent_hand': "exactly one pair of the board's highest rank"})}


def prepare(folder, practice):
    for split in ['train', 'dev', 'test']:
        lines = []
        for i, line in enumerate((practice / f'{split}.jsonl').read_text().splitlines()):
            data = json.loads(line)
            view = json.loads(data['messages'][1]['content'])
            strong = view['if_called'] == 'you win'
            target = best_action(view['fold_percent']/100, strong, view['pot_bb'], view['bet_bb'])
            for name, (system, obs) in variants(view).items():
                answer = ('win' if strong else 'lose') if name == 'hand_read' else target
                messages = [{'role': 'system', 'content': system}, {'role': 'user', 'content': canonical(obs)}]
                if split == 'train':
                    messages.append({'role': 'assistant', 'content': canonical({'action': answer})})
                    row = {'messages': messages}
                else:
                    row = {'group': i, 'format': name, 'target': answer, 'messages': messages}
                lines.append(canonical(row))
        if split == 'train':
            lines = list(dict.fromkeys(lines))
        atomic(folder / f'{split}.jsonl', '\n'.join(lines)+'\n')
    # Separate training target: the observed recent fold rate is knowable exactly.
    # The hidden current rate in the actual games remains evaluator-only.
    seen = set()
    for split, size, seed in [('train', 1200, 1440000), ('dev', 120, 1450000), ('test', 240, 1460000)]:
        rng = random.Random(seed)
        examples = []
        while len(examples) < size:
            start, end = rng.choice([(.2,.8),(.8,.2),(.2,.2),(.8,.8)])
            gradual = rng.choice([True,False])
            history = []
            for hand in range(60):
                mix = min(1,max(0,(hand-20)/20)) if gradual else int(hand>=30)
                p = start + (end-start)*mix
                history.append(rng.choices(['check','fold','call'], [.35,.65*p,.65*(1-p)])[0])
            signature = tuple(history)
            if signature in seen:
                continue
            seen.add(signature)
            recent = [a for a in history if a != 'check'][-20:]
            target = round(100*recent.count('fold')/len(recent),1)
            messages = [{'role':'system','content':'Give the observed fold percentage among the last 20 responses to a bet, or all responses if fewer. A check gives no response. Round to one decimal and return JSON with fold_percent.'},
                        {'role':'user','content':canonical({'events_oldest_first':history})}]
            row = {'messages':messages}
            if split == 'train':
                messages.append({'role':'assistant','content':canonical({'fold_percent':target})})
            else:
                row['target'] = target
            examples.append(canonical(row))
        atomic(folder/f'reads-{split}.jsonl', '\n'.join(examples)+'\n')


class Overnight(Study):
    episode = OwnChoice.episode

    def quiz(self, rows, index, original=False):
        data = rows[index]
        view = json.loads(data['messages'][1]['content'])
        for name, (system, obs) in variants(view, original).items():
            if original and name not in ['plain','table']:
                continue
            key = f'quiz/{int(original)}/{index}/{name}'
            if self.done(key):
                continue
            self.current = key
            read = name == 'hand_read'
            answer, error = self.ask(system, obs, 'action', ['win','lose'] if read else ['check','bet'])
            target = ('win' if view['if_called']=='you win' else 'lose') if read else data['target']
            scored = answer or ('invalid' if read else 'check')
            regret = None if read else max(data['values_bb'].values()) - data['values_bb'][scored]
            self.record(key, 'quiz', dict(group=index, original=original, format=name, view=view,
                        answer=answer, scored=scored, target=target, error=error, regret_bb=regret))

    def report(self, state):
        quizzes = defaultdict(list)
        for row in self.records('quiz'):
            quizzes[row['original'],row['format']].append(row)
        lines = [f'# Local river study — {state}', '', 'Quiz results on development situations. Expected loss is in big blinds per decision.', '']
        for (original, fmt), rows in sorted(quizzes.items()):
            costs = [r['regret_bb'] for r in rows if r['regret_bb'] is not None]
            correct = sum(r['scored']==r['target'] for r in rows)
            lines.append(f'- {"Legacy deals" if original else "Expanded deals"}, {fmt}: {correct}/{len(rows)} correct, {sum(bool(r["error"]) for r in rows)} answer failures' + (f', mean loss {statistics.mean(costs):.3f} bb.' if costs else '.'))
        complete = {(r['module'],r['seed'],r['arm']) for r in self.records('episode')}
        groups = defaultdict(lambda: defaultdict(list))
        errors = defaultdict(list)
        flips = Counter()
        paired = defaultdict(dict)
        for r in self.records('quiz'):
            if r['format'] in ['table','plain']:
                paired[r['original'],r['group']][r['format']] = r['scored']
        for (legacy,_), answers in paired.items():
            if len(answers)==2:
                flips[legacy,'pairs']+=1
                flips[legacy,'flips']+=answers['table']!=answers['plain']
        for legacy in (True,False):
            lines.append(f'- {"Legacy" if legacy else "Expanded"} format changes: {flips[legacy,"flips"]}/{flips[legacy,"pairs"]} paired decisions differed.')
        for r in self.records('hand'):
            mod, seed = r['module'],r['seed']
            if all((mod,seed,a) in complete for a in PROTOCOL['arms']):
                if r['arm']=='model':
                    errors[mod,r['condition']].append(abs(r['estimate']-100*r['p']))
                if '_to_' in r['condition'] and r['hand']>r['switch']:
                    groups[mod,seed][r['arm']].append(r['regret_bb'])
        lines += ['', f'Completed {len(complete)} arm-games. Post-change savings compare fully matched switched games only.']
        for mod in ['abrupt','gradual']:
            savings=[statistics.mean(v['model_frozen'])-statistics.mean(v['model']) for (m,s),v in groups.items() if m==mod]
            if savings:
                lines.append(f'- {mod}: fresh reads saved {statistics.mean(savings):.3f} bb/decision across {len(savings)} switched games.')
        for (mod,cond),vals in sorted(errors.items()):
            lines.append(f'- {mod} {cond}: model mean absolute fold-rate error {statistics.mean(vals):.1f} percentage points.')
        lines += ['', 'Raw hands, model notes, fold estimates, grading probabilities and replies are checkpointed in state.sqlite3. Training examples and reserved exams are separate JSONL files.']
        atomic(self.output/'summary.md','\n'.join(lines)+'\n')

    def run(self, practice):
        prepare(self.directory, practice)
        starts = PROTOCOL['seed_ranges']
        atomic(self.output/'registered-seeds.json', {m:list(range(s,s+48)) for m,s in starts.items()})
        rows = [json.loads(line) for line in (practice/'dev.jsonl').read_text().splitlines()]
        self.stage = 'matched format comparison'
        self.report('running')
        for i in range(len(rows)):
            self.quiz(rows,i,True)
        for block in range(PROTOCOL['blocks']):
            self.stage = 'cards and betting'
            for i in range(block,len(rows),PROTOCOL['blocks']):
                self.quiz(rows,i)
            self.stage = 'own-choice games and opponent reads'
            for mod in ['abrupt','gradual']:
                module = dict(name=mod, seed_start=starts[mod], strong_probability=.25)
                for arm in PROTOCOL['arms']:
                    self.episode(module, starts[mod]+block,arm)
            self.report('running')

    def export(self):
        for kind in ('quiz','note','hand','episode'):
            path = self.directory / f'{kind}.jsonl'
            with path.with_suffix('.tmp').open('w') as output:
                for (row,) in self.db.execute('select payload from records where kind=? order by key',(kind,)):
                    output.write(row+'\n')
            path.with_suffix('.tmp').replace(path)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['directory','output','practice']:
        parser.add_argument(name,type=Path)
    args=parser.parse_args()
    args.directory.mkdir(parents=True,exist_ok=True)
    with (args.directory/'worker.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        study=Overnight(args.directory,args.output,PROTOCOL)
        def stop(signum,frame):
            raise KeyboardInterrupt('Time budget reached')
        signal.signal(signal.SIGALRM,stop)
        state='complete'
        try:
            remaining=study.deadline-time.time()
            if remaining<=0:
                raise KeyboardInterrupt('Budget exhausted')
            signal.setitimer(signal.ITIMER_REAL,max(.01,remaining-60))
            study.run(args.practice)
        except (KeyboardInterrupt, TimeoutError):
            state='stopped at time limit or interruption'
        except Exception as exc:
            study.progress('error',repr(exc))
            study.report('error')
            raise
        finally:
            signal.alarm(0)
        study.export()
        study.progress(state)
        study.report(state)


if __name__=='__main__':
    main()
