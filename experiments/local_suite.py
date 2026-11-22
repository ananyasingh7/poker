"""Run the existing local pilots with POKER_MODEL / POKER_MODEL_URL overrides."""
import json
from pathlib import Path
import sys

import river_switch as model
import read_then_bet as reading
import read_then_bet_controls as controls
import read_then_bet_numeric as numeric_reading


def run(root):
    root.mkdir(parents=True, exist_ok=False)
    (root / 'settings.json').write_text(json.dumps({
        'model': model.MODEL, 'endpoint': model.URL,
        'chat_template_kwargs': model.TEMPLATE_KWARGS, 'json_prefill': model.JSON_PREFILL,
        'action_max_tokens': model.ACTION_MAX_TOKENS,
        'note_max_tokens': model.NOTE_MAX_TOKENS,
        'keep_prior_on_note_failure': model.KEEP_PRIOR_ON_NOTE_FAILURE,
        'replay_source': model.REPLAY_ROOT,
        'temperature': 0,
        'comparison': 'Original Qwen prompts, seeds, cards, and scoring. Token limits may differ.'
    }, indent=2) + '\n')
    model.run(root / 'river-switch')
    rows = json.loads((root / 'river-switch/results.json').read_text())
    notes = next(row['notes'] for row in rows if row['seed'] == 92101 and row['phase'] == 2)
    excerpt = notes.split('. ')[0] + '.'
    with (root / 'excerpt-trace.jsonl').open('x') as model.trace:
        actions = [model.action(model.deal(template, False, 'cdhs'), excerpt)
                   for template in model.PROBE]
    (root / 'excerpt.json').write_text(json.dumps({
        'seed': 92101, 'selection': 'First sentence at the same checkpoint as the Qwen follow-up.',
        'note_excerpt': excerpt, 'bluff_probe_actions': actions
    }, indent=2) + '\n')
    reading.run(root / 'read-then-bet')
    controls.run(root / 'payoff-controls')
    controls.run_read_controls(root / 'reader-controls', root / 'read-then-bet')
    controls.run_prompt_controls(root / 'reader-prompt-controls', root / 'reader-controls')
    # Verify the probability encoding without the legacy five-point string buckets.
    source = json.loads((root / 'reader-controls/design.json').read_text())
    numeric = []
    with (root / 'numeric-reader-trace.jsonl').open('x') as model.trace:
        for variant, evidence in source['cases'].items():
            answer = model.ask(reading.READ, evidence, 'fold_percent', numeric=True)
            numeric.append({'variant': variant, 'evidence': evidence, 'fold_percent': answer})
    (root / 'numeric-reader.json').write_text(json.dumps({
        'prompt': reading.READ, 'schema': {'type': 'number', 'minimum': 0, 'maximum': 100},
        'results': numeric
    }, indent=2) + '\n')
    numeric_reading.run(root / 'numeric-read-then-bet', root / 'read-then-bet')
    (root / 'complete.json').write_text(json.dumps({'complete': True}) + '\n')


if __name__ == '__main__':
    run(Path(sys.argv[1]))
