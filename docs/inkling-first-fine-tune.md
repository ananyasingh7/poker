# Inkling poker training sessions

Session 001 teaches a river decision: compare the hand with top pair, skip an unprofitable bluff, take a profitable bluff, and bet a winning hand. The same poker situation appears in several formats. Bet and hand targets are computed from the cards and chip values. The base player already updates its estimate of an opponent during a game. Training aims to improve how the player acts on that estimate.

## Prepared session 001

The versioned recipe is [session-001.json](../examples/14-inkling-finetuning/session-001.json). Its source is the expanded sleep study. The 1,200 examples for counting observed folds are a separate dataset reserved for a later experiment. The older two-action quiz and old training command are superseded by this bundle.

From the repo root:

```sh
python3 -B experiments/prepare_inkling_tune.py
```

This rebuilds or verifies runs/inkling-tuning/001-river. The manifest records the proposed parent, settings, source/split hashes, counts and shuffled training-file hash. Preparing the same session twice checks the existing bundle. A changed recipe or source needs a new session ID. Only the training file is passed to the trainer. Each betting format includes 640 each of bad bluffs, good bluffs and value bets. There are also 639 unique winning-hand and 640 losing-hand comparisons. All 7,039 answers passed independent grading against the restricted top-pair drill. Normalized training prompts are unique and disjoint from development and the locked exam.

The local preflight used Python 3.12 and tinker-cookbook 0.7.1, renderer tml_v0. The pinned environment inventory and local render check are in the session folder. At batch size 32 the installed recipe consumes 219 full batches, or 7,008 examples, per epoch. Its shuffled tail of 31 examples is skipped. The render check found 91,104 supervised token positions in those batches and a maximum example length of 232. The first pilot uses one epoch, rank 16 and learning rate 0.0001.

## Training and scoring sequence

1. Record the served base/version and renderer, effort, output constraints, failure fallback and decoding settings. Collect its development predictions on the intended evaluation runtime. The archived local IQ4_NL scores give a historical reference. A new hosted baseline provides the matched comparison for a hosted adapter.
2. After authorization for paid training, configure TINKER_API_KEY in the training environment. Do a small train, save and reload check in a separate log directory. Confirm that the exported adapter works with the intended serving path. Hosted and local outputs may differ.
3. Launch the pilot from the repo root after that check. This is a paid command and transfers synthetic training records to Tinker.

```sh
runs/inkling-training-env/bin/python -m tinker_cookbook.recipes.chat_sl.train \
  model_name=thinkingmachines/Inkling-2-Mini renderer_name=tml_v0 \
  dataset=runs/inkling-tuning/001-river/train.jsonl \
  log_path=runs/inkling-tuning/001-river/trainer \
  num_epochs=1 batch_size=32 lora_rank=16 learning_rate=0.0001 \
  max_length=1024 save_every=20
```

4. Evaluate saved candidate checkpoints on the 576 development rows. Each prediction record carries the question's group, format and decoded answer. A model-format failure uses a null answer and counts toward failures. Retry infrastructure interruptions separately. Retain the raw replies, model/checkpoint ID and serving settings with each set of predictions. The scorer rejects missing, duplicate and extra predictions. Example using the archived local responses:

```sh
python3 -B experiments/score_inkling_tune.py \
  runs/inkling-sleep-v1/dev.jsonl \
  runs/inkling-tuning/001-river/historical-base-dev.jsonl \
  runs/inkling-tuning/001-river/historical-base-dev.json
```

5. Primary development score is expected bb lost per betting decision, weighting the three betting formats equally. Track correctly reversed bluff pairs by format, profitable and bad bluffs taken, value bets, unique winning/losing hand accuracy, and failures. Select by the predeclared recipe. Apply the five-percentage-point guardrails to the equal-format average versus the matched original, and inspect each individual format for regressions. Preserve the parent and record rejected candidates too.

## Many sessions, one credible comparison

Give every new experiment a new config, ID, one-line hypothesis and a primary skill to improve. Record the base model revision and, for a continuation, the exact loadable parent checkpoint as a separate field and trainer input. Update source membership, hashes, settings, steps, costs, selected checkpoint, scorecard and promotion decision. Start from the same base when comparing two teaching methods. Continue from the strongest validated checkpoint when adding a skill. Keep examples of earlier skills in the training mix and repeat their evaluations.

| Session | Trigger and change | Evidence needed |
| --- | --- | --- |
| 001 river fundamentals | Card comparisons and paired bluff thresholds | Fewer chips lost while preserving good bluffs and value bets. |
| Next targeted refinement | Pick the largest costly development failure. Change one thing, such as representation balance, two-pair coverage, or difficult sizing thresholds. | Compare the candidate with both its parent and the original on the same old and fresh development situations. |
| Opponent reads and memory | Use observed responses if inaccurate reads become a limiting factor. Distinguish recent observed frequency from a hidden current fold probability. | Calibration on fresh histories and better own-choice play, alongside retained river decision scores. |
| Broader poker | Add more river actions and sizings, then turn play and full six-seat hands when teachers and grades are reliable. | Fresh opponent families and full-game results for claims about table strength. |

Use development and new development challenges for iterations. For a release candidate, lock the checkpoint and serving settings before opening the 1,908-row final exam and 48 reserved game seeds. Proposed release criteria are at least 10% lower mean betting loss versus the matched original, at most a five-point drop in each of the good-bluff and value-bet rates, and positive mean game savings with a paired whole-game 95% interval above zero. Keep invalid outputs and format-specific results visible. If a final result informs a refinement, treat that benchmark as historical and reserve a fresh exam and fresh games for the next independent release claim. A passing river release earns a full-table trial.

[Hosted training recipe](https://tinker-docs.thinkingmachines.ai/cookbook/recipes/chat-sl/) · [Inkling renderer](https://tinker-docs.thinkingmachines.ai/cookbook/inkling/) · [Completed study](../results/13-inkling-sleep/deep-dive.md)
