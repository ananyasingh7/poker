"""Convert the Session 001 Tinker Mini adapter for the text-only inkling2_loop GGUF."""
import argparse
import json
from pathlib import Path

import gguf
import numpy as np
from safetensors import safe_open


ATTENTION = {'gate_attn':'gate','wk_dv':'k','wo_ud':'output','wq_du':'q','wr_du':'rel','wv_dv':'v'}


def convert(folder, base, output):
    config=json.loads((folder/'adapter_config.json').read_text())
    rank=config['r']
    if config['peft_type']!='LORA' or config['use_rslora'] or config['use_dora'] or config['rank_pattern'] or config['alpha_pattern']:
        raise ValueError('Unsupported adapter scaling')
    if output.exists():
        raise FileExistsError(output)
    reference=gguf.GGUFReader(base)
    if reference.fields['general.architecture'].contents()!='inkling2_loop':
        raise ValueError('Expected an Inkling 2 Mini loop GGUF')
    shapes={t.name:tuple(reversed(t.shape.tolist())) for t in reference.tensors}
    temporary=output.with_suffix('.partial')
    writer=gguf.GGUFWriter(temporary,'inkling2_loop',use_temp_file=True)
    writer.add_type(gguf.GGUFType.ADAPTER)
    writer.add_string(gguf.Keys.Adapter.TYPE,'lora')
    writer.add_float32(gguf.Keys.Adapter.LORA_ALPHA,float(config['lora_alpha']))
    with safe_open(str(folder/'adapter_model.safetensors'),framework='pt') as weights:
        unused=set(weights.keys())
        def pair(name):
            result=[]
            for side in ('A','B'):
                key=f'language_model.{name}.lora_{side}.weight'
                unused.remove(key)
                result.append(weights.get_tensor(key).float().numpy())
            return result
        def write(name,a,b):
            shape=shapes[name+'.weight']
            if a.shape[:-2]!=shape[:-2] or b.shape[:-2]!=shape[:-2] or (b.shape[-2],a.shape[-1])!=shape[-2:] or a.shape[-2]!=b.shape[-1]:
                raise ValueError(f'Mismatched adapter for {name}: {a.shape}, {b.shape} versus {shape}')
            writer.add_tensor(name+'.weight.lora_a',np.ascontiguousarray(a,dtype=np.float16))
            writer.add_tensor(name+'.weight.lora_b',np.ascontiguousarray(b,dtype=np.float16))
        for layer in range(24):
            prefix=f'layers.{layer}'
            for src,dst in ATTENTION.items():
                write(f'blk.{layer}.attn_{dst}',*pair(f'{prefix}.attn.{src}'))
            ma,mb=pair(f'{prefix}.mlp.experts.w2')
            experts=ma.shape[0]
            write(f'blk.{layer}.ffn_down_exps',ma,np.broadcast_to(mb,(experts,*mb.shape[-2:])))
            ga,gb=pair(f'{prefix}.mlp.experts.w1')
            ua,ub=pair(f'{prefix}.mlp.experts.w3')
            # The GGUF stores gate and up outputs consecutively. A block-diagonal
            # B applies their two independent updates. llama.cpp scales by
            # alpha/(2*rank) for this fused pair, hence the factor of two.
            a=np.concatenate([ga,ua],axis=1)
            a=np.broadcast_to(a,(experts,*a.shape[-2:]))
            width=gb.shape[1]
            b=np.zeros((experts,width*2,rank*2),dtype=np.float32)
            b[:,:width,:rank]=2*gb
            b[:,width:,rank:]=2*ub
            write(f'blk.{layer}.ffn_gate_up_exps',a,b)
        write('output',*pair('lm_head'))
        if unused:
            raise ValueError(f'Unhandled weights: {sorted(unused)[:3]}')
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()
    temporary.replace(output)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('adapter','base','output'):
        p.add_argument(key,type=Path)
    a=p.parse_args()
    convert(a.adapter,a.base,a.output)
