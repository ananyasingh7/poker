# Inkling Poker · Session 001

The selected poker adapter is step 140 of 219. On the 432 hosted development betting decisions, average expected chips given up fell from 6.019 to 0.000 big blinds. This measures how well each decision uses the given fold chance in a restricted heads-up river drill. Game winnings and in-game adaptation require separate tests.

The GGUF adapter is stored on the training Mac at `runs/inkling-tuning/001-river/selected-adapter/poker.gguf`. The sibling checkpoint directory contains the PEFT weights. [Try the local model](../../docs/table.md#try-inkling-session-001) · [Trainer settings](config.json) · [All candidate scores](selection.json)

## What we taught

Inkling practiced three decisions after the opponent checks the river. Bet a winning hand for value, bluff a losing hand when folds justify the risk, and check a losing hand when the bet costs too much. Each decision had three presentations, including a version requiring the model to read its own cards. Separate examples asked whether its hand beat a specified opponent holding top pair.

We trained a rank-16 LoRA on Tinker using the native Inkling renderer, an initial learning rate of 0.0001 with linear decay, and one epoch. The prepared file contained 7,039 verified examples. The installed trainer consumed 7,008 of them in 219 full batches, dropping the final 31. A separate 32-example rehearsal tested saving, reloading, and converting the adapter. The main run started from the original Inkling-2-Mini.

## Matched hosted results

| Measure | Original | Step 140 |
| --- | ---: | ---: |
| Expected big blinds given up per bet/check decision | 6.019 | 0.000 |
| Profitable bluffs taken | 60.4% | 100.0% |
| Unprofitable bluffs taken | 41.0% | 0.0% |
| Winning hands bet for value | 86.1% | 100.0% |
| Both sides of a bluff threshold correct, across formats | 33/144 | 144/144 |
| Winning hands correctly identified, unique prompts | 10/48 | 48/48 |
| Losing hands correctly identified, unique prompts | 33/48 | 48/48 |

The development set had 576 rows from 12 held-out pot sizes. Betting comprised 144 underlying scenarios in three formats. The 144 hand-reading rows had 96 distinct prompts. Each model saw the same 528 unique inputs at temperature zero, with the same answer prefix and stopping rule. Invalid betting replies were scored as checks, and invalid hand reads were scored incorrect. The original had 1 invalid betting reply and 33 invalid hand-reading rows. The selected checkpoint had 0 and 0, respectively. The hand-reading score includes improvements in returning a usable answer.

| Input format | Original loss (bb) | Tuned loss (bb) | Original threshold pairs | Tuned threshold pairs |
| --- | ---: | ---: | ---: | ---: |
| Hand strength given | 5.536 | 0.000 | 6/48 | 48/48 |
| Cards and hand description | 5.169 | 0.000 | 14/48 | 48/48 |
| Cards without hand description | 7.354 | 0.000 | 13/48 | 48/48 |

## Why this checkpoint

We evaluated every saved checkpoint. A candidate first had to preserve both the profitable-bluff rate and the value-bet rate within five percentage points of the original. Among those that passed, the predeclared rule selected the smallest average betting loss, breaking ties by correctly reversing more bluff decisions, then by earlier step. These development results informed selection. An independent release evaluation is still pending.

| Step | Avg. loss (bb) | Profitable bluffs taken | Unprofitable bluffs taken | Passed guardrails |
| ---: | ---: | ---: | ---: | --- |
| 20 | 5.394 | 13.2% | 2.1% | No |
| 40 | 3.849 | 47.9% | 16.7% | No |
| 60 | 1.850 | 72.2% | 18.1% | Yes |
| 80 | 2.004 | 58.3% | 4.9% | Yes |
| 100 | 0.231 | 86.1% | 1.4% | Yes |
| 120 | 0.156 | 93.8% | 0.0% | Yes |
| 140 | 0.000 | 100.0% | 0.0% | Yes |
| 160 | 0.019 | 97.9% | 1.4% | Yes |
| 180 | 0.009 | 100.0% | 1.4% | Yes |
| 200 | 0.000 | 100.0% | 0.0% | Yes |
| 219 | 0.000 | 100.0% | 0.0% | Yes |

## What transferred to the Mac

The selected weights were exported, converted to GGUF and loaded on the existing Inkling-2-Mini IQ4_NL base. The local paired test used the same llama.cpp server, seed, answer schema and questions for both arms. The adapter scale was 0 for the original and 1 for the tuned player. Two development pots (202, 164 bb) were sampled in advance with seed 14001. On their 96 questions, including 72 bet/check decisions, mean betting loss was 9.746 bb for the original and 0.418 bb for the adapter. These questions overlap the hosted development set. The local and hosted runtimes use different quantization and output settings.

[Local scores and serving settings](local-comparison.json) · [Local base and runtime hashes](local-identity.json) · [Adapter conversion check](conversion-verification.json) · [Export SHA-256 hashes](sha256.json)

## Can it use a new bet size?

The practice generator uses four bet-to-pot ratios and four offsets from the break-even fold chance. Of the 31 distinct fold percentages occurring on losing training hands, 30 map to a single target action. A lookup built from training labels that knows whether the hand wins and ignores both the pot and bet size answers 138 of the 144 plain development decisions correctly. A high score on that quiz alone leaves open whether Inkling learned the arithmetic.

For a separate local probe, we fixed the same losing hand, pot and fold chance and changed only the bet size. A correct player bluffs for the smaller amount and checks for the larger one. We used four new fold percentages, three fractional pot sizes, and two input formats. Everyone saw the same 12 pairs per format. This probe was specified before checkpoint selection and played no part in choosing the winner.

| Fresh size-switch pairs | Original, both actions correct | Tuned, both actions correct |
| --- | ---: | ---: |
| Hand strength given | 0/12 | 7/12 |
| Cards without hand description | 0/12 | 6/12 |

These are repeated views of 12 situations with one losing hand. The next development set should vary fold rates and bet sizes independently, with more card textures and more hands near the break-even point. Evaluate this checkpoint first, then use those errors to decide what to teach in Session 002. The 1,908-row sealed exam and 48 reserved games remain available for a frozen release candidate. Full-table performance against skilled opponents is unmeasured.

[Fold-percentage control](fold-only-control.json) · [Fresh questions](size-transfer-questions.jsonl.gz) · [Full scorecard](scorecard.json)

## Run and cost record

The selected training state is `tinker://e70a1d2c-a779-5de0-ad50-db4344636389:train:0/weights/000140` and its sampler is `tinker://e70a1d2c-a779-5de0-ad50-db4344636389:train:0/sampler_weights/000140`. Local weights are saved independently of the remote snapshot lifetime. Both training runs together used 1,230,597 training tokens, estimated at $0.9229. Including logged samples at the uncached input rate and the maximum cost of incomplete calibration batches, the estimated usage ceiling is $1.1431. Reserving the maximum output length for every submitted sampling request gives the more conservative ceiling of $1.4235. Both are token-usage estimates and exclude any unrelated account activity.

[Training manifest](manifest.json) · [Token ledger](sampling-reservations.jsonl.gz) · [Training settings and scoring protocol](../../docs/inkling-first-fine-tune.md)
