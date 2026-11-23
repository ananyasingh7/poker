"""Compare PEFT and GGUF adapter updates on a fixed probe vector per matrix."""
import json
import sys
from pathlib import Path
import gguf
import numpy as np
from safetensors import safe_open

root = Path(sys.argv[1])
src = next(root.glob('*/adapter_model.safetensors'))
config = json.loads((src.parent / 'adapter_config.json').read_text())
dest = gguf.GGUFReader(root / 'poker.gguf')
tensors = {t.name: t.data for t in dest.tensors}
rank = config['r']
alpha = config['lora_alpha']
assert dest.fields['adapter.lora.alpha'].contents() == alpha
rng = np.random.default_rng(14001)
errors = {}
with safe_open(str(src), framework='pt') as source:
    def peft(path, expert):
        matrices = []
        for side in ('A', 'B'):
            t = source.get_slice(f'language_model.{path}.lora_{side}.weight')
            sizes = t.get_shape()
            mat = t[:] if len(sizes) == 2 else t[expert if sizes[0] != 1 else 0]
            matrices.append(mat.float().numpy())
        return matrices

    def compare(target, names, expert=None):
        a, b = [tensors[f'{target}.weight.lora_{s}'] for s in ('a','b')]
        if expert is not None:
            a, b = a[expert], b[expert]
        x = rng.standard_normal(a.shape[-1], dtype=np.float32)
        actual = (alpha / a.shape[0]) * (b.astype('f') @ (a.astype('f') @ x))
        parts = []
        for name in names:
            left, right = peft(name,expert)
            parts.append((alpha / rank) * (right @ (left @ x)))
        expected = np.concatenate(parts)
        err = np.linalg.norm(expected - actual) / max(float(np.linalg.norm(expected)), 1e-10)
        errors[f'{target}/{expert}'] = float(err)

    for layer in range(24):
        for src_name, gg_name in [('wq_du','q'),('wk_dv','k'),('wv_dv','v'),('wo_ud','output'),('gate_attn','gate'),('wr_du','rel')]:
            compare(f'blk.{layer}.attn_{gg_name}', [f'layers.{layer}.attn.{src_name}'])
        for expert in [0, 63, 127]:
            prefix = f'layers.{layer}.mlp.experts'
            compare(f'blk.{layer}.ffn_gate_up_exps', [prefix+'.w1', prefix+'.w3'],expert)
            compare(f'blk.{layer}.ffn_down_exps', [prefix+'.w2'],expert)
    compare('output',['lm_head'])
worst = max(errors,key=errors.get)
assert errors[worst]<.002,(worst,errors[worst])
report = {'checks':len(errors), 'largest_relative_error':errors[worst], 'worst_matrix':worst,
          'seed':14001, 'experts_per_layer':[0,63,127], 'layers':24}
(root / 'conversion-verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(report)
