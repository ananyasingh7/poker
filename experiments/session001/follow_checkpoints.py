"""Score each periodic checkpoint as it becomes available, then rank candidates."""
import json
import subprocess
import sys
import time
from pathlib import Path

root=Path.cwd()
session=root/'runs/inkling-tuning/001-river'
hosted=session/'hosted-dev'
evaluated={}
while True:
    file=session/'trainer/checkpoints.jsonl'
    checkpoints=[json.loads(l) for l in file.read_text().splitlines()] if file.exists() else []
    for ck in checkpoints:
        path=ck.get('sampler_path')
        if not path or path in evaluated:
            continue
        label='epoch-'+ck['name']
        scores=hosted/label/'scores.json'
        if not scores.exists():
            with (session/'candidate-eval.log').open('a') as log:
                subprocess.run([sys.executable,'-B','experiments/inkling_hosted_dev.py',label,'--checkpoint',path],
                               check=True,stdout=log,stderr=log)
        evaluated[path]=(label,json.loads(scores.read_text()),ck)
    if any(c['name']=='final' for c in checkpoints):
        break
    time.sleep(8)
base=json.loads((hosted/'original/scores.json').read_text())['overall_betting']
candidates=[]
for path,(label,s,c) in evaluated.items():
    m=s['overall_betting']
    guard=all(m[k]>=base[k]-.05 for k in ['value_bet_rate','good_bluff_bet_rate'])
    pairs=sum(s[f]['pairs_correct'] for f in ['table','cards_only','plain'])
    step=219 if c['name']=='final' else int(c['name'])
    candidates.append(dict(label=label,step=step,guardrail_pass=guard,pairs_correct=pairs,**m,
                           state_path=c['state_path'],sampler_path=path))
ranked=sorted([c for c in candidates if c['guardrail_pass']], key=lambda c:(c['mean_loss_bb'],-c['pairs_correct'],c['step']))
selected=ranked[0] if ranked and ranked[0]['mean_loss_bb']<base['mean_loss_bb'] else None
(session/'selection.json').write_text(json.dumps(dict(original=base,checkpoints=candidates,selected=selected),indent=2)+'\n')
if selected:
    subprocess.run([str(Path(sys.executable).parent/'tinker'),'checkpoint','download',selected['state_path'],
                    '--output',str(session/'selected-adapter')],check=True)
