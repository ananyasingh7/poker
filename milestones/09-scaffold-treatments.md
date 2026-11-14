# Milestone 9: Scaffold treatments

**Status:** Implemented 2026-11-13. An 11,494-hand pilot found a large effect of the first level (`features`); differences between higher levels are not resolved.

**Goal:** Measure how much of the model's play depends on computation we could do for it. Three cumulative treatments add host-computed *decision aids* to each action observation. Each level adds exactly one thing, so adjacent levels can be compared directly:

| Scaffold | Adds | Question it answers |
| --- | --- | --- |
| `none` | nothing (control) | baseline |
| `features` | position name, pot odds as required equity, stack and effective stack in big blinds, stack-to-pot ratio, raises and last raiser this street, hand notation (`AKs`), made hand, draws, whether the hole cards improve a five-card board | Does the model lose value by misreading facts it could compute itself? |
| `equity` | features + Monte Carlo showdown equity against the remaining opponents holding random hands, with a caveat that bettors hold stronger hands | Does a calculator help? |
| `advice` | equity + the action the fixed `solid` rule would take, labelled as a rule that ignores who the opponents are | Does the model follow or improve on simple advice? |

## Design decisions

- **Information boundary unchanged.** Aids are computed by the seat's own worker from its `PlayerView` (`src/scaffold.rs`), so they contain nothing the seat could not already see. Equity uses a seed derived from visible cards, so runs stay deterministic and replayable.
- **Orthogonal to memory.** `scaffold` is a separate config field; a batch's `scaffolds` list is crossed with `conditions`. Condition labels read `memory+scaffold` (for example `none+equity`, `stats+advice`), and the `none` scaffold keeps the plain memory label so earlier runs analyse unchanged.
- **Same deals in every arm.** Every treatment plays the same seeds and seat rotations, so `analyze` reports paired differences against the first condition.
- **Adherence metric.** `analyze` replays each model decision and records whether it matches the reference rule's choice (fold / check-or-call / raise). It is computed for every run, old ones included, so "agreement" in the control arm measures how close the model's unaided play already is to the rule.
- **The advice is deliberately mediocre.** `solid` wins about +109 bb/100 against the default field, while the tuned ceiling wins about +288. A model that follows the advice exactly should approach `solid`'s result; a model that beats it is adding judgement.
- **Not done:** a tool-calling version (the model asks for equity on demand). The always-present aid is the simpler, cheaper upper bound on what the tool could contribute; tool use adds a second variable (whether the model calls it).

## Done when

- [x] Aids are deterministic, cumulative, information-safe and identical across in-process and subprocess backends (unit, integration and worker tests).
- [x] Batches cross scaffolds with memory conditions; analysis labels, pairs and reports adherence per condition.
- [x] Live smoke test: four arms, zero fallbacks.
- [x] Pilot comparison with paired intervals against the control (below).
- [ ] Adjacent differences (`equity` vs `features`, `advice` vs `equity`) resolved; needs about 4× the seeds.

## Claude feedback (2026-11-13)

- The headline is a measurement-validity point as much as a poker one: the unaided baseline understates what this model can do, mostly because of how the observation is presented. Any cross-model comparison should report a `features` arm, or it partly measures parsing ability.
- I would make `features` the default observation for future memory experiments, and keep `none` as a labelled control. Otherwise memory effects are measured on a handicapped player.
- Do not over-read `advice`: the suggested policy is the untuned `solid` rule, which wins far less than the tuned ceiling (+109 vs +288 bb/100 against this field). A strong-advice arm (the tuned ceiling policy) would test whether the model can recognise good advice.
- Next cheap experiments: per-feature ablation (5 arms × 8 seeds ≈ $3), then 32 seeds on `features` vs `equity` to resolve whether a calculator helps once the facts are given.

## Running it

```sh
poker-lab batch examples/experiment-scaffold.json runs/experiment-scaffold-v1
poker-lab analyze runs/experiment-scaffold-v1
```

## Results (2026-11-13, `runs/experiment-scaffold-v1`)

8 seeds (101–108) × 6 seat rotations × 60 hands × 4 arms = 192 sessions, 11,494 hands, 25,635 requests, **$2.17**. 185 sessions clean; 2 stopped early on edge 503s absorbed by the unknown-cost allowance, 5 contain single model fallbacks. Same field as the baseline (station, overfolder, solid, adaptive, tight), stacks reset every hand.

