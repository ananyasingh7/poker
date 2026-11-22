# Results

Experiments in date order. Sections 1–6 use DeepSeek V4.1 Flash unless stated otherwise, sections 7–8 cover GLM-5.3 Flash, section 9 covers local Qwen/Pi, and sections 10–11 cover local Inkling. See [methods](methods.md) for scoring and the [roadmap](../ROADMAP.md) for next steps.

**Grader fix (2026-11-18).** River and turn costs on this page were re-graded after a fix: bots whose randomness depends on their cards (the `solid` bluff roll) were graded with each hand in one card order only; both orders are now considered. Costs moved by 1-9 bb/100 and no conclusion changed, except that tracker + rule's turn effect on seeds 1-10 (−35 [−76, +5]) no longer clears zero. Chip results are unaffected. The thinking-off analyses in `results/2-thinking-off` predate the fix, except `memory-budget-v1.txt` (re-run 2026-11-19 with learning curves).

**Reading the numbers.** bb/100 = big blinds won per 100 hands. `[a, b]` is a 95% interval over seeds (one seed = one set of deals, the unit of replication). "Paired" means two arms on the same deals, differenced seed by seed. "Luck-adjusted" removes all-in luck. "River cost" and "turn cost" are chips per 100 hands given up against the best choice for a player who knows how the bots play (lower is better). Unless noted the field is the five bots `station, overfolder, solid, adaptive, tight`, stacks reset to 100 BB every hand, 60-hand sessions.

## Headline findings

| # | Finding | Key number | How solid |
| --- | --- | --- | --- |
| 1 | The model beats the five-bot field without help, but captures a small share of what a tuned bot wins | +56 [+25, +88] vs ceiling +288 (thinking off) | Solid (28 seeds) |
| 2 | Telling the model facts it could compute (position, pot odds, made hand) is the biggest single gain with thinking off | +120 [+37, +203]; replicated on Qwen (+208) | Solid |
| 3 | In the earlier API table tests, an equity calculator or the `solid` rule's advice add nothing measurable on top of facts | equity −16 [−58, +26] | Unresolved, small |
| 4 | Accurate opponent descriptions (cheat sheet) help; much more with thinking on | +33 [+2, +65] off; +107 [+54, +159] on | Solid with thinking |
| 5 | DeepSeek's own notes change behaviour, but no chip gain is resolved in the earlier table tests | +13 [−22, +49] off; +2 [−148, +152] on | Unresolved |
| 6 | Every kind of opponent information cuts river mistakes, in the order tracker < notes < cheat sheet | −72 / −140 / −278 river cost | Solid (10 seeds) |
| 7 | Only an explicit sizing instruction fixes bet sizing; the sharp rule holds on fresh cards | sharp: river −142 [−265, −18], chips +232 [+31, +434] | Solid on 10 fresh seeds |
| 8 | Helper wins tuned on seeds 1-10 shrink on fresh seeds | plain rule −200 → −85 [−206, +35] | Solid warning |
| 9 | Thinking (low effort) is the largest lever; with thinking off the helpers do nothing | thinking off −238 [−434, −43] | Solid (10 seeds) |
| 10 | GLM-5.3 Flash wins as many chips as DeepSeek at a fifth of the cost and help gains it chips too, but it makes more graded mistakes; gpt-oss-20b has broken output | GLM vs DeepSeek chips +4 [−85, +94], turn cost +48 [+15, +82] | Two thinking models, one field |
| 11 | Against varied bots, DeepSeek beats GLM on chips; GLM costs a fifth as much | DeepSeek minus GLM +126 [+23, +228] chips | 20 fresh seeds |
| 12 | GLM’s highest-scoring setup combines facts, tracker and sharp sizing. Notes plus counts also beat no help | facts + tracker + sharp: chips +222 [+30, +415], river −146 [−238, −54]; notes + counts +184 [+1, +367]; notes alone +82 [−90, +254] | 20 fresh seeds |
| 13 | The earlier full-table learning hint (GLM notes closing on the cheat sheet) **failed a pre-registered replication** | original +393 [+112, +673]; replication −72 [−396, +253], 40 new seeds | Pre-registered |
| 14 | The sharp sizing rule is not clearly better than the plain one for GLM (the plain rule even wins more chips) | sharp minus plain: chips −130 [−303, +42], river −78 [−197, +42] | 20 fresh seeds |
| 15 | Local Qwen/Pi’s updated notes gave the same decisions as frozen notes after an opponent switched | 0 bb saved per decision in both directions | Two-session pilot |
| 16 | Inkling used fresh notes to adapt in both directions in a controlled river drill | fresh versus frozen reads: +1.003 [0.729, 1.281] bb saved per decision | Predeclared test; 48 changed histories |
| 17 | Inkling's larger weakness was choosing the bet: a fixed calculator using the same read removed about 88% of avoidable loss | calculator +0.903 [0.660, 1.144]; recent counts added +0.125 [0.052, 0.219] bb saved per decision | Descriptive comparisons; 96 histories |
| 18 | Separating reading from betting has no clearly resolved benefit when Inkling chooses its own bets | +0.081 [−0.161, +0.362] bb saved per decision | 24 paired games |

