# Thinking with low effort, with and without the cheat sheet (2026-11-15)

Two paired batches of `deepseek/deepseek-v4.1-flash-1017` with thinking on and `reasoning_effort: low`, run to answer two questions: does the cheat sheet (`memory: oracle`) fix the leaks seen at the spectator table, and does thinking help at all? Total spend $1.08.

## Setup

| | |
| --- | --- |
| Configs | `examples/experiment-thinking-low-none.json`, `examples/experiment-thinking-low-oracle.json` |
| Runs | `runs/thinking-low-none-v1`, `runs/thinking-low-oracle-v1` (20 sessions each, all completed) |
| Deals | seeds 1-10, model in seats 0 and 3, 60 hands per session, stacks reset every hand: 1,200 hands per arm |
| Requests | `reasoning_enabled: true`, `reasoning_effort: low`, `max_tokens: 4096`, 120 s timeout, DeepInfra-first fp8 pinning |
| Cost | none $0.391 ($0.00033/hand), oracle $0.688 ($0.00057/hand) |

The no-help arm uses exactly the deals, seats and opponent order of the 28-seed thinking-off baseline (`runs/experiment-baseline-v1`, seeds 1-10, seats 0 and 3), so it also gives a matched thinking on vs off comparison.

A 12-hand smoke test first ($0.015) showed that effort `low` shortens typical reasoning (median 220-340 tokens) but does not prevent a rare degenerate loop (one decision repeated "So only two combos" 268 times up to 12,288 tokens). The batches therefore used a 4,096 cap to keep a loop cheap; that turned out to be too tight for the cheat sheet (below).

## Results

bb/100 with 95% intervals over 10 seed blocks.

| | No help | Cheat sheet | Cheat sheet minus no help (paired) |
| --- | --- | --- | --- |
| Raw | +116 [+12, +219] | +222 [+114, +331] | **+107 [+54, +159]** |
| All-in adjusted | +140 [+51, +229] | +257 [+159, +355] | **+117 [+25, +209]** |
| Call/fold mistake cost | 44 | 29 | -15.5 [-42, +11] |
| VPIP / PFR | 39% / 25% | 32% / 15% | |
| Truncations (fallbacks) | 12 (0.5%) | 130 (5.5%) | |

**The cheat sheet clearly helps a thinking model:** about +110 bb/100 on the same deals, with both intervals above zero. For comparison, the thinking-off memory experiment found oracle +33 [+2, +65] over none: knowing who the opponents are is worth much more when the model can reason about it.

### Did it fix the two station leaks?

| | No help | Cheat sheet |
| --- | --- | --- |
| Bluffs (postflop bets under 30% equity) in hands `station` stayed in | 48 of 75 (64%) | **24 of 81 (30%)** |
| River, ahead, checked to, value-bet rate when the model answered | 37% | **53%**, median size 1.0x pot (no help: 0.67x) |
| Of those river spots, truncated (fallback check) | 5 of 139 | **45 of 121 (37%)** |

- **Bluffing the station: fixed.** It halved.
- **Value-betting the station: fixed when it answers, but cut off a third of the time.** With the cheat sheet the model deliberates longest exactly when sizing a value bet against the station, and 4,096 tokens was not enough. None of the 130 cheat-sheet truncations were loops (all above 60% unique sentences); 91 were on the river, and in 60 of the 121 unopened ones the model held over 70% equity. The measured +107 is therefore probably an underestimate.

### Thinking on vs off (no help, same deals)

| | Thinking off (baseline subset) | Thinking on, effort low |
| --- | --- | --- |
| Raw bb/100 | +72 | +116 |
| All-in adjusted | +61 | +140 |
| Paired difference, raw | | +44 [-84, +172] |
| Paired difference, all-in adjusted | | **+79 [-26, +185]** |
| Aggression (postflop bets per call) | 0.39 | 6.8 |
| Call/fold mistake cost | 41 | 44 |
| Cost per hand | $0.00015 | $0.00033 |

Leaning positive but not resolved at 10 seeds. Thinking changes style far more than results: the model goes from passive (bets 2% when checked to) to aggressive, while its call/fold accuracy is unchanged. It costs about 2.1x per hand with effort low (about 19x per decision unbounded).

## What this means for settings

- The table keeps `reasoning_effort: low` with `max_tokens: 12288`. In the no-help arm the longest completed answer was 3,974 tokens, and 12 decisions truncated at 4,096; with the cheat sheet, 40 completed above 3,000 and 130 truncated. A 4,096 cap is too tight for thinking-on runs with opponent information.
- A runaway loop at 12,288 costs about $0.0036; a re-ask with thinking off after a truncation (next-steps 19b) would save the decision for about $0.00002 instead of auto-checking a value hand.
