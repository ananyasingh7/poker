# Can Inkling recognize a winner and act on it?

Start each of three copies from Session 001 step 140. Teach only the card read. Each board appears twice and changing one hole card flips the answer against exactly one pair of the board's highest rank. Half the pairs share a rank with the board on both deals and half on neither. The model sees 1,280 labeled examples over 40 batches of 32. Only training order changes across seeds 204001–204003. Use the existing rank-16 free Tinker recipe and its learning-rate schedule.

Freeze 32 separate development boards before running inference. Use 64 hands and 128 betting decisions, crossing both hands with a cheap and an expensive bluff price. Each board and fold rate is new relative to the reference files in the generator. The controlled drill supplies the opponent's hand class and fold chance.

* **Direct play.** Show the cards and price and ask the model to bet or check.
* **Read then act.** First ask whether its cards win. Feed its own answer and the price into a second call to the same adapter. Grade the action against the actual cards, including confidently wrong reads. A malformed output invalidates that run's comparison.
* **Scoring.** Measure expected big blinds forgone per decision. Zero means the action with the highest expected payoff was chosen in every situation. Resample whole boards for paired 95% bootstrap intervals, keeping all four actions together.
* **Selection.** Rank the three seeds by read-then-act mean loss, breaking ties by seed, and report all three. For the median copy to advance to further development, its reads must improve, and the lower bounds on its chip savings must exceed zero against both step 140 read-then-act and step 140 direct. It must take at least as many winning bets and make no more expensive bluffs than step 140 direct. It must also give up fewer chips than always betting. Report direct results for every copy. Keep the incumbent at the table. An advance here earns a larger independent test.

Save the final PEFT weights for each run with Tinker’s external export and verify the local conversion. With this storage choice, an interrupted training run restarts from its parent.

No game seeds or sealed exam are scored. This experiment measures decisions against a specified hand and fold chance. It does not establish play against a human or adaptation during a game.

## Reproduce the experiment

From the repository root, use `experiments/inkling_hand_reading.py` with these commands.

| Command | What it does |
| --- | --- |
| `prepare` | Rebuild the frozen examples. Requires the historical input files for the overlap check. |
| `preflight` | Render and count the training tokens in the local Tinker environment. |
| `baseline` | Grade the local step-140 adapter on both policies. |
| `train` | Train three separate free-tier copies from step 140, saving their final adapter weights. |
| `score` | Download and verify each final adapter, then replay both policies locally. |
| `rescore` | Recalculate the comparison from the published answer files with standard Python. |

Use `runs/inkling-training-env/bin/python -B` for training and preflight. For example, `python3 -B experiments/inkling_hand_reading.py rescore` uses the saved answers and needs no remote account or model. The local replays use the same four-bit base, adapter scale, output constraints and greedy decoding as Session 003.
