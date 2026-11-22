"""Replay saved notebooks with numeric probabilities instead of string buckets."""
import itertools
import json
from pathlib import Path
import random
import sys

import read_then_bet as experiment


def run(directory, source):
    directory.mkdir(parents=True, exist_ok=False)
    rows = json.loads((source / 'results.json').read_text())
    tapes = [json.loads(line) for line in (source / 'trace.jsonl').read_text().splitlines()]
    design = {'source': str(source), 'model': experiment.model.MODEL,
              'purpose': 'Correct the legacy percentage grammar; reuse original observations and notebooks.',
              'schema': {'type': 'number', 'minimum': 0, 'maximum': 100},
              'exploratory': True, 'new_notebook_updates': False}
    (directory / 'design.json').write_text(json.dumps(design, indent=2) + '\n')
    results = []
    experiment.cache = {}
    with (directory / 'trace.jsonl').open('x') as experiment.model.trace:
        for row in rows:
            suits = random.Random(row['seed']).sample('cdhs', 4)
            probes = [(experiment.model.deal(t, strong, suits), strong)
                      for t, strong in itertools.product(experiment.PROBES, (False, True))]
            observations = [h for tape in tapes if tape['kind'] == 'tape'
                            and tape['seed'] == row['seed'] and tape['phase'] <= row['phase']
                            for h in tape['hands']]
            evidence = {arm: {'private_notebook': note} for arm, note in row['notes'].items()}
            evidence['raw'] = {'completed_hands': observations}
            estimates = {arm: experiment.model.ask(experiment.READ, context, 'fold_percent', numeric=True)
                         for arm, context in evidence.items()}
            if row['phase'] == 1:
                frozen = estimates.copy()
            contexts = {**estimates, 'counts': row['estimates']['counts'],
                        **{f'frozen_{arm}': value for arm, value in frozen.items()}}
            scores = experiment.evaluate(probes, row['true_fold_percent'] / 100, {}, contexts)
            results.append({'seed': row['seed'], 'phase': row['phase'],
                            'true_fold_percent': row['true_fold_percent'],
                            'estimates': estimates, 'frozen_estimates': frozen.copy(), 'scores': scores})
            (directory / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
            print('numeric replay', row['seed'], row['phase'], estimates, flush=True)
    return results


if __name__ == '__main__':
    run(Path(sys.argv[1]), Path(sys.argv[2]))
