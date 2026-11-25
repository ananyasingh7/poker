# Session 003 · Bet the winning hand

**Question.** Can Inkling collect more chips with a winning hand while still declining an expensive bluff? Session 002’s representative lost 86% of its forgone chips by checking winners on the [fresh development replay](../17-inkling-fresh-diagnosis/report.md). This session starts from Session 001 step 140.

**One example, two different right answers.** Hold the board, opponent, pot, fold chance and price still. Replace just one of Inkling’s two cards to turn a winner into a loser. At the expensive price the winner should bet and the loser should check. At the cheap price the loser can profitably bluff too. Some winners and losers match a board rank and others do not. Both hands in each group share this property.

**Data.** Each situation has two card judgments and eight betting questions: two hands × two prices × two formats (strength supplied or cards shown). Training contains 128 groups. Development contains 32 separate groups. The generator excludes boards and fold rates from the earlier training and development sets, the fresh diagnostic, and the reserved exams. Each answer is a short JSON action. Opponent strength and fold behavior follow the same synthetic river rules used in Session 002.

**Training recipe.** Three copies of the step-140 adapter, seeds 183001–183003. Same 1,280 training rows, different orders. Use rank 16, batch size 32, one epoch (40 optimizer updates), starting learning rate 0.00005 with linear decay. Evaluate the final checkpoint of every run. The predeclared representative is the median by mean full-decision loss, ties to the earliest seed. Report all three.

**Development decision.** Count expected big blinds forgone per full decision (128 per model), expected chips missed on winners, hand accuracy and both bluff prices correct. Require the representative to reduce winner and overall loss versus step 140, with a positive lower bound for overall savings in a paired 95% bootstrap over the 32 groups. Require at least the parent’s hand accuracy and bluff-pair score. On the earlier 32-group diagnostic require at least the Session 002 representative’s 18 correct bluff pairs and at most step 140’s mean loss (use the exact recorded mean). Recheck the historical 96 controls for at least 95% of profitable bluffs and value bets taken. These are development criteria. Release and full-game evaluation follow separately.

**Reproduce.** Run `python3 -B experiments/inkling_session003.py prepare` from the repository root to regenerate the splits. Use `baseline` for the local step-140 replay. Use the existing training environment’s Python and `preflight` to verify rendering and count tokens without training. The separate `train` command submits paid work. After all three final checkpoints exist, run `python3 -B experiments/score_inkling_session003.py` to export them, score every replicate and apply the historical controls to the median one.

[Manifest](manifest.json) · [Training set](train.jsonl.gz) · [Development questions](dev.jsonl.gz) · [Preflight](render-check.json)

## Step 140 before training

Step 140 completed all 320 local prompts. Told explicitly that its hand would win, it bet 64 of 64 times. Shown the cards, it bet 55 of those same 64 opportunities and checked the other nine. Those checks cost an average of 5.78 big blinds across the 64 winning decisions. Overall it gave up 4.75 big blinds per full decision and handled both bluff prices correctly in 6 of 32 groups. Giving the model the correct hand strength changes its behavior, though these prompts cannot distinguish a missed hand read from a failure to use that read.

[Full baseline scores](baseline.json) · [Step-140 answers](step140.jsonl.gz)
