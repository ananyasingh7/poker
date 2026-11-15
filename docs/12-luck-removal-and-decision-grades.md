# Removing luck: all-in adjustment and decision grades

How `poker-lab analyze` separates decision quality from card luck, and what it showed when re-run over every earlier experiment. Implemented 2026-11-14 in `src/evaluation.rs`; no API spend.

## All-in adjustment

When the money is in before the board is complete (the last decision of a hand happens before the river and the hand reaches showdown), the cards still to come no longer depend on anyone's decisions. The adjusted result replaces the actual run-out with the average over all of them:

- The hand's recorded actions are replayed through the real engine on decks that keep every hole card and every board card dealt so far, and vary only the board cards still to come (`Hand::with_deck`, evaluator-only). Side pots, split pots and odd chips are therefore exact.
- One missing card is enumerated exactly; two or more are sampled (400 run-outs, seeded from the hand, so the result is deterministic).
- Hands that end by a fold, or whose last decision is on the river, are unchanged.

The estimate is unbiased (its mean equals the true expected result) and never noisier than the raw one. Reported as `all-in adjusted` beside every win rate and paired difference.

**Limit:** with stacks reset to 100 big blinds every hand, few pots are all-in before the river, so the gain is modest (error bars shrink by roughly 0 to 20%). Most of the remaining noise comes from river showdowns, which this does not touch.

## Decision grades

Every decision where a seat faced a bet and called or folded is graded against the pot odds:

- Equity is the share of showdowns the seat would win against the cards its remaining opponents **actually held**, with the rest of the board unknown (exact for one missing card, 200 sampled run-outs otherwise).
- EV(call) = equity × (pot + call) − call; EV(fold) = 0. The chosen action's **loss** is the gap to the better of the two (0 when correct).
- Reported per label and per condition: graded decisions, the share of bad calls and bad folds, and the **mistake cost** in bb/100 hands, with seed-block intervals and paired differences.

**What it is not:** a range-based judgement. Using the real opponent cards is hindsight about hands, not about the board; a call that is correct against a range can grade as a mistake against the specific hand faced. Later betting (implied odds, being bluffed off later) is ignored. Raises and checks are not graded. Treat it as a consistent, low-noise yardstick for comparing conditions on the same deals, not as an absolute verdict.

**Why it matters:** it measures every graded decision instead of only net chips, so on the same deals it is much less noisy. In the memory experiment its paired intervals were about 2.5x narrower than chip results.

## Usage

```sh
poker-lab analyze RUN_OR_BATCH_DIRECTORY              # includes adjustment and grades
poker-lab analyze RUN_OR_BATCH_DIRECTORY --no-grades  # skip the (slow) per-decision grading
```

Grading every seat of a 17,000-hand batch takes about two minutes.

## Also added (opt-in, off by default)

- `repair_illegal: true`: after a well-formed but illegal action, one follow-up request shows the model its answer, why it is illegal ("You cannot check: there is a bet to call") and the legal options. Counted as `repaired_actions`; a still-illegal reply falls back as before.
- `label_positions: true`: the raw observation also carries the seat's position name and the big blind. Off by default because it changes what the `none` scaffold shows.

## Results of the re-analysis (2026-11-14, existing runs, no API spend)

Model rows only; bb/100 with 95% intervals over seed blocks. "Mistake cost" is call/fold loss per 100 hands (lower is better).

**DeepSeek baseline (28 seeds, 10,070 hands):** raw +56 [+25, +88] (SE 15.3); all-in adjusted **+50 [+27, +73] (SE 11.3)**. The adjustment narrows the interval by about a quarter, which is worth about 1.8x as many hands. Mistake cost 67 [56, 78]: 17% of graded calls and 11% of graded folds were mistakes against the cards actually held.

**DeepSeek decision aids (8 seeds):**

| Aid vs none | Raw | All-in adjusted | Mistake cost change |
| --- | --- | --- | --- |
| features | +120 [+37, +203] | +102 [+17, +186] | -20 [-52, +12] |
| equity | +104 [+25, +183] | +84 [+31, +138] | -19 [-61, +24] |
| advice | +113 [+50, +176] | +91 [+28, +153] | -23 [-65, +20] |

The aid effect survives luck removal (somewhat smaller, still clearly above zero). Its call/fold mistake cost falls by about 20 but not significantly: bad folds drop from 11% to 3 to 4%, bad calls stay at 16%. So most of the aids' gain is **not** in call-or-fold choices: it comes from betting, raising and preflop decisions, which the grade does not cover.

**Memory experiment (8 seeds):** oracle vs none: raw +33 [+2, +65], adjusted +28 [-2, +60], mistake cost **-12.5 [-24.7, -0.3]**. Notes vs none: raw +24 [-5, +52], adjusted +13 [-12, +38], mistake cost -6 [-17, +6]. With luck removed, the oracle's chip gain is no longer clearly above zero, but the grade independently shows it reduces mistakes.

**Qwen baseline (28 seeds):** raw -104 [-149, -60], adjusted -103 [-136, -70] (SE 21.7 to 16.1). Its call/fold mistake cost (36) is *lower* than DeepSeek's (67), yet it loses money. Its forced folds after illegal checks are graded (they are much of its 13% bad-fold rate), but its over-raising (PFR 46%) is not: raises are outside the grade. This is the grade's main blind spot.

**Qwen decision aids (8 seeds):** features vs none +153 [+78, +228] adjusted (raw +208).

## What this changes

- Report all-in adjusted results as the headline from now on; raw results stay alongside.
- The aid finding is robust; the oracle finding is borderline on chips and solid on decision quality; notes remain unresolved on every measure.
- The decision grade is a useful second yardstick only for call/fold play. Extending it to bets and raises needs a model of how opponents respond, which is future work.
