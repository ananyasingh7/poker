# Results data

Small, readable outputs of the runs in `runs/` (which is git-ignored and private). Regenerate with the commands below; interpretation is in [docs/results.md](../docs/results.md).

| File | Made by |
| --- | --- |
| `runs.csv` | `poker-lab catalog runs > results/runs.csv`: one row per run or batch (model, thinking, help, seeds, completion, requests, reported cost) |
| `1-pilots/ceiling-*.json`, `bench-subprocess-v1.json` | `poker-lab ceiling`, `poker-lab bench` |
| `2-thinking-off/*.txt`, `3-qwen/*.txt`, `4-table/*.txt` | `poker-lab analyze runs/<topic>/<run>` (`memory-budget-v1.txt` includes learning curves) |
| `5-thinking-low/comparison.{md,json}` | `poker-lab compare runs/5-thinking-low/thinking-low-* --seat 0 --turn --save ...` |
| `6-lineup/comparison.{md,json}` | `poker-lab compare` over `runs/6-lineup/` (fresh-* and lineup-* batches) `--seat 0 --turn --save ...` |
| `7-glm-varied/comparison.{md,json}` | `poker-lab compare runs/7-glm-varied/{none,facts,oracle,stats-sharp,stats-hint,facts-stats-sharp,notes-every5,both-every5,deepseek-anchor-none}-v1 --seat 0 --turn --save ...` |
| `8-glm-learning/notes-vs-oracle-v1.txt` | `poker-lab analyze runs/8-glm-learning/notes-vs-oracle-v1 --turn` (the pre-registered test, results §8) |
| `7-glm-varied/ceiling-{varied,fixed}-v1.json` | `poker-lab ceiling current 8 40 OUT [--varied] --check-seeds 21-40 --check-seat 0` |
| [9-local-pi/river-switch-v1.json](9-local-pi/river-switch-v1.json) | Two river-switch sessions, checkpoint notes, probe actions and scores, plus the exploratory excerpt test |
| [9-local-pi/river-switch-hands-v1.json](9-local-pi/river-switch-hands-v1.json) | The 32 synthetic river hands and all eight notebook updates, in play order |
| [9-local-pi/read-then-bet-v1.json](9-local-pi/read-then-bet-v1.json) | Four controlled histories, saved design, predictions, decisions, 96 observed bets and 32 notebook updates |
| [9-local-pi/read-then-bet-controls-v1.json](9-local-pi/read-then-bet-controls-v1.json) | Exploratory probability, payoff and reader-prompt controls |
| [10-inkling/suite-v1.json](10-inkling/suite-v1.json) | Inkling table checks, switch drills, notebooks, fixed observation histories and corrected numeric reader replay |
| [10-inkling/comparison-v1.json](10-inkling/comparison-v1.json) | Qwen/Inkling scores with probability-grammar and runtime caveats |
| [11-inkling-overnight/takeaways.md](11-inkling-overnight/takeaways.md), [summary.md](11-inkling-overnight/summary.md), [analysis.json](11-inkling-overnight/analysis.json) | Completed overnight briefing, full study and uncertainty estimates |
| [11-inkling-overnight/protocol.json](11-inkling-overnight/protocol.json), [registered-seeds.json](11-inkling-overnight/registered-seeds.json) | Plan and independent histories fixed before the main run |
| `11-inkling-overnight/{tape,checkpoint,controlled_probe}.json` | Observed responses, notebooks, estimates, choices and exact grades from 96 histories; comparison rows are not independent games |
| `11-inkling-overnight/{online_hand,online_note}.json` | Decisions and notes from the 24 paired own-choice games |
| `11-inkling-overnight/{count_diagnostic,bet_diagnostic,training-candidates}.json` | Skill questions, labels and training candidates; each record preserves its split |
| [11-inkling-overnight/audit-v1.json](11-inkling-overnight/audit-v1.json), [progress.json](11-inkling-overnight/progress.json) | Independent check of the reported totals and final completion status |
| [12-inkling-own-choice/takeaways.md](12-inkling-own-choice/takeaways.md), [summary.md](12-inkling-own-choice/summary.md), [analysis.json](12-inkling-own-choice/analysis.json), [extra-analysis.json](12-inkling-own-choice/extra-analysis.json) | Reviewed findings, automatic report, paired game comparisons, memory and exam results |
| `12-inkling-own-choice/{protocol,registered-seeds,extra-registration,source-manifest,completion,progress}.json` | Original plan, seed/split registration, executed source hashes and completion |
| `12-inkling-own-choice/hand.json.gz`, `baseline_game.json.gz` | Losslessly compressed decisions and code-only trajectories, with sizes and SHA-256 hashes in `data-manifest.json` |
| `12-inkling-own-choice/{note,episode,memory_hand,memory_note,memory_episode,exam,training-candidates}.json` | Notebooks, finished games, cross-session decisions and skill questions; only exam development groups enter training candidates |
| [12-inkling-own-choice/audit.json](12-inkling-own-choice/audit.json) | Independent grading and matching checks, failure accounting and the exam split caveat |

Full table histories and private model traces stay under `runs/`.

Export a completed local suite with `python3 -B experiments/export_local_suite.py SUITE_DIR SMOKE_DIR JOURNAL_DIR results/10-inkling`. The legacy five-point reader grammar can distort percentages; its outputs do not establish reader accuracy. The numeric replay allows ordinary percentages.

The overnight supervisor is `experiments/run_inkling_overnight.py RUN_DIR RESULT_DIR`. It keeps the Mac awake and resumes from the request cache after bounded retries. The final export includes observations, notes, reads, decisions, exact grades and separate training candidates. Raw replies and the recovery database stay in `runs/`. The report labels incomplete runs and does not claim a measured fine-tuning benefit.

The own-choice study uses `experiments/run_inkling_own_choice.py`; without `--execute` it only prints the plan. The original executed code and protocol are preserved by the source manifest. The protocol's status describes preparation; `progress.json` and `completion.json` record the finished run. The automatic `summary.md` is supplemented by the reviewed `takeaways.md` and independent audit. Uncompressed originals and raw requests remain under ignored `runs/`.

Read the published large datasets with Python's `json.load(gzip.open(path, 'rt'))`. Compression changes no records or split assignments. The exam split is imbalanced; its development and evaluation accuracies should not be treated as a clean generalization comparison. This study did not train a model.
