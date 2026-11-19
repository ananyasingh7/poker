# Results

Every experiment so far, oldest first, grouped by the same six topics as `runs/`, `examples/` and `results/`. The model is DeepSeek V4.1 Flash (`deepseek/deepseek-v4.1-flash-1017`) unless a section says otherwise. How the numbers are made is in [methods](methods.md).

**Reading the numbers.** bb/100 = big blinds won per 100 hands. `[a, b]` is a 95% interval over seeds (one seed = one set of deals, the unit of replication). "Paired" means two arms on the same deals, differenced seed by seed. "Luck-adjusted" removes all-in luck. "River cost" and "turn cost" are chips per 100 hands given up against the best choice for a player who knows how the bots play (lower is better). Unless noted the field is the five bots `station, overfolder, solid, adaptive, tight`, stacks reset to 100 BB every hand, 60-hand sessions.

## Headline findings

| # | Finding | Key number | How solid |
| --- | --- | --- | --- |
| 1 | The model beats the five-bot field without help, but captures a small share of what a tuned bot wins | +56 [+25, +88] vs ceiling +288 (thinking off) | Solid (28 seeds) |
| 2 | Telling the model facts it could compute (position, pot odds, made hand) is the biggest single gain with thinking off | +120 [+37, +203]; replicated on Qwen (+208) | Solid |
| 3 | A calculator or the `solid` rule's advice add nothing measurable on top of those facts | equity −16 [−58, +26] | Unresolved, small |
| 4 | Accurate opponent descriptions (cheat sheet) help; much more with thinking on | +33 [+2, +65] off; +107 [+54, +159] on | Solid with thinking |
| 5 | The model's own notes are accurate and change behaviour, but no chip gain is resolved | +13 [−22, +49] off; +2 [−148, +152] on | Unresolved |
| 6 | Every kind of opponent information cuts river mistakes, in the order tracker < notes < cheat sheet | −75 / −144 / −274 river cost | Solid (10 seeds) |
| 7 | Only an explicit sizing instruction fixes bet sizing; the sharp rule holds on fresh cards | sharp: river −142 [−265, −19], chips +232 [+31, +434] | Solid on 10 fresh seeds |
| 8 | Helper wins tuned on seeds 1-10 shrink on fresh seeds | plain rule −200 → −82 [−200, +35] | Solid warning |
| 9 | Thinking (low effort) is the largest lever; with thinking off the helpers do nothing | thinking off −238 [−434, −43] | Solid (10 seeds) |
| 10 | GLM-5.3 Flash wins as many chips as DeepSeek at a fifth of the cost and help gains it chips too, but it makes more graded mistakes; gpt-oss-20b has broken output | GLM vs DeepSeek chips +4 [−85, +94], turn cost +50 [+17, +82] | Two thinking models, one field |

