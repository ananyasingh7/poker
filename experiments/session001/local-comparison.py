import json,sys,socket,time,subprocess
from urllib.request import Request,urlopen
from urllib.error import URLError
from pathlib import Path
p=Path(__file__).resolve().parent
adapter=Path(sys.argv[1]).resolve()
server=Path('/Volumes/FastSSD/inkling-pi-test/runtime/build/bin/llama-server')
model=Path('/Volumes/FastSSD/inkling-pi-test/models/Inkling-2-Mini-IQ4_NL.gguf')
with socket.socket() as sock:
    sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
url=f'http://127.0.0.1:{port}'
with (adapter.parent/'local-comparison.log').open('w') as log:
    proc=subprocess.Popen([str(server),'-m',str(model),'--lora',str(adapter),'--host','127.0.0.1','--port',str(port),'--ctx-size','2048','--parallel','1','--gpu-layers','all','--jinja','--reasoning','off','--chat-template-kwargs','{"reasoning_effort":"none"}'],stdout=log,stderr=log)
    try:
        for _ in range(150):
            if proc.poll() is not None:raise RuntimeError(f'Local server exited with {proc.returncode}')
            try:
                if json.load(urlopen(url+'/health',timeout=2))['status']=='ok':break
            except (URLError,TimeoutError): pass
            time.sleep(1)
        else: raise TimeoutError('Local server warm-up')
        dev=[json.loads(l) for l in Path('runs/inkling-sleep-v1/dev.jsonl').read_text().splitlines()]
        # Prespecified balanced cross-section: 2 of 12 dev pots, 4 sizes,
        # 3 scenarios per size, 4 formats. Use identical prompts on both arms.
        from random import Random
        from score_inkling_tune import score
        plain={r['group']:json.loads(r['messages'][1]['content']) for r in dev if r['format']=='plain'}
        pots=Random(14001).sample(sorted({v['pot_bb'] for v in plain.values()}),2)
        selected=[q for q in dev if plain[q['group']]['pot_bb'] in pots]
        adapters=json.load(urlopen(url+'/lora-adapters',timeout=5))
        assert len(adapters)==1 and Path(adapters[0]['path']).resolve()==adapter and adapters[0]['id']==0
        result={'pots':pots,'questions':len(selected),'model':str(model),'registered_adapters':adapters,
                'seed':14001,'temperature':0,'comparison_scales':{'original':0.0,'adapter':1.0}}
        from size_transfer import make_pairs,score_pairs
        transfer=make_pairs(dev)
        (adapter.parent/'size-transfer-questions.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in transfer))
        for name,scale in [('original',0.0),('adapter',1.0)]:
            out=[]
            with (adapter.parent/f'local-{name}-dev.jsonl').open('w') as log_answers:
                for q in selected+transfer:
                    opts=['win','lose'] if q['format']=='hand_read' else ['check','bet']
                    body=dict(lora=[{'id':0,'scale':scale}],messages=q['messages']+[{'role':'assistant','content':'{'}],
                              seed=14001,max_tokens=64,temperature=0,
                              response_format={'type':'json_schema','json_schema':{'name':'action','strict':True,
                                'schema':{'type':'object','properties':{'action':{'type':'string','enum':opts}},'required':['action'],'additionalProperties':False}}})
                    resp=json.load(urlopen(Request(url+'/v1/chat/completions',json.dumps(body).encode(),{'Content-Type':'application/json'}),timeout=90))
                    choice=resp['choices'][0]
                    try:
                        action=json.loads(choice['message']['content'])['action'] if choice['finish_reason']=='stop' else None
                    except (ValueError,KeyError,TypeError):
                        action=None
                    row=dict(group=q['group'],format=q['format'],answer=action,content=choice['message']['content'],finish_reason=choice['finish_reason'])
                    out.append(row)
                    if len(out)<=len(selected):
                        log_answers.write(json.dumps(row)+'\n');log_answers.flush()
            result[name]=score(selected,out[:len(selected)])
            challenge=[{**q,**ans} for q,ans in zip(transfer,out[len(selected):])]
            (adapter.parent/f'local-{name}-size-transfer.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in challenge))
            result[name+'_size_transfer']=score_pairs(challenge)
        (adapter.parent/'local-comparison.json').write_text(json.dumps(result,indent=2))
    finally:
        proc.terminate()
        try: proc.wait(timeout=20)
        except subprocess.TimeoutExpired: proc.kill();proc.wait()
