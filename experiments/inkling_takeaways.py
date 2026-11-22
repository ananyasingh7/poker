"""Write a plain-language morning briefing from the overnight study's saved data."""
import argparse
import json
from pathlib import Path
import statistics
import time


def describe(result):
    if result['mean'] is None:
        return 'not enough completed histories yet'
    text = f'{result["mean"]:.3f} bb per decision'
    if result['interval_95']:
        text += f' (95% interval {result["interval_95"][0]:.3f} to {result["interval_95"][1]:.3f})'
    return text


def clear_gain(result):
    return result['interval_95'] is not None and result['interval_95'][0] > 0


def render(data, directory):
    complete = data['state'] == 'complete'
    primary = data['primary_fresh_vs_frozen']
    directions = data['directions']
    math_fix = data['interventions']['fixed_calculator']
    memory_fix = data['interventions']['exact_recent_counts']
    combined = data['interventions']['counts_and_calculator']
    verdict = ('The study stopped before completion. The saved results are provisional.'
               if data['state'] == 'incomplete' else 'The study is still running. These are provisional results.')
    if complete:
        if clear_gain(primary) and all(v['mean'] is not None and v['mean'] > 0 for v in directions.values()):
            verdict = 'Inkling passed our predeclared adaptation test in this controlled river drill.'
        else:
            verdict = 'Inkling did not pass our predeclared test for reliable adaptation.'
    lines = ['# What we learned about Inkling', '', f'**Status: {data["state"]}.** Updated {data["updated"]}.', '',
             verdict, '', f'Finished {data["complete_histories"]}/{data["planned_histories"]} independent histories and '
             f'{data["completed_online_pairs"]}/{data["planned_online_pairs"]} paired own-choice games.', '',
             '## Which change should we try next?', '']
    if data.get('error'):
        lines += ['Run error: ' + data['error'], '']
    if not complete:
        lines += ['The final recommendation waits until the whole study finishes.', '']
    if math_fix['mean'] is not None:
        if clear_gain(math_fix) and clear_gain(memory_fix):
            first = 'a fixed betting calculator' if math_fix['mean'] >= memory_fix['mean'] else 'better memory based on exact recent counts'
            lines += [f'**Start with {first}.** Both fixes reduced mistakes in these comparisons. Test their combination in new own-choice games before treating the gain as established in ordinary play.', '']
        elif clear_gain(math_fix):
            lines += ['**Start with a fixed betting calculator.** It improved decisions using exactly the same opponent estimates. The model can keep reading the opponent while code chooses the more profitable bet.', '']
        elif clear_gain(memory_fix):
            lines += ['**Start with better memory based on exact recent counts.** It improved the information feeding the same calculator. Rewriting free-form notes appears to leave useful evidence behind in this setup.', '']
        else:
            lines += ['**The data does not yet clearly choose a fix.** The intervals still allow no gain. Replicate on new histories before making a training decision.', '']
    lines += [f'- Replacing Inkling\'s betting with a calculator saved **{describe(math_fix)}**, using the same read.',
              f'- Replacing its notes with exact recent counts saved **{describe(memory_fix)}**, using the same calculator.',
              f'- Replacing both steps saved **{describe(combined)}** compared with its separate reading-and-betting setup.', '',
              'These comparisons are descriptive. Positive savings means fewer chips given up through bad choices.', '',
              '**Fine-tuning remains untested.** We should train only if useful, repeatable mistakes remain after testing the simpler fix. The study saves correctly graded training candidates and keeps evaluation histories separate.', '',
              '## Did it use new evidence?', '',
              f'Fresh reads saved **{describe(primary)}** compared with the old read after an opponent changed.', '']
    for name, label in [('caller_to_folder', 'When a caller became a folder'), ('folder_to_caller', 'When a folder became a caller')]:
        lines.append(f'- {label}: {describe(directions[name])} saved.')
    lines += ['', 'A positive average alone does not pass our test: the interval must sit above zero and both switch directions must improve on average.', '',
              '## Did it work when choosing its own bets?', '',
              f'Separating reading from betting saved **{describe(data["online_split_advantage"])}** after a change. '
              'Each pair faced the same cards and response draws, but its choices determined what it learned.', '']
    checkpoint_path = directory / 'checkpoint.json'
    if complete and checkpoint_path.exists():
        checkpoints = json.loads(checkpoint_path.read_text())
        completed = set(data['per_history_post_change_loss'])
        rows = [r for r in checkpoints if r['hand'] > 60 and str(r['seed']) in completed]
        if rows:
            lines += ['## How accurate were the reads?', '', 'Average distance from the opponent\'s actual fold percentage; lower is better.', '']
            for name, label in [('notes', 'Inkling reading its notes'), ('raw_history', 'Inkling reading the full action history'), ('counts', 'Exact recent counts with a small smoothing adjustment')]:
                error = statistics.mean(abs(r['reads'][name] - r['true_fold_percent']) for r in rows)
                lines.append(f'- {label}: {error:.2f} percentage points.')
            lines += ['']
    lines += ['## Basic skills', '']
    for key, label in [('bet_numeric_label_True', 'Given the fold chance and hand strength'),
                       ('bet_payoffs_label_True', 'Given the calculated winnings and hand strength'),
                       ('bet_numeric_label_False', 'Given the fold chance, reading its own cards')]:
        result = data['diagnostics'].get(key)
        if result and result['n']:
            lines.append(f'- {label}: {100*result["optimal_fraction"]:.1f}% correct across {result["n"]} questions.')
    if not any(data['diagnostics'].get(key, {}).get('n', 0) for key in ['bet_numeric_label_True', 'bet_payoffs_label_True']):
        lines.append('The skill quiz has not finished yet.')
    lines += ['', f'{data["note_failures"]} note writes failed and kept the previous note. Failed responses remain in the results.', '',
              '## What this establishes', '',
              'This measures controlled river decisions, not full six-seat poker or a measured fine-tuning benefit. '
              'The main histories supply hand strength and guarantee exposure to opponent responses. '
              'The own-choice games check what happens when exposure depends on Inkling\'s bets. '
              'Intervals treat whole histories as independent; repeated test questions are not counted as independent games.', '',
              '[Detailed report](summary.md) · [Analysis data](analysis.json) · [Registered plan](protocol.json)', '']
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--watch', action='store_true')
    args = parser.parse_args()
    began = time.monotonic()
    previous = None
    while True:
        source = args.directory / 'analysis.json'
        data = json.loads(source.read_text()) if source.exists() else None
        if data and data['updated'] != previous:
            text = render(data, args.directory)
            temporary = args.directory / 'takeaways.md.tmp'
            temporary.write_text(text)
            temporary.replace(args.directory / 'takeaways.md')
            previous = data['updated']
        if not args.watch or (data and data['state'] == 'complete') or time.monotonic() - began > 11 * 3600:
            return
        time.sleep(30)


if __name__ == '__main__':
    main()