**To date:** 85,755 hands, $16.64 reported by the runs (see [Spend](#spend)).

---

## 1. Pilots (2026-11-11 to 11-12)

Plumbing checks, not strength measurements. Runs `runs/1-pilots/`, configs `examples/1-pilots/`.

| Run | What it checked | Hands | Cost | Outcome |
| --- | --- | --- | --- | --- |
| `baseline-v1` | one model vs the original random-ish field, 2 seeds × 6 seats | 120 | $0.017 | 0 fallbacks; model +29 bb/100 ± 55 SE (meaningless) |
| `memory-v1` | none / notes / stats / both, 10 hands each | 40 | $0.014 | plumbing works; three arms chip-for-chip identical |
| `six-models-v1` | six model seats, in-process | 6 | $0.003 | 0 fallbacks |
| `subprocess-pilot-v1`, `six-subprocess-v1`, `subprocess-notes-v1` | worker processes | 19 | $0.0095 | reproduced the in-process session exactly; 1 fallback (`call` with an amount) |
| `profile-notes-v1` | notebook prompt v3 (profiles) | 18 | $0.010 | real per-opponent profiles; station read correctly; cap overruns |
| `experiment-smoke-v1/v2` | parallel batch, none vs oracle | 144 | $0.028 | 24 sessions in 59 s; every hand replays |

Findings that shaped later work: notebooks were used as hand transcripts (fixed by profile prompts); strict parsing folded unambiguous calls (`normalize_call_amount`); the original opponents were too wild to measure reads (replaced by the field above).

**Exploitation ceiling** (`results/1-pilots/ceiling-*.json`): a 162-setting tunable policy, chosen on 8 training seeds, measured on 40 held-out seeds (14,400 hands). A lower bound on what a competent exploiter wins.

| Field | Best tuned policy | Default `solid` |
| --- | --- | --- |
| Original (tight, loose, aggressive, tight, loose) | +360 [+315, +405] | +152 [+123, +181] |
| Current (station, overfolder, solid, adaptive, tight) | +288 [+252, +325] | +109 [+84, +135] |

**Subprocess overhead** (`results/1-pilots/bench-subprocess-v1.json`, 100 × 10-hand scripted sessions): 29 µs median per decision round trip (in-process < 1 µs); six workers start in 10 ms. Irrelevant next to model latency; keep large scripted simulations in-process.

## 2. Thinking off (2026-11-13 to 11-14)

Reasoning disabled, 256-token answers. Runs `runs/2-thinking-off/`, configs `examples/2-thinking-off/`, analyses `results/2-thinking-off/`.

### 2a. Baseline (`experiment-baseline-v1`)

28 seeds × 6 seat rotations × 60 hands: 10,070 hands, $1.57, 20 minutes at 12 parallel sessions. 15 fallbacks (about 1 per 1,500 decisions).

| Seat | bb/100 | Luck-adjusted |
| --- | --- | --- |
| adaptive | +110 [+75, +146] | |
| solid | +71 [+32, +111] | |
| **DeepSeek** | **+56 [+25, +88]** | **+50 [+27, +73]** |
| tight | 0 [−32, +32] | |
| overfolder | −18 [−57, +21] | |
| station | −220 [−267, −173] | |

Style: VPIP 37%, PFR 18%, postflop aggression 0.43 (calls far more than it bets), folds to 58% of postflop bets. Call/fold mistake cost 67 [56, 78] bb/100. It captures about 20% of the ceiling. One seed block has SD ≈ 81 bb/100, so resolving a 50 bb/100 difference unpaired needs about 40 seeds per arm.

### 2b. Decision aids (`experiment-scaffold-v1`)

Seeds 101-108 × 6 seats × 60 hands × 4 arms: 11,494 hands, $2.17. Each arm adds one thing to the previous (see [model input](model-input.md)).

| Arm | bb/100 | Paired vs none | Luck-adjusted vs none | VPIP | AF | Agrees with `solid` rule |
| --- | --- | --- | --- | --- | --- | --- |
| none | +29 [−15, +73] | | | 38% | 0.43 | 61% |
| features (facts sheet) | +149 [+50, +248] | **+120 [+37, +203]** | +102 [+17, +186] | 45% | 0.71 | 60% |
| equity (+ calculator) | +133 [+52, +214] | +104 [+25, +183] | +84 [+31, +138] | 39% | 0.93 | 63% |
| advice (+ `solid` rule's move) | +142 [+82, +202] | +113 [+50, +176] | +91 [+28, +153] | 28% | 2.25 | 78% |

Adjacent differences: equity − features −16 [−58, +26]; advice − equity +9 [−57, +75]. The facts sheet is the gain; most of it is not in call/fold choices (their mistake cost falls only about 20, not significantly) but in betting, raising and preflop play. Advice changes style (follows the rule 78% of the time) more than results. The unaided observation understates the model: any cross-model comparison should include a facts-sheet arm.

### 2c. Memory (`memory-budget-v1`, `memory-structured-v1`)

Pre-registered (primary question: does the cheat sheet beat no memory?), facts sheet on in every arm, seeds 1-8 × 6 seats × 120 hands, fp8 providers. 17,026 hands, $4.92; structured-notes rerun 5,760 hands, $1.61.

| Arm | bb/100 | Paired vs none | Luck-adjusted vs none | Call/fold mistake cost vs none |
| --- | --- | --- | --- | --- |
| none | +68 [+26, +110] | | | |
| cheat sheet (`oracle`) | +101 [+48, +155] | **+33 [+2, +65]** | +28 [−2, +60] | **−12.5 [−24.7, −0.3]** |
| own notes, free text (v4) | +92 [+59, +125] | +24 [−5, +52] | +13 [−12, +38] | −6 [−17, +6] |
| own notes, structured (v5) | +81 [+56, +107] | +13 [−22, +49] | | |

- Primary question: yes, narrowly. The model can turn correct reads into money, modestly (captures 24% of the ceiling without memory, 35% with the cheat sheet).
- Own notes: not resolved; no sign of learning across a session (first half +29, second half +17). Free-text notebooks overran their cap on 12.5% of updates; structured profiles (prompt v5) cut failures to 1 in 5,761 but did not make notes more useful. Notes were 68% of the spend.

## 3. Qwen 3.6 35B (2026-11-14)

`qwen/qwen3.6-35b-a3b` pinned to PrimeIntellect fp8, same configs and seeds as section 2 (`examples/3-qwen/`, `runs/3-qwen/`). Baseline 10,080 hands for $0.46; aids 11,520 hands for $0.31.

| | DeepSeek | Qwen |
| --- | --- | --- |
| Baseline (28 seeds) | +56 [+25, +88] | **−104 [−149, −60]** (paired −160 [−213, −108]) |
| Aids: none | +29 [−15, +73] | −105 [−192, −18] |
| features | +149 [+50, +248] | +103 [+2, +204] |
| Paired gain of features | +120 [+37, +203] | **+208 [+98, +317]** |
| Fallbacks, no aids | 4 | 186 |

Qwen's unaided loss is mostly one fault: 98% of its illegal moves are "check" while facing a bet (forced fold). The facts sheet spells out the price and its fallbacks drop to 7. With aids the two models land in the same range: the aid matters more than the model here. Style: very loose and aggressive (VPIP 48%, PFR 46%).

## 4. Spectator table sessions (2026-11-14 to 11-15)

Thinking on for the first time, carry-over stacks, one seed: leads, not results. `runs/4-table/`.

- **First live run** (`table-1794705445`, 309 hands, $0.15, all DeepInfra): raw −33 bb/100 because of one 449-chip cooler; luck-adjusted +110. Thinking turned the model into the aggressor (bets 26% when checked to, against 2% with thinking off). Call/fold mistake cost 24 (thinking-off baseline 67). Leaks: missed thin value against the station (checked 14 of 21 winning rivers) and bluffed into it; 14% of river decisions truncated at 4,096 tokens.
- **Cheat sheet + thinking** (`table-oracle-thinking-v1`, 79 hands, $0.085, config `examples/4-table/oracle-thinking.json`): fewer bluffs into the station, but 5.4% of decisions truncated even at 12,288 tokens because the model counted opponent combinations one by one (35% of the spend). **Fix: `reasoning_effort: low`**, checked live on the failing prompts (answered in 433-1,804 tokens). The table and every later run use it.

## 5. Thinking on, low effort (2026-11-15 to 11-16)

DeepSeek with `reasoning_effort: low`, `max_tokens` 4,096, seeds 1-10. Runs `runs/5-thinking-low/`, configs `examples/5-thinking-low/`, full comparison (model in seat 0, with turn grades) in `results/5-thinking-low/comparison.md`.

### 5a. No help vs cheat sheet, and thinking on vs off ($1.08)

| | No help | Cheat sheet | Paired |
| --- | --- | --- | --- |
| bb/100 (seats 0 and 3) | +116 [+12, +219] | +222 [+114, +331] | **+107 [+54, +159]** |
| Luck-adjusted | +140 [+51, +229] | +257 [+159, +355] | **+117 [+25, +209]** |
| Bluffs in hands the station stayed in | 64% | 30% | |
| Truncations | 0.5% | 5.5% | |

The cheat sheet is worth about three times more with thinking than without (+107 vs +33). Thinking on vs off on the same deals (no help): luck-adjusted +79 [−26, +185], unresolved here (resolved later, section 6). Thinking changes style far more than call/fold accuracy, at about 2.1x the cost per hand.

Own notes with thinking (seeds 1-5, $0.40): the model profiles the station correctly within 1-4 hands, bluffs it less and value-bets it like the cheat sheet does, but notes minus none is +2 [−148, +152].

### 5b. River grade: where each kind of help acts (seat 0, seeds 1-10)

The chip results could not separate these arms; the [river grade](methods.md#grading-decisions) can. Paired against no help:

| Arm | Seeds | Chips | River cost | Turn cost (approx) | $/hand |
| --- | --- | --- | --- | --- | --- |
| No help (river cost 435) | 10 | | | | 0.00033 |
| Tracker statistics | 10 | +8 [−43, +58] | **−75 [−141, −9]** | −28 [−52, −4] | 0.00053 |
| Own notes, every hand | 5 | +2 [−148, +152] | **−106 [−155, −57]** | | 0.00112 |
| Own notes, every 5 hands | 8 | −6 [−72, +60] | **−144 [−186, −102]** | −35 [−62, −9] | 0.00077 |
| Cheat sheet | 10 | +144 [+27, +262] | **−274 [−435, −113]** | −58 [−104, −11] | 0.00057 |
| Tracker + sizing rule | 8 | +60 [−96, +215] | **−200 [−305, −95]** | −38 [−75, −1] | 0.00056 |
| Tracker + bet menu | 9 | +62 [−48, +172] | −107 [−194, −20] | −22 [−54, +9] | 0.00055 |
| Tracker + rule + menu | 9 | +63 [−76, +203] | −183 [−292, −74] | −34 [−80, +12] | 0.00056 |

- Every kind of opponent information helps on the river, in the order tracker < notes < cheat sheet; turn effects are in the same order but much smaller. The helpers are mostly river fixes.
- Only the cheat sheet fixed **bet sizing** (sizing cost 30 against 150-170 for the rest) and bluffing the station (36% called down against 67-84%): it says in words "bet big, never bluff it". The others tell the model who calls, not what to do.
- A one-sentence sizing rule closes much of that gap; the bet menu does nothing (it lists small sizes first).
- Notes every 5 hands do as well as every hand for about 30% less; 20% of batched updates truncated.

## 6. New cards, model lineup, sharp rule (2026-11-17)

$1.70. Runs `runs/6-lineup/`, configs `examples/6-lineup/`, full table `results/6-lineup/comparison.md`. Seeds 11-20 had never been used to tune a helper (they appear only in the prompt-v2 thinking-off baseline).

### 6a. Do the helper wins hold on fresh cards? (DeepSeek, thinking low, seat 0)

| vs no help | Seeds | Chips | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| Tracker + rule, tuning seeds 1-10 | 8 | +60 [−96, +215] | −200 [−305, −95] | −38 [−75, −1] |
| Tracker + rule, fresh seeds 11-20 | 10 | +130 [−10, +271] | −82 [−200, +35] | −4 [−28, +21] |
| Tracker + rule, both | 18 | **+99 [+5, +193]** | **−135 [−213, −57]** | −19 [−40, +2] |
| Tracker + **sharp rule**, fresh seeds | 10 | **+232 [+31, +434]** | **−142 [−265, −19]** | **−34 [−67, −1]** |

The plain rule's river effect on fresh cards is less than half the tuning-seed figure: tuning on the same seeds overstated it. The sharp rule ("against a player who rarely folds, bet at least the pot on the river, usually all-in; never bluff them") was written after seeing seeds 1-10 and tested only on 11-20: the first helper clearly better than no help on fresh deals in both chips and river cost. Sharp vs plain directly: river −60 [−126, +7], chips +102 [−81, +285], not yet resolved. It shoves 32 of 44 river bets (plain rule 11 of 36; the grader says all-in was best in 29).

### 6b. Model lineup (same field, same deals)

Rule: cheaper per decision than our DeepSeek route. Only models that honour `reasoning_effort: low` qualify with thinking on.

| Arm | Seeds | Chips | River cost | River decisions /100 hands | VPIP | $/hand | Fallbacks /100 hands |
| --- | --- | --- | --- | --- | --- | --- | --- |
| DeepSeek · thinking low · none | 20 | +175 [+87, +264] | 362 | 34 | 36% | 0.00032 | 1.3 |
| DeepSeek · thinking low · tracker + rule | 18 | +292 [+154, +430] | 224 | 31 | 32% | 0.00055 | 2.1 |
| GLM-5.3 Flash · low · none | 20 | +180 [+79, +280] | 434 | 34 | 40% | 0.00006 | 0.7 |
| GLM-5.3 Flash · low · tracker + rule | 20 | +334 [+164, +503] | 342 | 35 | 46% | 0.00015 | 0.7 |
| gpt-oss-20b · low · none | 18 | −744 [−984, −504] | 1196 | 73 | 87% | 0.00007 | 14.9 |
| gpt-oss-20b · low · tracker + rule | 20 | −221 [−456, +14] | 45 | 4 | 43% | 0.00006 | 11.3 |
| DeepSeek · thinking off · none | 10 | +15 [−63, +93] | 574 | 36 | 36% | 0.00011 | 0.0 |
| DeepSeek · thinking off · tracker + rule | 10 | −8 [−122, +106] | 559 | 32 | 32% | 0.00023 | 0.0 |
| Gemma 4 26B · off · none | 10 | −10 [−64, +44] | 659 | 40 | 47% | 0.00007 | 0.3 |
| Gemma 4 26B · off · tracker + rule | 5 | −47 [−216, +122] | 914 | 45 | 56% | 0.00014 | 14.4 |

Help effect (tracker + rule minus none, paired): DeepSeek low +99 chips / −135 river; GLM +154 [+22, +286] chips / −92 [−190, +6] river / turn **+49** [+7, +90]; DeepSeek off −22 chips / −15 river.

- **GLM-5.3 Flash ties DeepSeek on chips** (+4 [−85, +94] paired, 20 seeds; an interval that wide means "not distinguishable", not "equal") at a fifth of the cost, and help lifts its chips similarly. On decision quality it is worse: turn cost +50 [+17, +82] without help and +124 [+87, +161] with it, river cost +107 [+18, +196] with help. It wins its chips with a looser style (VPIP 40-46%) against a field that pays looseness off. It thinks very briefly (about 57 output tokens per move), runs on 4-bit providers, and has never been tested with the facts sheet, notes or cheat sheet.
- **gpt-oss-20b** plays 87% of hands and loses heavily; 4-9% of its answers are empty. With the tracker it mostly folds, so its tiny river cost is an artefact of rarely reaching the river.
- **Thinking off costs DeepSeek 238 bb/100** on the same deals [−434, −43], and removes the helpers' effect.
- **Gemma 4** ignores effort settings (too dear with thinking), so it ran with thinking off; it ties DeepSeek thinking off. Its tracker arm has 5 seeds only.
- Not testable: Qwen 3.7 Flash and Meta Muse (blocked by the account's privacy settings); Qwen 3.6, Nemotron, Seed, Mistral ignore low effort and cost more than DeepSeek; Llama 4 Scout cannot think.

## Spend

From `results/runs.csv` (`poker-lab catalog runs`): cost as reported by each run's summary. Real OpenRouter usage is slightly higher, because sessions killed mid-way and later resumed are not in their batch's summary (the lineup cost $1.70 on the account against $1.58 reported).

| Topic | Runs | Hands | Reported cost |
| --- | --- | --- | --- |
| 1. Pilots | 10 | 501 | $0.11 |
| 2. Thinking off | 9 | 45,262 | $10.49 |
| 3. Qwen | 9 | 24,633 | $0.82 |
| 4. Table sessions | 4 | 648 | $0.39 |
| 5. Thinking low | 8 | 5,730 | $3.25 |
| 6. New cards and lineup | 20 | 8,981 | $1.58 |
| **Total** | **60** | **85,755** | **$16.64** |
