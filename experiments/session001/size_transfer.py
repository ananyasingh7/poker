"""Unscored-by-selection local probe. Hold cards, pot and fold probability fixed within each pair."""
import json
import random
from pathlib import Path
from inkling_overnight import loss

def make_pairs(dev):
    messages={r['format']:r['messages'] for r in dev if r['group']==0}
    pots=[73.5,90.5,119.5]
    chances=[26.4,41.2,53.1,62.4]
    rng=random.Random(14611)
    rows=[]
    for i,pot in enumerate(pots):
        for j,pct in enumerate(chances):
            p=pct/100
            breakeven=pot*p/(1-p)
            for fmt in ['plain','cards_only']:
                template=messages[fmt]
                if fmt=='plain':
                    state={'pot_bb':pot,'fold_percent':pct,'if_called':'you lose'}
                else:
                    state={'board':['Jc','8h','4s','3d','2c'],'hole':['Kd','9h'],'pot_bb':pot,'estimated_opponent_fold_percent':pct}
                for factor in [.72,1.28]:
                    bet=round(breakeven*factor,2)
                    action='bet' if p*pot-(1-p)*bet>0 else 'check'
                    prompt=json.dumps(dict(state,bet_bb=bet),sort_keys=True,separators=(',',':'))
                    rows.append({'group':f'{i}-{j}','format':fmt,'pot':pot,'bet':bet,'fold_pct':pct,
                                 'messages':[template[0],{'role':'user','content':prompt}],'target':action})
    rng.shuffle(rows)
    return rows

def score_pairs(rows):
    results={}
    for fmt in ['plain','cards_only']:
        group={}
        regrets=[]
        correct=invalid=0
        for r in rows:
            if r['format']!=fmt: continue
            right=r['answer']==r['target']
            group.setdefault(r['group'],[]).append(right)
            correct+=right
            invalid+=r['answer'] not in ('bet','check')
            regrets.append(loss(r['answer'] or 'check',r['fold_pct']/100,False,r['pot'],r['bet']))
        results[fmt]={'correct':correct,'questions':len(regrets),'pairs_correct':sum(all(v) for v in group.values()),
                      'pairs':len(group),'invalid':invalid,'mean_loss_bb':sum(regrets)/len(regrets)}
    return results
