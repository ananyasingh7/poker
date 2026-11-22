"""Save shareable local benchmark data. Raw provider replies stay in runs/."""
import json
from pathlib import Path
import sys


def read(path):
    return json.loads(path.read_text())


def trace(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def means(rows):
    final = [row for row in rows if row['phase'] == 2]
    return {arm: sum(row['scores'][arm]['mean_loss_bb'] for row in final) / len(final)
            for arm in final[0]['scores']}


def export(root, smoke, journal, output):
    assert read(root / 'complete.json')['complete']
    output.mkdir(parents=True, exist_ok=True)
    data = {'settings': read(root / 'settings.json'), 'table_checks': {}}
    events = [r for path in root.rglob('*trace*.jsonl') for r in trace(path)]
    calls = [r['reply'] for r in events if r['kind'] == 'inference']
    failures = [r for path in root.rglob('*trace*.jsonl') for r in trace(path)
                if r['kind'] == 'notebook_failure']
    assert sum(reply['choices'][0]['finish_reason'] != 'stop' for reply in calls) == len(failures)
    data['inference_summary'] = {
        'requests': len(calls),
        'reused_from_interrupted_run': sum(r.get('replayed', False) for r in events),
        'failed_notebook_updates': len(failures),
        'completion_tokens': sum(reply['usage']['completion_tokens'] for reply in calls),
        'nonempty_reasoning_replies': sum(bool(reply['choices'][0]['message'].get('reasoning_content'))
                                         for reply in calls)}
    for name, directory in [('smoke', smoke), ('journal', journal)]:
        summary = read(directory / 'summary.json')
        assert summary['completed_hands'] == summary['requested_hands']
        data['table_checks'][name] = {
            'config': read(directory / 'manifest.json'), 'summary': summary,
            'notebook_updates': [r for r in trace(directory / 'model-private.jsonl')
                                 if r['kind'] == 'notebook_update']}
    river = trace(root / 'river-switch/trace.jsonl')
    data['river_switch'] = {'checkpoints': read(root / 'river-switch/results.json'),
                            'hands': [r for r in river if r['kind'] == 'hand'],
                            'notebook_updates': [r['reply']['choices'][0]['message']['content']
                                                 for r in river if r['kind'] == 'inference'
                                                 and 'notes' in r['request']['response_format']['json_schema']['schema']['properties']],
                            'excerpt': read(root / 'excerpt.json')}
    for name in ['read-then-bet', 'payoff-controls', 'reader-controls',
                 'reader-prompt-controls', 'numeric-read-then-bet']:
        entry = {'design': read(root / name / 'design.json'),
                 'results': read(root / name / 'results.json')}
        if name == 'read-then-bet':
            rows = trace(root / name / 'trace.jsonl')
            entry['observed_hands'] = [r for r in rows if r['kind'] == 'tape']
            entry['notebook_updates'] = [r['reply']['choices'][0]['message']['content']
                                         for r in rows if r['kind'] == 'inference'
                                         and 'notes' in r['request']['response_format']['json_schema']['schema']['properties']]
        data[name] = entry
    data['numeric_reader'] = read(root / 'numeric-reader.json')
    compatibility = Path('runs/inkling-smoke-strict-v1/summary.json')
    if compatibility.exists():
        data['compatibility_without_prefill'] = read(compatibility)
    diagnostic = Path('runs/inkling-preflight-v1/budget-2048.json')
    if diagnostic.exists():
        sample = read(diagnostic)
        data['legacy_grammar_diagnostic'] = {
            'request': sample['request'],
            'answer': sample['reply']['choices'][0]['message']['content'],
            'finish_reason': sample['reply']['choices'][0]['finish_reason'],
            'observed_fold_percent': 100 * 2 / 24,
            'caveat': 'This output schema excludes the observed percentage. It does not identify Qwen\'s intended answer.'}
    data['caveats'] = [
        'Small descriptive pilots, not a model ranking or evidence of general learning.',
        'Legacy five-point percentage string buckets can distort estimates; numeric replay corrects the encoding.',
        'Qwen used Q4_K_M plus Pi LoRA, context 16384. Inkling used IQ4_NL, context 8192 and native JSON answer prefill.',
        'Qwen numeric replay is unavailable in this comparison. Do not rank reader accuracy using the legacy outputs.']
    data['caveats'].append('The drill initially stopped on a truncated 256-token notebook update. The continuation replays saved requests and keeps the previous notebook on note truncation, matching the table behavior. This policy was added after inspecting the failure.')
    (output / 'suite-v1.json').write_text(json.dumps(data, indent=2) + '\n')
    qwen = read(Path('results/9-local-pi/read-then-bet-v1.json'))
    qwen_payoff = read(Path('results/9-local-pi/read-then-bet-controls-v1.json'))['pi-read-then-bet-controls-v1']['results']
    comparison = {
        'metric': 'Mean bb lost per probe decision; lower is better.',
        'read_then_bet_legacy': {'qwen': qwen['post_phase_2_mean_loss_bb'],
                                'inkling': means(data['read-then-bet']['results'])},
        'inkling_numeric_replay': means(data['numeric-read-then-bet']['results']),
        'payoff_controls': {}, 'river_switch': {}, 'caveats': data['caveats']}
    for model, rows in [('qwen', qwen_payoff), ('inkling', data['payoff-controls']['results'])]:
        comparison['payoff_controls'][model] = {
            variant: {'optimal': sum(r['loss_bb'] == 0 for r in rows if r['variant'] == variant),
                      'decisions': sum(r['variant'] == variant for r in rows),
                      'mean_loss_bb': sum(r['loss_bb'] for r in rows if r['variant'] == variant) / 8}
            for variant in ['numeric', 'plain_language', 'calculated_payoffs']}
    for model, rows in [('qwen', read(Path('results/9-local-pi/river-switch-v1.json'))['checkpoints']),
                        ('inkling', data['river_switch']['checkpoints'])]:
        comparison['river_switch'][model] = [r for r in rows if r['phase'] == 2]
    (output / 'comparison-v1.json').write_text(json.dumps(comparison, indent=2) + '\n')
    print(json.dumps(comparison, indent=2))


if __name__ == '__main__':
    export(*(Path(arg) for arg in sys.argv[1:]))
