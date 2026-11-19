# Results data

Small, readable outputs of the runs in `runs/` (which is git-ignored and private). Regenerate with the commands below; interpretation is in [docs/results.md](../docs/results.md).

| File | Made by |
| --- | --- |
| `runs.csv` | `poker-lab catalog runs > results/runs.csv`: one row per run or batch (model, thinking, help, seeds, completion, requests, reported cost) |
| `1-pilots/ceiling-*.json`, `bench-subprocess-v1.json` | `poker-lab ceiling`, `poker-lab bench` |
| `2-thinking-off/*.txt`, `3-qwen/*.txt`, `4-table/*.txt` | `poker-lab analyze runs/<topic>/<run>` (`memory-budget-v1.txt` includes learning curves) |
| `5-thinking-low/comparison.{md,json}` | `poker-lab compare runs/5-thinking-low/thinking-low-* --seat 0 --turn --save ...` |
| `6-lineup/comparison.{md,json}` | `poker-lab compare` over `runs/6-lineup/` (fresh-* and lineup-* batches) `--seat 0 --turn --save ...` |
| `7-glm-varied/comparison.{md,json}` | `poker-lab compare runs/7-glm-varied/{none,facts,oracle,stats-sharp,notes-every5,deepseek-anchor-none}-v1 --seat 0 --turn --save ...` |
| `7-glm-varied/ceiling-{varied,fixed}-v1.json` | `poker-lab ceiling current 8 40 OUT [--varied] --check-seeds 21-40 --check-seat 0` |

These files hold no hole cards or hand records, only aggregates (seed numbers appear, as they do in every config).
