# What costs Inkling chips on fresh river hands?

**Train the next candidate to bet its winning hands.** Missed winning bets accounted for 86%, 62% and 83% of expected chip loss across the three Session 002 runs. All three improved their bluff-pair score over step 140. Only run 2 had lower average loss on this sample, and its paired interval includes zero. Step 140 remains the incumbent.

We asked the four existing models the same 256 questions each on the Mac. These came from 32 new situations with two matched deals apiece. For each of the 96 betting decisions we recorded the actual bet or check, whether the model could read the hand alone, and whether it could price that bet when given the correct hand strength. All 1,024 answers are available here. The simulator supplied the opponent’s chance of folding. This diagnostic used local inference and no additional training.

## Results

A bluff pair counts only when both the cheap bluff and the expensive check are correct. Loss is the expected number of big blinds given up per full decision. A positive savings interval favors the candidate. The intervals use 10,000 paired bootstrap draws over entire situations. Each adapter saw the same 32 groups, so the three runs cannot be pooled as 96 independent groups.

| Model | Hand reads | Bluff pairs, strength given | Bluff pairs, cards shown | Loss per decision ↓ | 95% interval for savings vs step 140 | Winning bets missed | Share of loss from those misses |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Step 140 | 36/64 | 12/32 | 8/32 | 4.73 | Reference | 8/32 | 51% |
| Run 1 | 45/64 | 20/32 | 18/32 | 6.16 | [-5.15, +1.64] | 8/32 | 86% |
| Run 2 | 50/64 | 17/32 | 12/32 | 4.19 | [-1.51, +2.54] | 5/32 | 62% |
| Run 3 | 51/64 | 19/32 | 18/32 | 4.82 | [-2.46, +2.10] | 7/32 | 83% |

## Where the chips went

Each cell gives the **number of wrong full decisions / total expected big blinds given up**. The column says which standalone answers were wrong on those same decisions. A model can get a step wrong and still pick the right final action. The table counts only full-decision errors. These comparisons describe behavior across different prompts and do not establish what computation occurred inside the model.

| Decision ledger | Only hand read wrong | Only price wrong | Both wrong | Both right, full action wrong |
| --- | ---: | ---: | ---: | ---: |
| [Step 140](step140-decisions.jsonl.gz) | 14 / 286.0 | 12 / 113.5 | 2 / 27.2 | 4 / 27.2 |
| [Run 1](1622101-decisions.jsonl.gz) | 6 / 252.8 | 4 / 11.8 | 2 / 27.2 | 10 / 299.1 |
| [Run 2](1622102-decisions.jsonl.gz) | 5 / 250.1 | 9 / 64.1 | 0 / 0.0 | 11 / 88.3 |
| [Run 3](1622103-decisions.jsonl.gz) | 5 / 250.1 | 7 / 37.3 | 0 / 0.0 | 9 / 175.5 |

**Two pair against aces, group 2.** Run 1 held 5♣ Q♥ on 9♣ Q♠ 5♥ 3♠ A♣ against a known pair of aces. It answered “win” on the cards and “bet” when told its hand would win. Given the full decision, it checked. The bet was 247.87 big blinds and the opponent would call 50.5% of the time. Checking gave up 125.17 big blinds of expected winnings. This was the most expensive individual mistake for the previously selected representative.

**A missed straight, group 1.** Step 140 held 6♦ A♣ on 4♣ 10♥ 7♦ 9♣ 8♦. Its six completed a ten-high straight against one pair. It judged the hand a loser and checked. It priced the bet correctly when given the winning label. That check gave up 77.26 big blinds.

## Decision for Session 003

Keep step 140 at the table. Design the next training experiment around recognizing winning hands and betting them in the full river prompt. Pair a winning hand with a losing hand at the same pot, bet size and fold rate. Grade the expected chips given up and keep separate checks on expensive bluffs, so indiscriminately betting fails. Include examples of both observed failure patterns: missing a winning hand and checking a hand that the model correctly identifies as a winner in isolation. Use new deals for training and future validation. Reserve these 32 groups for regression and keep the release exam for a candidate that clears development.

The strongest claim available here concerns river decisions against the specified one-pair opponent. Real games add uncertainty about the opponent’s cards, fold rate and future behavior. The large bets in this diagnostic give a small number of missed opportunities considerable weight.

[Evaluation protocol](protocol.md) · [Frozen questions](questions.jsonl.gz) · [Machine-readable scores](scores.json) · [Previous Session 002 report](../16-inkling-session-002/report.md)
