# Step 140 before Session 002

Decision: keep step 140 as a research baseline. Start the next experiment with better card labels and a harder price benchmark. The four-game pilot does not establish stronger overall play.

## What happened on the training Mac

| Test | Original | Step 140 |
| --- | ---: | ---: |
| Losing hand, 30% folds, quarter-pot then pot-sized bet | Bet / check | Bet / check |
| Correct price switches, ten other fold rates | 0/10 | 3/10 |
| Correct checks on recorded caller situations | 2/11 | 11/11 |
| Correct hand comparisons, eight deliberately chosen examples | 4/8 | 3/8 |
| Mean expected loss per decision in four paired 16-hand games | 1.113 bb | 0.931 bb |

These are exploratory local comparisons. Each model saw the same system instructions and decoding settings (greedy, JSON schema, seed 14001). Scale 0 and scale 1 used the same IQ4_NL server and the verified Session 001 adapter. All answers parsed. No paid inference or new training was used. The separate historical 91,104 supervised positions were normalized within examples; the trainer’s batch weight sum of 32 was not a token count.

**Bluff pricing.** At 30%, bluffing 2.5 into 10 earns 1.25 bb over checking. Bluffing 10 loses 4 bb. Both models switched correctly. Step 140 also solved three of ten new fold-rate pairs on one losing hand. Each pair holds the pot, hand and fold rate fixed and changes the price. The original probe's 6/12 cards-only score was two of four settings repeated at three scales.

**Caller replay.** We took the first eligible decision from each distinct historical abrupt-switch model game in which a folder became a caller, more than 40 hands had passed since the switch, the hand was losing, and the model's supplied fold estimate was below 50%. Eleven seeds qualified. Their recorded observations went to both models with the table-format hand hint. This isolates action choice given the old read. A different decision in a live game could change the future evidence.

**Hand reading.** The original got four of eight right. Step 140 missed bottom pair, middle pair, a straight, an overpair and a flush. Those five outcomes all agree with the faulty board-rank shortcut. The other three are controls on which the shortcut also gives the right answer. Thus step 140 matched the shortcut on all eight. This is evidence of a serious failure mode; behavior on eight chosen examples cannot identify its internal algorithm.

**Sequential pilot.** Four new paired seeds, 16 hands each, two change directions, four possible bet sizes, equal starting information. Each arm's own bets determined which calls and folds it observed. Both received the same smoothed counting algorithm over their own six most recent observed responses. Each game used the same deals and random response draws across arms. The tuned player improved on two seeds and worsened on two. The average savings of 0.181 bb per decision needs independent replication. These games tested betting with a count-based read, not unconstrained notebook writing or six-seat play. No reserved game seed was consumed.

**Read quality.** On 4,800 recorded hands in the 48 abrupt model games, mean absolute read error was 19.13 percentage points for historical Inkling, 11.07 for a smoothed last-20 count and 3.45 for a Bayesian observer. The observer knows the four possible regimes and the switch window, but receives only the same previously observed calls and folds. It updates after every observation; Inkling refreshed periodically. This is an environment-informed reference for historical reads, not a measurement of step 140’s reading ability.

## Benchmark and operating changes

- The original development split contains 42 pricing cases and the original sealed exam contains 48. All occur among the 48 training cases after normalizing bet by pot size and retaining fold rate and win/lose status. The old exam is now a regression check.
- Frozen a separate, **unscored** release challenge of 96 rates, 96 cheap/expensive bluff pairs and 96 value bets. Its 288 normalized cases have zero overlap with Session 001 training. Within both winning and losing hands, half have a hole-card rank on the board. The pricing label comes from expected chip values; the opponent has exactly top pair. The challenge is synthetic and still assumes a supplied read. Uncompressed SHA-256: `e8030e66f721667a2ab43f77a866fbb18d4449bb7c7c0890c22ecb355e81a5b2`. Reserve it from model selection, including Session 002.
- The 48 historical reserved game seeds measure pot-size bluffs only. Use fresh variable-price games as a distinct release gate. Three training runs per method and the new guardrails are specified in the [Session 002 protocol](../../docs/inkling-session-002.md).
- Copied the deployable GGUF and PEFT weights/config to `/Volumes/FastSSD/inkling-backups/session-001-step-140` and verified their three SHA-256 hashes. Removed automatic expiration from the private Tinker **training state** at step 140 and confirmed `expires_at: null`. The portable adapters and resumable training state serve different purposes.

## Reproduce

From the repository root, run the local launcher and then `python3 -B experiments/inkling_transfer_audit.py`. The recorded-hand replay needs the original overnight hand log. Run `python3 -B experiments/summarize_inkling_audit.py` for scores or `python3 -B experiments/inkling_read_benchmark.py` for the historical read comparison. Use `python3 -B experiments/inkling_split_audit.py TRAIN.jsonl.gz CHALLENGE.jsonl.gz --release` against both training and development. This checks normalized cases, fold rates and shared boards. Session 001 train and development passed against the new challenge.

[Predictions and visible prompts](predictions.json) · [Scores per game](scores.json) · [Historical read benchmark](read-benchmark.json) · [Frozen challenge](size-challenge.jsonl.gz)
