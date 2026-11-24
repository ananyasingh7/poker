# Session 002 · The price of a wrong move

**Keep Session 001 step 140 as the table model.** The representative Session 002 run correctly handled both bet sizes in 5 of 16 unfamiliar situations, up from 3. Its choices still gave up 3.66 big blinds per decision on average, compared with 3.39 for step 140. Lower is better. A big blind is the table's basic unit for measuring chips.

## What we taught

We started three copies of Inkling 2 Mini with the same Session 001 poker adapter. Each practiced the same 2,304 examples once, in a different order. Some questions asked whether the cards were winners. Others asked whether a bluff was worth its price. The last kind required both skills at once. The targets were short answers such as `bet` and `check`. We measured whether the *choices* changed. The experiment does not identify an internal calculation.

A bluff can be a good idea for a small amount and a costly mistake for a large amount. The development test reused the same opponent, cards and pot at two different prices. It gave credit for a pair only if Inkling bet at the cheaper price **and** checked at the expensive price. All 16 situations had fold rates and boards held out from training. Testing used the IQ4_NL base on the Mac, the same prompts and deterministic decoding for every adapter. The 128 questions are different views of those 16 situations.

## Local results

The last five columns use the full decision, where the model had to read the cards itself. “Loss” is the expected chips given up relative to the best legal choice under the supplied fold chance. It includes 16 cheap bluffs, 16 expensive bluffs and 16 winning hands per player.

| Model | Correct hand reads | Pairs with strength given | Pairs from cards | Bad bluffs taken ↓ | Good bluffs taken ↑ | Winning hands bet ↑ | Loss in big blinds ↓ |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Original Inkling | 22/32 | 0/16 | 0/16 | 13/16 | 12/16 | 15/16 | 4.01 |
| Session 001 step 140 | 19/32 | 5/16 | 3/16 | 12/16 | 15/16 | 14/16 | 3.39 |
| Run 1 (seed 1622101) · median | 24/32 | 10/16 | 5/16 | 9/16 | 14/16 | 14/16 | 3.66 |
| Run 2 (seed 1622102) | 23/32 | 11/16 | 8/16 | 7/16 | 15/16 | 13/16 | 3.68 |
| Run 3 (seed 1622103) | 25/32 | 11/16 | 12/16 | 3/16 | 15/16 | 13/16 | 3.52 |

The predeclared representative was run 1, the middle run by full-decision loss. Its estimated savings against step 140 were -0.27 big blinds per decision. The paired 95% bootstrap intervals for savings, resampling complete situations, were run 1 [-1.46, +0.64], run 2 [-2.14, +1.40], run 3 [-2.97, +2.35]. These intervals describe the small development set. The three training shuffles shared a starting checkpoint and questions.

## One replay that explains the score

With Q♠ 5♣ on 9♥ 4♠ 2♠ 8♣ J♠, Inkling has queen high against the opponent's described pair of jacks. The pot is 151.40 big blinds and the opponent folds 53.4% of the time. A bluff of 123.49 earns about 23.30 big blinds on average. Increasing the bluff to 223.49 makes it lose about 23.30. At the higher price, step 140 checked. All three Session 002 copies bet. This is development group 7. The table above gives every situation equal representation.

In an exploratory check, the representative answered *both* the standalone hand question and the corresponding standalone price question correctly on 31 of the 48 full decisions. When shown all the information together, it still missed 4 of those 31. We also replayed each recorded hand read through the exact bet-pricing formula, using the same supplied fold chance. This gave 2.83, 5.32 and 3.02 big blinds of average loss for runs 1, 2 and 3 respectively. A calculator can price a bluff exactly and still act on a misread hand. Before another paid run, test the costly card-reading and pricing errors on fresh development hands. These exploratory replays played no part in model selection.

## Did it forget the old drill?

The independent local replay of step 140 reproduced its 96 historical actions exactly. The candidate copies answered the same 96 questions. The table averages the three betting formats across 72 answers. The other 24 questions tested hand reading. These are familiar situations used to detect regression.

| Model | Good bluffs taken | Winning hands bet | Bad bluffs taken ↓ | Loss in big blinds ↓ |
| --- | ---: | ---: | ---: | ---: |
| Step 140 | 100.0% | 100.0% | 4.2% | 0.42 |
| Run 1 | 100.0% | 100.0% | 16.7% | 2.46 |
| Run 2 | 100.0% | 100.0% | 16.7% | 1.32 |
| Run 3 | 100.0% | 100.0% | 20.8% | 1.74 |

## Training and model files

Each run completed 72 optimizer updates and processed 362,977 token positions. The third local training process stopped after update 48. Tinker had saved its weights and optimizer state. The recipe resumed from that checkpoint at the next batch. The trainer estimated **$0.8167 total** for the three runs. All evaluation used local inference. Every exported adapter passed 289 numerical conversion checks. The three local GGUF adapters are in `runs/inkling-tuning/002-pricing/<seed>/final.gguf`. The PEFT weights are in each run's `export` directory. Verified second copies of both formats and the training logs are on FastSSD under `inkling-backups/session-002/<seed>/`. These exports can be used after the hosted training states expire.

The independent release test and reserved games remain sealed. Full-table strength, adaptation during play and performance against people are still open questions.

[Protocol and reproduction commands](protocol.md) · [Complete metrics, checkpoint identifiers and hashes](results.json) · [Research plan](../../docs/inkling-session-002.md)
