# Session 003 results

**Keep step 140 at the table.** All three copies completed 40 updates on Tinker's free training tier. Each answered 320 local development prompts on 32 groups. When given the full cards and price, two copies bet on every decision. The third checked three times.

| Model | Winning bets taken | Costly bluffs skipped | Both bluff prices right | Expected big blinds given up per full decision |
| --- | ---: | ---: | ---: | ---: |
| Step 140 | 55/64 | 8/32 | 6/32 | 4.75 |
| Run 1 | 64/64 | 0/32 | 0/32 | 2.52 |
| Run 2 | 62/64 | 1/32 | 1/32 | 2.95 |
| Run 3 | 64/64 | 0/32 | 0/32 | 2.52 |

Each group contains four full decisions. Betting is correct on three of them. An always-bet policy therefore gets 96 of 128 right and collects every winning bet. Runs 1 and 3 exactly matched that policy on this set. Run 1 represented the method under the frozen median-loss and tie rules. It saved 2.24 expected big blinds per decision against step 140 (paired 95% interval 0.70 to 4.24). It correctly switched between betting small and checking large on zero of 32 bluff pairs, below the parent's six. On the earlier fresh diagnostic it also solved zero of 32 pairs, below the required 18. The candidate failed promotion; the independent release exam remains reserved.

For a future experiment, give expensive losing bluffs more representation in training and score an always-bet policy alongside the model. Keep the evaluation mix fixed so changes in chip loss remain comparable.

[Protocol](protocol.md) · [All scores and selection checks](results.json)
