"""Reserve only complete question groups with no train/dev prompt overlap."""
import argparse
import hashlib
import json
from pathlib import Path

from inkling_overnight import atomic, canonical


def lock_exam(folder):
    used = set()
    for split in ['train', 'dev']:
        for line in (folder / f'{split}.jsonl').read_text().splitlines():
            used.add(canonical(json.loads(line)['messages'][:2]))
    rows = [json.loads(line) for line in (folder/'test.jsonl').read_text().splitlines()]
    # Remove all related questions at the same pot and size, not just the
    # individual card-read prompt that happened to repeat.
    spots = {r['group']: json.loads(r['messages'][1]['content']) for r in rows if r['format']=='plain'}
    def group(row):
        spot = spots[row['group']]
        return spot['pot_bb'], spot['bet_bb']
    omit = {group(r) for r in rows if canonical(r['messages'][:2]) in used}
    clean = [r for r in rows if group(r) not in omit]
    content = ''.join(canonical(r)+'\n' for r in clean)
    atomic(folder/'final-exam.jsonl',content)
    return {'use_for_final_comparison':'final-exam.jsonl', 'questions':len(clean),
            'excluded_pot_size_groups':len(omit), 'matched_train_or_dev_prompts_remaining':0,
            'sha256':hashlib.sha256(content.encode()).hexdigest()}


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('folder',type=Path)
    p.add_argument('manifest',type=Path)
    args=p.parse_args()
    atomic(args.manifest,lock_exam(args.folder))
