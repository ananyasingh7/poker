# Session 002 · Separate reading the cards from pricing the bet

The parent is the private Session 001 step-140 training state. Each replicate starts from those same learned weights with a fresh optimizer. The three predeclared shuffle seeds are 1622101, 1622102 and 1622103. This tests sensitivity to training order. Initialization is shared and exact hosted execution determinism is outside our control.

## Frozen design

Every new group contains one losing hand, one winning hand and a fold rate. Half the groups have a hole-card rank matching the board in both hands; the other half have no match in either. For the losing hand, changing only the bet size reverses the correct action. The amounts straddle the calculated break-even point at varying distances. Value bets protect against a policy that simply checks expensive bets.

| Task | Model sees | Grade per development group |
| --- | --- | --- |
| Hand reading | Cards and the opponent’s top-pair description | Two win/lose judgments |
| Pricing | Pot, bet, fold chance and explicit win/lose status | Both bluff prices correct; value bet separately |
| Combined | Cards, pot, bet and fold chance | Same two prices and value bet, now requiring hand reading |

Training uses 256 new groups (2,048 examples) plus 64 prior training examples from each of four formats. Development uses 16 separate groups (128 questions). This is a small development sample. Representations and related prices in the same group are dependent. The sealed release challenge remains unscored. All training targets were checked; normalized decision keys, boards and fold rates are disjoint between training, development and the frozen release challenge.

## Before training

| Local development test | Original Inkling | Step 140 |
| --- | ---: | ---: |
| Correct hand reads | 22/32 | 19/32 |
| Both bluff prices correct, hand strength supplied | 0/16 | 5/16 |
| Both bluff prices correct, cards only | 0/16 | 3/16 |

With hand strength supplied, the original checked all losing hands. Step 140 took 15 of 16 profitable bluffs, but still took 10 of 16 unprofitable ones. With cards alone it took 12 of 16 unprofitable bluffs. Each arm returned 128 valid answers. The development results establish a starting point for this refinement; the three tasks reuse 16 underlying groups.

[Raw original answers](original.jsonl.gz) · [Raw step-140 answers](step140.jsonl.gz) · [Full baseline scores](baseline-scores.json)

## Training and selection

Use one epoch, batch size 32, native tml_v0 renderer, rank 16 inherited from the parent, starting learning rate 0.00005 with linear decay, and 72 updates. Evaluate the final checkpoint for every seed. Intermediate checkpoints are for recovery. Compare the median replicate and publish all three scores; avoid choosing the luckiest shuffle. For a representative candidate use the median combined-task mean loss (tie: earlier declared seed). Require lower combined loss than the parent, no drop in hand-reading accuracy or correctly priced bluff pairs on the separate tasks, and the Session 001 good-bluff and value-bet retention checks before considering promotion. An inconclusive 16-group comparison keeps step 140 as the incumbent. The independent release and game gates are in the [research plan](../../docs/inkling-session-002.md).

## Reproduce from the repository root

1. Run `python3 -B experiments/inkling_session002.py` to recreate the splits. This requires the historical split files in runs/ for exclusion checks. The committed compressed splits suffice for training and evaluation.
2. Run `runs/inkling-training-env/bin/python -B experiments/train_inkling_session002.py` for the offline renderer check. It creates three deterministic file orders in the ignored run directory.
3. Start `scripts/serve-inkling-001.sh`, wait for its health endpoint, and run `python3 -B experiments/inkling_session002.py --baseline`. Both arms use the same IQ4_NL base, JSON constraints, temperature zero and decoding seed 14001. Adapter scales are 0 and 1.
4. After spending authorization, adding `--train` to the renderer-check command submits the three Tinker runs. Download each final adapter, verify its conversion using the Session 001 procedure, and save the GGUF as `runs/inkling-tuning/002-pricing/<seed>/final.gguf`. Load that file with the launcher's INKLING_ADAPTER override and run `experiments/inkling_session002.py --candidate-seed <seed>` with Python. Also replay the matched historical development controls before promotion.

[Manifest](manifest.json) · [Training examples](train.jsonl.gz) · [Development examples](dev.jsonl.gz) · [Token count and order hashes](render-check.json)