The table-run catalog predates the Inkling studies. Keep full-table hands, forced observations and synthetic river decisions separate; see [Spend](#spend) and sections 10–11 for their counts and costs.

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
| Current, varied bots (`results/7-glm-varied/ceiling-varied-v1.json`, 2026-11-18) | +313 [+253, +374] | +174 [+121, +227] |

Varied bots are slightly *easier* to beat: their deviations are mostly loose calls and stray bets, which a value-heavy policy punishes (the best setting moves from value 0.50 to 0.75). On the exact deals and seat of the GLM batch (seeds 21-40, seat 0) the tuned policy wins +271 [+146, +396] and default `solid` +130 [+42, +219]; on the fixed field the same seeds give +296 and +105. `poker-lab ceiling default` is the *original* field; use `current` for this one.

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
| No help (river cost 436) | 10 | | | | 0.00033 |
| Tracker statistics | 10 | +8 [−43, +58] | **−72 [−130, −13]** | −28 [−50, −6] | 0.00053 |
| Own notes, every hand | 5 | +2 [−148, +152] | **−98 [−146, −50]** | −29 [−92, +34] | 0.00112 |
| Own notes, every 5 hands | 8 | −6 [−72, +60] | **−140 [−178, −101]** | −32 [−63, −1] | 0.00077 |
| Cheat sheet | 10 | +144 [+27, +262] | **−278 [−437, −118]** | −56 [−102, −11] | 0.00057 |
| Tracker + sizing rule | 8 | +60 [−96, +215] | **−200 [−297, −103]** | −35 [−76, +5] | 0.00056 |
| Tracker + bet menu | 9 | +62 [−48, +172] | −113 [−191, −35] | −18 [−51, +14] | 0.00055 |
| Tracker + rule + menu | 9 | +63 [−76, +203] | −184 [−289, −80] | −31 [−76, +15] | 0.00056 |

- Every kind of opponent information helps on the river, in the order tracker < notes < cheat sheet; turn effects are in the same order but much smaller. The helpers are mostly river fixes.
- Only the cheat sheet fixed **bet sizing** (sizing cost 30 against 150-170 for the rest) and bluffing the station (36% called down against 67-84%): it says in words "bet big, never bluff it". The others tell the model who calls, not what to do.
- A one-sentence sizing rule closes much of that gap; the bet menu does nothing (it lists small sizes first).
- Notes every 5 hands do as well as every hand for about 30% less; 20% of batched updates truncated.

## 6. New cards, model lineup, sharp rule (2026-11-17)

$1.70. Runs `runs/6-lineup/`, configs `examples/6-lineup/`, full table `results/6-lineup/comparison.md`. Seeds 11-20 had never been used to tune a helper (they appear only in the prompt-v2 thinking-off baseline).

### 6a. Do the helper wins hold on fresh cards? (DeepSeek, thinking low, seat 0)

| vs no help | Seeds | Chips | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| Tracker + rule, tuning seeds 1-10 | 8 | +60 [−96, +215] | −200 [−297, −103] | −35 [−76, +5] |
| Tracker + rule, fresh seeds 11-20 | 10 | +130 [−10, +271] | −85 [−206, +35] | −7 [−33, +18] |
| Tracker + rule, both | 18 | **+99 [+5, +193]** | **−136 [−213, −59]** | −20 [−41, +2] |
| Tracker + **sharp rule**, fresh seeds | 10 | **+232 [+31, +434]** | **−142 [−265, −18]** | **−38 [−72, −4]** |

The plain rule's river effect on fresh cards is less than half the tuning-seed figure: tuning on the same seeds overstated it. The sharp rule ("against a player who rarely folds, bet at least the pot on the river, usually all-in; never bluff them") was written after seeing seeds 1-10 and tested only on 11-20: the first helper clearly better than no help on fresh deals in both chips and river cost. Sharp vs plain directly: river −60 [−126, +7], chips +102 [−81, +285], not yet resolved. It shoves 32 of 44 river bets (plain rule 11 of 36; the grader says all-in was best in 29).

### 6b. Model lineup (same field, same deals)

Rule: cheaper per decision than our DeepSeek route. Only models that honour `reasoning_effort: low` qualify with thinking on.

| Arm | Seeds | Chips | River cost | River decisions /100 hands | VPIP | $/hand | Fallbacks /100 hands |
| --- | --- | --- | --- | --- | --- | --- | --- |
| DeepSeek · thinking low · none | 20 | +175 [+87, +264] | 364 | 34 | 36% | 0.00032 | 1.3 |
| DeepSeek · thinking low · tracker + rule | 18 | +292 [+154, +430] | 225 | 31 | 32% | 0.00055 | 2.1 |
| GLM-5.3 Flash · low · none | 20 | +180 [+79, +280] | 433 | 34 | 40% | 0.00006 | 0.7 |
| GLM-5.3 Flash · low · tracker + rule | 20 | +334 [+164, +503] | 342 | 35 | 46% | 0.00015 | 0.7 |
| gpt-oss-20b · low · none | 18 | −744 [−984, −504] | 1200 | 73 | 87% | 0.00007 | 14.9 |
| gpt-oss-20b · low · tracker + rule | 20 | −221 [−456, +14] | 43 | 4 | 43% | 0.00006 | 11.3 |
| DeepSeek · thinking off · none | 10 | +15 [−63, +93] | 577 | 36 | 36% | 0.00011 | 0.0 |
| DeepSeek · thinking off · tracker + rule | 10 | −8 [−122, +106] | 563 | 32 | 32% | 0.00023 | 0.0 |
| Gemma 4 26B · off · none | 10 | −10 [−64, +44] | 662 | 40 | 47% | 0.00007 | 0.3 |
| Gemma 4 26B · off · tracker + rule | 5 | −47 [−216, +122] | 933 | 45 | 56% | 0.00014 | 14.4 |

Help effect (tracker + rule minus none, paired): DeepSeek low +99 chips / −136 river; GLM +154 [+22, +286] chips / −91 [−191, +9] river / turn **+46** [+5, +87]; DeepSeek off −22 chips / −14 river.

- **GLM-5.3 Flash ties DeepSeek on chips** (+4 [−85, +94] paired, 20 seeds; an interval that wide means "not distinguishable", not "equal") at a fifth of the cost, and help lifts its chips similarly. On decision quality it is worse: turn cost +48 [+15, +82] without help and +121 [+85, +157] with it, river cost +105 [+17, +193] with help. It wins its chips with a looser style (VPIP 40-46%) against a field that pays looseness off. It thinks very briefly (about 57 output tokens per move), runs on 4-bit providers, and has never been tested with the facts sheet, notes or cheat sheet.
- **gpt-oss-20b** plays 87% of hands and loses heavily; 4-9% of its answers are empty. With the tracker it mostly folds, so its tiny river cost is an artefact of rarely reaching the river.
- **Thinking off costs DeepSeek 238 bb/100** on the same deals [−434, −43], and removes the helpers' effect.
- **Gemma 4** ignores effort settings (too dear with thinking), so it ran with thinking off; it ties DeepSeek thinking off. Its tracker arm has 5 seeds only.
- Not testable: Qwen 3.7 Flash and Meta Muse (blocked by the account's privacy settings); Qwen 3.6, Nemotron, Seed, Mistral ignore low effort and cost more than DeepSeek; Llama 4 Scout cannot think.

## 7. GLM against varied bots (2026-11-18 to 11-19)

For this API study the workhorse was GLM-5.3 Flash (thinking low), against less predictable bots (`opponent_variation`: per-session personalities, 10% mixing, a field drawn per seed from the five current styles), on fresh seeds 21-40, seat 0, with one DeepSeek anchor arm on the same deals. Configs `examples/7-glm-varied/`; comparison `results/7-glm-varied/comparison.{md,json}` (`poker-lab compare ... --seat 0 --turn`). 7,212 hands, $1.03 reported ($1.08 on the account, smoke test included). Every batch completed 20 of 20 sessions; GLM had 0-0.6 fallbacks per 100 hands and no truncations, DeepSeek 20 truncated answers in 1,200 hands.

### 7a. Ceiling for the varied field (free)

`results/7-glm-varied/ceiling-{varied,fixed}-v1.json`. The tuned policy wins +313 [+253, +374] against varied bots (fixed: +288), default `solid` +174 (fixed: +109): the varied field is slightly *softer*, because most deviations are loose calls and stray bets. On the batch's exact deals and seat (seeds 21-40, seat 0) the tuned policy wins +271 [+146, +396]. The best model arm below (+173) reaches about two thirds of that.

### 7b. Arms (GLM unless named, seat 0, 20 seeds, bb/100)

| Arm | Chips | Luck-adjusted | River cost | Turn cost | VPIP | $/hand |
| --- | --- | --- | --- | --- | --- | --- |
| No help | +22 [−111, +155] | +30 [−64, +125] | 438 | 243 | 44% | 0.00007 |
| Facts sheet | +173 [+11, +334] | +131 [+3, +259] | 369 | 167 | 40% | 0.00007 |
| Cheat sheet | +98 [−26, +223] | +84 [−47, +215] | 316 | 267 | 41% | 0.00011 |
| Tracker + sharp rule | +98 [−76, +271] | +158 [−21, +336] | 301 | 253 | 48% | 0.00015 |
| Own notes every 5 hands | +104 [−35, +243] | +111 [−28, +250] | 489 | 247 | 48% | 0.00015 |
| DeepSeek, no help (anchor) | +148 [+51, +245] | +137 [+42, +231] | 401 | 181 | 36% | 0.00032 |

Paired against GLM with no help (same deals):

| Help | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| Facts sheet | +151 [+0, +301] | +101 [−16, +219] | −69 [−174, +35] | **−76 [−137, −16]** |
| Cheat sheet | +76 [−105, +258] | +54 [−106, +214] | −122 [−249, +5] | +24 [−35, +83] |
| Tracker + sharp rule | +76 [−68, +219] | +127 [−6, +261] | **−137 [−228, −46]** | +10 [−41, +60] |
| Own notes every 5 | +82 [−90, +254] | +81 [−73, +234] | +51 [−66, +168] | +4 [−59, +67] |
| *DeepSeek instead of GLM* | **+126 [+23, +228]** | **+106 [+23, +190]** | −37 [−159, +85] | −62 [−145, +20] |

- **On varied bots DeepSeek is clearly the stronger player** (+126 chips), unlike the fixed field where they tied (+4 [−85, +94]). GLM costs 4.6x less per hand. GLM has more room to improve, which was the point of choosing it.
- **The facts sheet is GLM's best help**, as it was DeepSeek's with thinking off: it lifts chips about to DeepSeek's level and is the only help that cuts turn mistakes.
- **Tracker + sharp rule is the clearest river fix** (−137), as for DeepSeek on fresh cards (−142). The cheat sheet's river effect is similar but not resolved.
- **GLM's own notes do nothing measurable** (river +51, the wrong direction), where DeepSeek's notes cut river mistakes. Its notebooks are tentative and thin: after 60 hands each seat's profile rests on "2 hands" or "4 hands" of evidence, because the model rewrites counts from recent hands instead of accumulating them. Fixable (host-kept counts beside the notes, or more frequent updates); a concrete target for "improve the weaker model".

### 7b'. Follow-up arms (2026-11-19, same seeds 21-40)

Three more GLM arms on the same deals, paired against the arms above (`results/7-glm-varied/comparison.{md,json}` now holds all nine arms). A provider overload (Wafer answering 429 `server_overloaded`) interrupted these batches; every interrupted session was re-run from scratch, and all 60 sessions completed.

| Arm | Chips | River cost | Turn cost | Paired against | Chips | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Tracker + plain sizing rule | +228 | 379 | 219 | tracker + sharp rule | sharp minus plain: −130 [−303, +42] | −78 [−197, +42] | +34 [−17, +85] |
| Facts + tracker + sharp rule | +244 | 292 | 203 | no help | **+222 [+30, +415]** | **−146 [−238, −54]** | −40 [−130, +50] |
| | | | | facts sheet alone | +71 [−75, +218] | −77 [−154, +1] | +37 [−43, +116] |
| Notes + tracker counts, every 5 | +206 | 355 | 241 | no help | **+184 [+1, +367]** | −83 [−224, +59] | −2 [−68, +64] |
| | | | | notes alone | +102 [−104, +307] | −134 [−276, +8] | −7 [−58, +45] |
| | | | | tracker + plain rule | −22 [−199, +156] | −24 [−186, +139] | +22 [−45, +88] |

- **Sharp vs plain rule (the held-back item 6), with GLM:** not settled, and the plain rule even wins more chips. The sharp rule's advantage for DeepSeek on seeds 11-20 does not carry over to GLM against varied bots. Keep both as options; do not claim the sharp rule is better.
- **Facts + tracker + sharp rule is GLM's best arm** (+222 chips, −146 river vs no help), about DeepSeek's level and above it. The pieces only partly add up: it keeps the river fix but loses the facts sheet's turn advantage.
- **Notes + counts beat no help** (+184 chips). The comparison with notes alone is too uncertain to isolate the counts' contribution. The tracker + plain rule arm also changes the sizing prompt, so this design does not isolate the contribution of notes on top of counts. Notebooks still cite only 2–5 hands per seat.

### 7c. Learning within a session (new measure, free on old runs too)

`analyze` and `compare` now split each session into 20-hand segments and report the last minus the first, paired by seed ([methods](methods.md#measuring)). Because the cards differ by hand number, an arm is compared with another arm on the same deals.

- **No arm, on any model, measurably improves over a session relative to no help on chips.** Old runs agree: DeepSeek thinking off with notes over 120-hand sessions, (late minus early) minus no help's: +8 [−87, +103] chips, river +30 [−38, +100] (`results/2-thinking-off/memory-budget-v1.txt`); thinking low, seeds 1-10 and 11-20: every chip interval includes zero (`results/5-thinking-low`, `results/6-lineup`).
- **No help is the wrong control for learning.** On seeds 21-40 the costly river spots cluster late, so every help that fixes river mistakes, including ones that cannot learn (facts, cheat sheet), shows a "learning" signal against no help (−312, −342 river). The right control for notes is the cheat sheet: same knowledge, from hand 1. If notes learn, their gap to the cheat sheet should close.
- **Against that control, GLM's notes close the gap on chips** (late minus early, notes minus cheat sheet: +393 [+112, +673]) **but not on the river** (+134 [−136, +405]); over the whole session they stay 173 [+77, +269] river cost behind the cheat sheet. One positive among many intervals examined: a hypothesis to replicate on fresh seeds, not a finding.
- **A 20-hand slice is very noisy on chips** (±300 bb/100 at 20 seeds). Learning claims need more seeds (cheap with GLM), longer sessions, or grades rather than chips.

## 8. Pre-registered: does GLM learn from its notes? (registered 2026-11-19, before running; result added after)

Written and committed before any of these hands were played, so the test cannot be fitted to the result.

**Hypothesis.** In section 7c, GLM with its own notes closed part of its gap to the cheat sheet over a session: (hands 41-60 minus hands 1-20) for notes minus the same for the cheat sheet was +393 [+112, +673] luck-adjusted bb/100 on seeds 21-40. If GLM really learns from its notes, this holds on new cards.

**Design.** `examples/8-glm-learning/notes-vs-oracle.json`: GLM-5.3 Flash, thinking low, seat 0, varied bots (personalities, mixing 0.1, random field from the five current styles), 60-hand sessions, **seeds 41-80 (never used before)**, two arms on the same deals: cheat sheet (`oracle`) and own notes rewritten every 5 hands (`notes`, structured profiles). Cap $0.75. Only complete seeds count.

**Primary test (the only one that decides replication).** Per seed: notes (hands 41-60 minus 1-20, luck-adjusted bb/100) minus cheat sheet (same). 95% t-interval over seeds, from `poker-lab analyze runs/8-glm-learning/notes-vs-oracle-v1 --turn` (the "learning" row of notes vs oracle). **Replicates** if the interval lies above zero; **fails** if it includes zero or lies below.

**Secondary (reported, not decisive).** The same difference for river cost and turn cost (learning would make them negative); whole-session notes minus cheat sheet for chips, river and turn cost. Anything else is exploratory and labelled so.

**Result (added after the run; nothing above was changed).** 80 of 80 sessions complete, 4,800 hands, $0.61 reported (several sessions re-run after provider overloads). `results/8-glm-learning/notes-vs-oracle-v1.txt`.

- **Primary: −72 [−396, +253] bb/100** (40 seeds; `analyze` uses 1.96 above 31 blocks; with t₃₉ the interval is [−407, +264]). It includes zero, and the point estimate has the opposite sign of the original +393. **The hint does not replicate.**
- Secondary: river cost +23 [−155, +200], turn cost +10 [−116, +136]: no learning there either. Whole session, notes minus cheat sheet: chips +90 [−20, +200], river +45 [−21, +110], turn +1 [−50, +52] (on these seeds notes were close to the cheat sheet throughout, unlike seeds 21-40).
- *Exploratory:* both arms do best in hands 1-20 and worst in 41-60 on these deals (cards again), and the notes arm is already ahead of the cheat sheet in hands 1-20 (+227 vs +120 adjusted). Whatever GLM's notes contribute arrives early or not at all.
- **Reading:** the original +393 was most likely one of the many intervals examined in 7c coming out positive by chance. We still have no evidence of any model learning within a session at this table.

## 9. Local Qwen/Pi (2026-11-19)

Qwen3.8-27B Q4_K_M ran locally with the Pi F16 LoRA, thinking off and temperature 0. On seed 901 it made 13 legal decisions in six hands. Both structured notebook updates left the profiles empty. In a two-hand replay, its free-form note reached the next hand but called three eights a pair. Both runs cost $0 in API fees. [Smoke config](../examples/9-local-pi/smoke.json)

### Hidden river policy switch (pilot, 2026-11-20)

Two 16-hand river games, one switch in each direction. After eight hands the opponent switched from always calling to always folding, or vice versa. Qwen faced top pair and could check or bet the 10 bb pot. It wrote notes every four hands. At each checkpoint we tested two bluff hands and two value hands with four versions of the notebook. The two seeds varied suits and shared rank templates. Identical probe inputs reused the same response. [Runner](../experiments/river_switch.py)

| Opponent after switch | Updated notes | Frozen notes | Blank | True profile |
| --- | ---: | ---: | ---: | ---: |
| Always folds | 5 | 5 | 5 | 0 |
| Always calls | 0 | 0 | 0 | 0 |

Values are bb lost per test decision. Qwen saw five folds after the first switch but still checked both profitable bluffs. Against the caller, its usual play was already optimal. Live winnings before and after the switch were +40 then +10 bb for call to fold, and 0 then +40 bb for fold to call. [Hands and notebook updates](../results/9-local-pi/river-switch-hands-v1.json)

**Exploratory follow-up.** The full note advised checking weak hands. Given only its own first sentence about the opponent folding, Qwen bet both bluffs. We selected that sentence after seeing the result. The follow-up compared observation-only notes with notes that also give advice; see the next subsection. [Probe actions and scores](../results/9-local-pi/river-switch-v1.json)

### Reading the opponent separately from betting (pilot, 2026-11-20)

Four fixed observation histories used fold rates of 20% → 80%, 80% → 20%, 20% → 20% and 80% → 80%. Each contained 24 predetermined pot bets. Both notebook writers saw the same responses and updated every six bets. One prompt requested observations only; the other also requested advice. At each 12-bet checkpoint, Qwen estimated the current fold rate in one call and chose bets in separate calls. Four fresh river probes included explicit hand-strength labels. Thinking was off, temperature 0. [Runner and design](../experiments/read_then_bet.py)

| Notebook prompt | Direct notes to bet | Notes to fold estimate to bet |
| --- | ---: | ---: |
| Observations only | 1.875 | 1.500 |
| Observations plus advice | 1.500 | 2.000 |

Values are mean bb lost per test decision at the final checkpoint, averaged over four histories. The observation-based separate stage matched the blank notebook's actions. Every separate-stage bluff was checked, even when the supplied fold rate was 80% or 100%. This helped against callers and missed profitable bluffs against folders. The advice-based stage also checked a value hand when supplied a 0% fold rate. Splitting the steps did not produce reliable adaptation. [All scores, observations and notebook updates](../results/9-local-pi/read-then-bet-v1.json)

**Reader-test correction.** The response grammar allowed only strings at five-point intervals. An intended 8.3% answer could be forced into 80% or 85%. Inkling's later diagnostic exposed this problem. Qwen's recorded 80% answers from 22 calls and two folds cannot establish a reading failure. The betting scores remain as recorded, but the separate-stage scores depend on this flawed reader. Numeric replays allow ordinary percentages.

**Exploratory payoff control.** On eight probe situations, supplying the true fold percentage as a number or sentence left two profitable bluffs unplayed. Supplying the calculated expected payoffs made all eight choices optimal. On an 80%-fold bluff, checking was worth −5 bb and betting +1 bb, including the sunk contribution. These controls were selected after inspecting the main run. [Control runner](../experiments/read_then_bet_controls.py), [control data](../results/9-local-pi/read-then-bet-controls-v1.json)

**Limits.** One history per condition, shared board templates, forced observation bets and supplied hand strength. The observations-only writer sometimes added advice, and notebooks confused the player's cards with the opponent's. The percentage grammar needs correction before drawing conclusions about reading. The direct betting and supplied-payoff controls remain useful; this is a controlled drill rather than full-table learning.

## 10. Local Inkling 2 Mini (2026-11-20)

Inkling IQ4_NL ran on port 9932 with an 8,192-token context, temperature 0 and the same seeds, cards and scoring as Qwen. The original short answer limits filled with reasoning and produced three failed table decisions. Starting the native JSON answer directly (`local_json_prefill`) let both table checks finish cleanly: six hands with structured notes and two with journal notes, no fallbacks or memory errors, $0 API fees. Inkling wrote nonempty notes, but their counts and card descriptions were not consistently accurate. Different quantization, context and answer prefixes limit model comparisons. [Configs](../examples/10-inkling/), [saved data](../results/10-inkling/suite-v1.json)

### Hidden river policy switch

| Opponent after switch | Qwen fresh / frozen notes | Inkling fresh / frozen notes |
| --- | ---: | ---: |
| Always folds | 5 / 5 | 5 / 5 |
| Always calls | 0 / 0 | 5 / 5 |

Values are bb lost per probe decision. Fresh notes did not improve either model over frozen notes. Inkling checked every hand after the opponent became a folder, so it never observed the changed response. In the reverse switch it saw eight calls but kept advice to bet every hand. These two small drills show no reliable adaptation. [Comparison](../results/10-inkling/comparison-v1.json)

### Reading separately from betting

The same four observation histories and notebook prompts as section 9 were tested. The corrected replay reused those notes but allowed ordinary numeric percentages.

| How Inkling chose its bet | Observations-only notes | Notes with advice |
| --- | ---: | ---: |
| Full notes directly | 1.500 | 1.875 |
| Fresh notes → numeric estimate → bet | 0.375 | 0.750 |
| Frozen notes → numeric estimate → bet | 2.250 | 1.125 |

Values are mean bb lost per final-checkpoint decision: 16 decisions per column and method, across four histories. Splitting the steps reduced average cost, but the corrected replay still made a bad bluff after the opponent became a caller. The observations-only stage improved against the newly folding opponent and worsened against the newly calling opponent compared with its frozen read. The simple recent-count baseline lost zero; supplying the true fold rate still lost 0.375. Betting choices remain inconsistent even with useful estimates. [Numeric replay](../experiments/read_then_bet_numeric.py)

Three of 32 notebook updates hit the 256-token limit. The drill continuation kept the previous note, matching the table's behavior; this policy was added after the failure. Saved requests were replayed and all completed checkpoints stayed unchanged.

### Small betting and reader checks

| Information supplied | Qwen correct bets | Inkling correct bets |
| --- | ---: | ---: |
| Fold chance as a number | 6 / 8 | 8 / 8 |
| Fold chance in a sentence | 6 / 8 | 6 / 8 |
| Calculated winnings for each choice | 8 / 8 | 8 / 8 |

These use the same eight situations as Qwen's exploratory controls. Inkling handled the numeric version better here, but the sentence version still missed two profitable bluffs.

The legacy percentage buckets returned 80% folds from 22 calls and two folds. With the same reader prompt and ordinary numeric output, Inkling returned 8.3%; reversing the counts returned 91.7%. This exposes a test-format problem, not evidence that Qwen cannot read counts. Qwen needs the corrected replay before comparing reader accuracy.

**Reading.** Inkling is a useful local player, and separating the steps is promising in this small drill. Full-note play did not adapt reliably. There is one history per condition, forced observation bets, shared card templates and no uncertainty interval. Repeat the corrected test on fresh histories before calling this reliable learning or choosing fine-tuning.

## 11. Overnight Inkling study (registered 2026-11-20)

The fixed plan includes 96 independent 120-observation histories, 1,200 skill questions and 24 pairs of 80-hand own-choice games. The primary test compares fresh and frozen reads after an opponent changes. Separate comparisons hold the read fixed while replacing betting with a calculator, then hold the calculator fixed while replacing notes with exact recent counts. These guide the choice between better memory, a betting calculator and a later fine-tuning experiment.

[Automatic report](../results/11-inkling-overnight/summary.md), [analysis data](../results/11-inkling-overnight/analysis.json), [live status](../results/11-inkling-overnight/progress.json), [registered plan](../examples/11-inkling-overnight/protocol.json). The report marks completion explicitly and includes uncertainty over independent histories. Fine-tuning is not performed in this study.

**Completed 2026-11-21.** All 96 histories, 1,200 skill questions and 24 game pairs finished in 6.63 hours, using 16,716 local requests. Five note writes hit the limit and kept their previous note; no action or reader answers failed. API fees were $0.

**Primary result: passed in the controlled drill.** Fresh reads saved 1.003 [0.729, 1.281] bb per decision compared with frozen reads after a switch. Both directions improved: caller to folder +1.076 [0.653, 1.549], folder to caller +0.931 [0.611, 1.271]. This is adaptation through notes with unchanged model weights, guaranteed observations and supplied hand strength.

**The biggest measured fix was choosing the bet.** On the same 1,152 final-half probes across all 96 histories, separate reading and model betting made 196 wrong choices; a calculator using the same reads made 24; recent counts plus the calculator made zero. The calculator removed about 88% of avoidable chip loss, saving 0.903 [0.660, 1.144] bb per decision. Exact recent counts then saved another 0.125 [0.052, 0.219]. These intervention comparisons are descriptive. Zero errors applies to these simple test situations.

**Memory was useful but imperfect.** Notes-based reads missed the true fold percentage by 8.13 points on average; raw-history reads by 8.61; recent counts by 7.21. Giving the entire history did not clearly improve decisions over notes. Most remaining model-betting errors were bluffs against frequent callers: 192 of 196 wrong choices in the separate-stage setup.

**Own-choice play remains unresolved.** Separating reading from betting saved 0.081 [-0.161, 0.362] bb per decision after a change in 24 paired games. The interval includes zero. The controlled result has not established a benefit when Inkling determines which evidence it collects.

**Next step.** Test a fixed betting calculator first, with exact-count memory as a separate addition, in fresh own-choice games. Fine-tuning remains a later candidate for residual errors; this study did not measure its benefit. [Plain-language findings](../results/11-inkling-overnight/takeaways.md)

## Spend

The totals below predate Inkling. Its completed table checks add eight hands at $0 API cost; compatibility failures are described in section 10.

From `results/runs.csv` (`poker-lab catalog runs`): cost as reported by each run's summary. Real OpenRouter usage is slightly higher, because sessions killed mid-way and later resumed are not in their batch's summary (the lineup cost $1.70 on the account against $1.58 reported; the 2026-11-19 runs $1.63 against about $1.30, because sessions interrupted by provider overloads were re-run).

| Topic | Runs | Hands | Reported cost |
| --- | --- | --- | --- |
| 1. Pilots | 10 | 501 | $0.11 |
| 2. Thinking off | 9 | 45,262 | $10.49 |
| 3. Qwen | 9 | 24,633 | $0.82 |
| 4. Table sessions | 4 | 648 | $0.39 |
| 5. Thinking low | 8 | 5,730 | $3.25 |
| 6. New cards and lineup | 20 | 8,981 | $1.58 |
| 7. GLM against varied bots | 10 | 10,812 | $1.72 |
| 8. Pre-registered learning test | 1 | 4,800 | $0.61 |
| 9. Local Qwen/Pi table checks | 2 | 8 | $0.00 |
| **Table total** | **73** | **101,375** | **$18.97** |

The separate river drill added 32 hands at $0 API cost.
