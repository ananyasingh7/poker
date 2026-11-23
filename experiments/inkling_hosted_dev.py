"""Session 001 hosted development evaluation, with a persistent sampling budget."""
import argparse
import fcntl
import json
from pathlib import Path

import tinker
from tinker_cookbook import renderers
from tinker_cookbook.tokenizer_utils import get_tokenizer

from inkling_overnight import atomic, canonical
from score_inkling_tune import score

ROOT = Path(__file__).resolve().parents[1]
RUN = ROOT/'runs/inkling-tuning/001-river'
BASE = 'thinkingmachines/Inkling-2-Mini'
MAX_TOKENS = 96


def evaluate(label, path, smoke):
    output = RUN/'hosted-dev'/label
    output.mkdir(parents=True, exist_ok=True)
    model = {'model_path':path} if path else {'base_model':BASE}
    spec = dict(model=model, effort=0.9, answer_header=True, stop_at_first_message=True, temperature=0, max_tokens=MAX_TOKENS)
    if (output/'settings.json').exists() and json.loads((output/'settings.json').read_text())!=spec:
        raise ValueError('Checkpoint or settings changed')
    atomic(output/'settings.json', spec)
    questions = [json.loads(l) for l in (ROOT/'runs/inkling-sleep-v1/dev.jsonl').read_text().splitlines()]
    if smoke:
        questions = questions[:4]
    messages = {canonical(q['messages']):q['messages'] for q in questions}
    renderer = renderers.get_renderer('tml_v0',get_tokenizer(BASE))
    # Use the two native tokens introducing an assistant text answer, as in SFT.
    exemplar = [{'role':'user','content':'hello'},{'role':'assistant','content':'hello'}]
    full = renderer.build_supervised_examples(exemplar)[0][0].to_ints()
    offset = renderer.build_generation_prompt(exemplar[:1]).length
    header = full[offset:offset+2]
    assert get_tokenizer(BASE).decode(header) == '<|message_model|><|content_text|>'
    def render_prompt(question):
        tokens = renderer.build_generation_prompt(question,effort=.9).to_ints()
        return tinker.ModelInput.from_ints(tokens + header)
    tokenizer = get_tokenizer(BASE)
    example_tokens = renderer.build_supervised_examples(exemplar)[0][0].to_ints()
    end_message = next(t for t in reversed(example_tokens) if tokenizer.decode([t])=='<|end_message|>')
    stops = list(dict.fromkeys([end_message, *renderer.get_stop_sequences()]))
    cached = {}
    history = output/'replies.jsonl'
    if history.exists():
        cached = {r['key']:r for r in map(json.loads,history.read_text().splitlines())}
    client = tinker.ServiceClient(display_name='river-001-dev-'+label).create_sampling_client(**model).result()
    missing = [k for k in messages if k not in cached]
    params = tinker.SamplingParams(max_tokens=MAX_TOKENS,temperature=0,stop=stops,seed=14001)
    ledger = RUN/'sampling-reservations.jsonl'
    with history.open('a',buffering=1) as log:
        for start in range(0,len(missing),32):
            prompts = [(k,render_prompt(messages[k])) for k in missing[start:start+32]]
            upper = sum((p.length*.15+MAX_TOKENS*.5)/1e6 for _,p in prompts)
            with ledger.open('a+') as f:
                fcntl.flock(f,fcntl.LOCK_EX)
                f.seek(0)
                reserved = sum(json.loads(l)['usd_upper'] for l in f)
                if reserved+upper > .98:
                    raise RuntimeError('Sampling ceiling reached ($1.02 separately reserved for training)')
                f.write(canonical({'label':label,'requests':len(prompts),'usd_upper':upper})+'\n')
                f.flush()
            futures = [(k,p,client.sample(prompt=p,num_samples=1,sampling_params=params)) for k,p in prompts]
            for k,p,future in futures:
                seq = future.result().sequences[0]
                tokens = list(seq.tokens)
                while tokens and tokens[-1] in stops:
                    tokens.pop()
                content = tokenizer.decode(tokens).strip()
                try:
                    parsed=json.loads(content)
                    answer=parsed.get('action') if isinstance(parsed,dict) else None
                except ValueError:
                    answer=content if content in ('bet','check','win','lose') else None
                if seq.stop_reason!='stop':
                    answer=None
                row=dict(key=k,answer=answer,content=content,stop_reason=seq.stop_reason,
                         input_tokens=p.length,output_tokens=len(seq.tokens))
                log.write(canonical(row)+'\n')
                cached[k]=row
    predictions=[dict(group=q['group'],format=q['format'],answer=cached[canonical(q['messages'])]['answer']) for q in questions]
    if smoke:
        if not any(r['answer'] in ('bet','check','win','lose') for r in predictions):
            raise RuntimeError('No valid smoke answers; inspect parsing')
    else:
        atomic(output/'predictions.jsonl',''.join(canonical(r)+'\n' for r in predictions))
        atomic(output/'scores.json',score(questions,predictions))


if __name__=='__main__':
    cli=argparse.ArgumentParser(description=__doc__)
    cli.add_argument('label')
    cli.add_argument('--checkpoint')
    cli.add_argument('--smoke',action='store_true')
    args=cli.parse_args()
    evaluate(args.label,args.checkpoint,args.smoke)