| Arm | Model bb/100 | Paired difference vs `none` | VPIP | PFR | AF | Fold to bet | Agrees with rule |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `none` | +29 [-15, +73] | — | 38% | 18% | 0.43 | 50% | 61% |
| `features` | +149 [+50, +248] | **+120 [+37, +203]** | 45% | 24% | 0.71 | 45% | 60% |
| `equity` | +133 [+52, +214] | **+104 [+25, +183]** | 39% | 17% | 0.93 | 42% | 63% |
| `advice` | +142 [+82, +202] | **+113 [+50, +176]** | 28% | 16% | 2.25 | 31% | 78% |

Adjacent differences (computed from the same seed blocks): `equity − features` −16 [−58, +26]; `advice − equity` +9 [−57, +75]. Intervals are 95% t-intervals over 8 seed blocks.

**What it says:**

1. **Telling the model facts it could compute itself adds about +120 bb/100 here** (the lower end of the interval is still +37). The `features` effect survives a Bonferroni correction for three comparisons. The control arm (+29) is consistent with the 28-seed baseline (+56 [+25, +88]).
2. **Equity and advice add nothing measurable on top of features** in this pilot. Their point estimates are within ±20 bb/100 of `features`, and the intervals are wide.
3. **Advice changes how it plays more than how much it wins.** With a suggestion, the model agrees with the rule 78% of the time (61% unaided), folds to bets far less (31% vs 50%) and bets instead of calling (AF 2.25 vs 0.43). Yet it wins about the same as with features alone, and below `solid`'s +102 in this sample. The advice is a mediocre rule, and partly following it lands near it.
4. Pairing helps but less than hoped: seed blocks of the control and `features` arms correlate at 0.55. One block has SD ≈ 52 bb/100 in this sample.

**Not yet known:** which feature matters. The likely candidates are position names and pot odds; the model previously wrote seat numbers as if positions were fixed. An ablation (each feature alone) would answer this cheaply, about $0.55 per arm at this size.

## Qwen comparison

`qwen/qwen3.6-35b-a3b`, run through the same milestones with identical configs and seeds (`examples/qwen/`). Provider preferences: PrimeIntellect first, fp8 only, fallbacks allowed (18% of baseline requests were served by CoreWeave, also fp8). Raw runs: `runs/qwen-*`.

| Arm (8 seeds, 11,520 hands each model) | DeepSeek V4.1 Flash | Qwen3.6-35B-A3B |
| --- | --- | --- |
| `none` | +29 [-15, +73] | **-105** [-192, -18] |
| `features` | +149 [+50, +248] | +103 [+2, +204] |
| `equity` | +133 [+52, +214] | +81 [+1, +161] |
| `advice` | +142 [+82, +202] | +130 [+38, +221] |
| Paired gain of `features` over `none` | +120 [+37, +203] | **+208 [+98, +317]** |
| `equity - features` | -16 [-58, +26] | -22 [-115, +71] |
| `advice - equity` | +9 [-57, +75] | +49 [-41, +139] |
| Model fallbacks, `none` arm | 4 | 186 |
| Model fallbacks, `features` arm | 7 | 7 |
| Cost of the experiment | $2.17 | **$0.31** |

What it says:

1. **The scaffold effect replicates on a second model, and is larger.** Both models gain about +100 to +230 bb/100 from the `features` aid; both intervals exclude zero.
2. **Equity and advice add nothing measurable beyond `features` on either model**, again with wide intervals. (`advice - equity` is +49 for Qwen, but its interval includes zero.)
3. **Most of Qwen's unaided loss is one systematic fault.** Across both Qwen runs, 98% of its illegal actions (747 of 763) are "check" while facing a bet. That forces a fallback fold. The `features` arm spells out the call price and its fallbacks drop from 186 to 7. So part of the scaffold effect is "stop making an illegal move", not "play better".
4. **With aids, the two models land in the same range** (+100 to +150), so the aid matters more than the model here. Style differs: Qwen plays far fewer hands with aids (VPIP 15 to 30%, vs 28 to 45% for DeepSeek) and bets more.

## Claude feedback (Qwen, 2026-11-14)

- The right way to describe Qwen's `none` result is "this model cannot reliably read the `legal` block of the raw observation", not "Qwen is a worse poker player". Its score with `features` is within noise of DeepSeek's.
- A cheap repair (clamp an illegal check to call/fold, or one re-ask) would likely close most of the gap without any aid; it is on the next-steps list as an opt-in, labelled treatment.
- Cost: pinned Qwen is 3.4x cheaper than unpinned DeepSeek on the baseline and 7x on the scaffold experiment. The fallback to CoreWeave (10x the per-token price) made it about twice as expensive as the list price implied.
