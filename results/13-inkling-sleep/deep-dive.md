# What Inkling learned during the November 23 river study

The worker finished on its first attempt at 03:13 Eastern after 3 h 44 min. All 96 registered game scenarios ran for 100 hands in each of three versions. There are 28,800 arm-hand records, 576 note checkpoints and 864 quiz rows. The 864 rows reuse 144 situations across formats and card variants. The local request cache contains 14,627 distinct requests. Three note replies hit their length limit and retained the previous note. All action and percentage answers parsed.

## Is Inkling choosing the right bluffs?

On each of 48 losing-hand situations, the quiz supplied a fold chance below the break-even point, then one above it. The cards, pot, size and hand strength were identical within each pair. A player maximizing expected chips should check the first and bet the second. Rows below count pairs among the 48.

| Format | Checks twice | Bets twice | Correctly switches to bet | Switches the wrong way |
| --- | ---: | ---: | ---: | ---: |
| Plain question | 41 | 2 | 1 | 4 |
| Expanded table with strength | 12 | 30 | 6 | 0 |
| Expanded table with cards alone | 8 | 28 | 12 | 0 |
| Original table with strength | 7 | 32 | 9 | 0 |

This is a more diagnostic measure than total accuracy. In the expanded deals the plain and strength-labeled table formats earned 93 and 98 correct answers out of 144. Between them they changed 75 actions, fixing 40 wrong answers and introducing 35. Of these switches 64 went from checking to betting and 11 went the other way. The original table format changed 78 answers relative to plain, fixing 44 and introducing 34. The plain condition is reused, so those two contrasts are related.

| Expanded format | Losing bluffs taken / 48 | Profitable bluffs taken / 48 | Winning hands bet / 48 | Average expected bb lost |
| --- | ---: | ---: | ---: | ---: |
| Plain | 6 | 3 | 48 | 6.644 |
| Table with strength | 30 | 36 | 44 | 6.178 |
| Cards alone | 28 | 40 | 33 | 8.472 |

The table presentation changes the rules text, field names, hand description and displayed cards together. Attribution to an individual wording or card cue would require another ablation. The earlier plain pilot scored 94/144 with a differently serialized request. The same-night plain condition is the matched reference here. The quiz varies pot and bet size. Its bb losses cannot be read as cash-game win rates or directly compared in magnitude with the fixed-pot games.

## Card reading

With a direct question asking who wins against exactly top pair, Inkling got 99/144 responses right. Losing hands are asked twice with identical card-only inputs. Deduplication gives 56/96 distinct hands correct. It recognized 43/48 losers and only 13/48 winners. The winners comprised 4/28 two-pair hands and 9/20 sets. A constant “lose” answer would earn 96/144 on the row-weighted quiz. Supplying known hand strength improved average betting loss by 2.294 bb on the same expanded situations. Training and testing card recognition separately will help distinguish learning the cards from learning the bet.

## Does it adapt in games?

Whole scenarios shared the deals, switch point and random call/fold draws across the three versions. Each version could bet independently. Only a bet revealed whether the opponent called or folded. The model wrote notes and refreshed its estimate every 20 dealt hands. The frozen arm stopped updating after hand 40. The code arm used recent observed responses and a calculator.

The following reviewer-computed descriptive intervals are the 2.5th and 97.5th percentiles of 10,000 bootstrap resamples of complete paired seeds (random seed 8592). First average expected loss over the relevant hands per arm and seed, then difference arms within seed. Positive savings favor the named intervention. The post-change comparison starts after the switch marker, which is the midpoint of the gradual transition.

| Comparison | Seeds | Mean bb saved per decision | 95% interval |
| --- | ---: | ---: | ---: |
| Fresh versus frozen after abrupt switches | 24 | 1.066 | 0.626 to 1.536 |
| Same, caller becomes folder | 12 | 1.742 | 1.072 to 2.421 |
| Same, folder becomes caller | 12 | 0.390 | 0.112 to 0.739 |
| Fresh versus frozen after gradual switches | 24 | 0.068 | 0.014 to 0.136 |
| Counts plus calculator versus fresh model, all abrupt-module hands | 48 | 1.065 | 0.764 to 1.380 |
| Counts plus calculator versus fresh model, all gradual-module hands | 48 | 0.456 | 0.157 to 0.749 |

Fresh beat frozen on 19 abrupt switched seeds, tied on four and lost on one. Gradual produced ten wins, twelve ties and two losses. The abrupt games switched between 20% and 80% folding. The gradual games moved between 35% and 65% over 30 hands. Speed of change, size of change and cost of a betting error all differ. Also, a gradual transition can start before the hand-40 freeze. Stable conditions are included only in the whole-module comparisons.

Pooling available hands more than 40 hands after an abrupt switch, the fresh model estimated 29.5% folds against the new caller, whose true rate was 20%. Of 93 losing hands in this window where even its own estimate put the pot-sized bluff below break-even, it bet 72. Against the new folder, it bet on 98.3% of all late hands and lost only 0.10 bb per decision in expectation. Broad aggression is rewarded in the latter game. This recreates the earlier failure in the former game on new seeds. These are descriptive pooled opportunities, clustered within games. Neither the quiz nor the games trained weights.

## Reuse and release

The practice corpus has 7,039 deduplicated betting/card examples and 1,200 fold-counting examples. The fold-counting labels measure the observable recent fold fraction. An opponent's true current probability is latent and can differ considerably. The direct empirical-rate set was generated for future use. The overnight model's opponent estimates came from its own notes in the games.

The locked final betting/card exam has 1,908 rows (1,749 distinct prompts). Its SHA-256 matches the published exam manifest. Neither its prompts nor the 240 separate fold-count exam prompts occurred in the training files or this run's request cache, checked after normalizing JSON field order and whitespace. There are 48 unused game seeds. Repeated representations and families of related hands must stay together when calculating uncertainty. Treat the 96 played scenarios and today's diagnosed examples as development evidence hereafter.

Suggested next comparison: train one small adapter on balanced action pairs and winning/losing card examples. Balance unique card inputs in the training sampler. Use the existing development set to choose a checkpoint. Select it before opening the final exam. Score unchanged and tuned models in the identical runtime. Record paired-threshold success, profitable bluffs taken, bad bluffs skipped, value bets, and expected chips lost. Then compare the two frozen model versions on the 48 reserved seeds, with matched memory settings. Test complete six-seat hands before making a full-table claim.

Review checks included reconciling all game lengths, matching the deals and response draws across arms, independently recomputing all 28,800 expected-loss labels, and verifying source/input hashes. The initial server warm-up and subsequent exact-request cache reuse are included in wall time. No inference was needed for this review.
