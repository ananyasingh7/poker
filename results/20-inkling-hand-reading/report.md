# Session 004 · Can Inkling read its hand?

All three copies practiced the same 640 pairs of hands. In each pair, changing one card flipped who would win against an opponent holding exactly top pair. Their only training answer was “win” or “lose.” Then each copy played the same 32 new boards as step 140, choosing a bet directly or after a separate call to read its cards.

| Copy | Correct reads / 64 | Both hands read correctly / 32 boards | Direct loss | Read-then-act loss |
| --- | ---: | ---: | ---: | ---: |
| Step 140 | 36 | 4 | 5.43 | 5.89 |
| 204001 | 39 | 9 | 5.43 | 8.05 |
| 204002 | 44 | 15 | 5.93 | 7.86 |
| 204003 | 44 | 12 | 6.03 | 7.68 |

Loss is expected big blinds given up per betting decision. Lower is better. Betting every hand costs 2.90 on this drill. Each board produces four decisions. The two bet sizes share the same cards.

| Copy and policy | Winning bets taken / 64 | Profitable bluffs taken / 32 | Unprofitable bluffs taken / 32 |
| --- | ---: | ---: | ---: |
| Step 140, direct | 53 | 30 | 24 |
| Step 140, read then act | 52 | 28 | 21 |
| 204001, direct | 56 | 30 | 25 |
| 204001, read then act | 51 | 26 | 21 |
| 204002, direct | 54 | 30 | 25 |
| 204002, read then act | 48 | 24 | 14 |
| 204003, direct | 53 | 30 | 24 |
| 204003, read then act | 47 | 23 | 12 |

## A hand worth replaying

Inkling held J♦ Q♥ on 2♣ K♦ A♦ 10♦ 5♥. It can make 10-J-Q-K-A. That straight beats the opponent’s pair of aces. Step 140 and the middle copy both answered “lose.” Across the 16 winning straights without a flush, step 140 recognized one. The three trained copies recognized two, one and one respectively. This exploratory breakdown uses the same development hands. It suggests a specific next diagnostic: show the ranks in order and ask which five make a straight, then test whether that read changes the bet on new boards.

## Did the training save chips?

The middle copy by read-then-act loss was 204002. Positive savings favor the new copy. The ranges below resample the 32 whole boards together with their matched decisions.

- Compared with step 140 playing direct, expected savings per decision were -2.42 big blinds (95% interval -6.24 to 0.23).
- Compared with step 140 playing read then act, expected savings per decision were -1.97 big blinds (95% interval -5.94 to 0.83).

The representative fell short of the predeclared rule for advancing to a larger experiment. Step 140 remains the table model.

Three shuffles test sensitivity to the order of the same practice hands. They share a development set. These scores describe the final river decision against a specified opponent hand and fold rate. Live table play and the sealed exam were outside this experiment.

[Protocol and reproduction commands](protocol.md) · [Complete scores](results.json) · [Step 140 baseline](baseline.json)

All three final adapters were saved as external PEFT weights on Tinker’s free model. The local GGUF conversions each passed 289 checks, and copies of the weights and run records on the external SSD were verified by SHA-256. [Adapter identifiers and verification records](weights.json)
