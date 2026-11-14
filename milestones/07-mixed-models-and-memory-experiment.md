# Milestone 7: Mixed-model tables and a powered memory experiment

**Status:** Memory experiment run on 2026-11-14 (budget design, see Results). A real mixed-model table has not been run.

**Goal:** Let different models share a table, and set up a memory experiment that can actually detect an effect.

- Per-seat model IDs, rotated through the seats by the batch so every model plays every position on every seed.
- A profile-style notebook prompt (version 3): one evidence-counted profile per opponent, no hand transcripts.
- An oracle-notes condition: accurate experimenter-written opponent descriptions, to separate using reads from forming them.
- Sessions of 120 hands with stacks reset every hand.

## Pre-registered comparison for the memory experiment

- **Config:** `examples/experiment-memory.json` (none, stats, notes, both, oracle; 4 seeds × 6 seats × 120 hands; field: station, overfolder, solid, adaptive, tight).
- **Primary metric:** the focal model's bb/100, paired difference from the `none` condition on shared seeds, with a 95% t-interval over seed blocks.
- **Primary question:** does `oracle` beat `none`? Secondary: does `notes` beat `none`, and how much of the oracle gain does it capture?
- **Also reported:** fallback and normalized-action counts, notebook rejection rate, cost per hand, playing style per condition, and fraction of the field's exploitation ceiling captured.
- **Stopping rule:** run the schedule once. If the oracle interval includes zero and is wider than ±40 bb/100, add seeds in multiples of four and say so; do not stop early on a favourable interim result.

## Amendment 1 to the pre-registration (2026-11-14, before any data)

Made after milestone 9 and three smoke tests (`runs/memory-smoke-v1..v3`, 240 hands each, not analysed for effects). None of the full experiment has been run.

| Item | Was | Now | Why |
| --- | --- | --- | --- |
| Config | `experiment-memory.json` | `experiment-memory-features.json` | — |
| Observation | raw view | `features` decision aid in every arm | Without it, DeepSeek misreads facts it can compute (+120 bb/100 from features alone); memory effects should be measured on the unhandicapped player. Condition labels become `none+features` etc. |
| Seeds | 4 (add in fours if inconclusive) | 12 (seeds 1 to 12) | Block SD from the baseline is about 57 bb/100 for 720 seat-hands; 4 paired blocks give roughly ±90, too wide for the ±40 target. 12 is the stopping rule's own increment applied up front. |
| Notebook prompt | version 3 (`profiles`) | version 4 (`compact_profiles`) | Version 3 kept hand logs and labelled seats by rotating positions; 8 to 15% of updates exceeded the cap and were discarded, freezing the notes. Version 4 names seats by number, states the current length, and on an oversize reply asks once to compress it (rejections 1% in the smoke test). |
| Provider | OpenRouter's choice | fp8 only, DeepInfra first | Holds weight precision fixed across arms. |
| Model faults | session stops after 3 in a row | unchanged (default) | DeepSeek's fault rate is 0.07%. |

Unchanged: model (`deepseek/deepseek-v4.1-flash-1017`), conditions (none, stats, notes, both, oracle), 6 seat rotations, 120 hands, stacks reset every hand, the field, the primary metric and question, the secondary questions, the reporting list, and the stopping rule (if the oracle interval includes zero and is wider than ±40, add seeds in fours and say so).

Size: 5 conditions × 12 seeds × 6 seats × 120 hands = 43,200 hands. Estimated cost $10 to $15 (smoke tests: $0.0002 to $0.0004 per hand depending on arm); batch cap $20.

## Amendment 2 (2026-11-14, before any data): budget

The project budget for this experiment is $6, so the amended design (about $13) was cut to `examples/experiment-memory-budget.json`: conditions **none, notes, oracle** (stats and both dropped), **8 seeds** (1 to 8), everything else as in amendment 1. 3 × 8 × 6 × 120 = 17,280 hands, estimated $4.60, hard cap $5.50. Expected precision is about ±50 bb/100 on each paired difference, so only a large effect will be resolved; the stopping rule (add seeds in fours) still applies if a later budget allows.

## Done when

- [x] A table can seat a different model in each seat, and analysis reports per-model results.
- [x] Profile notes and oracle notes are selectable conditions on both backends.
- [x] The pre-registered memory comparison has been run (budget design, amendment 2) and its intervals recorded.
- [ ] At least one table with genuinely different models has completed.

See [what was built and the live checks](../docs/11-analysis-experiments-and-opponents.md).

## Results (2026-11-14, `runs/memory-budget-v1`)

144 sessions, 17,026 of 17,280 hands, 46,531 requests, **$4.92**. Model `deepseek/deepseek-v4.1-flash-1017` with the `features` aid, fp8 providers (DeepInfra first).

| Condition | Model bb/100 | Paired difference vs `none` | VPIP | PFR | AF | Fold to bet |
| --- | --- | --- | --- | --- | --- | --- |
| `none` | +68 [+26, +110] | — | 44% | 23% | 0.61 | 52% |
| `notes` | +92 [+59, +125] | +24 [-5, +52] | 44% | 18% | 0.77 | 57% |
| `oracle` | +101 [+48, +155] | **+33 [+2, +65]** | 40% | 15% | 0.98 | 61% |

95% t-intervals over 8 seed blocks. `notes − oracle`: −10 [−48, +28].

**Primary question (does `oracle` beat `none`?): yes, narrowly.** Accurate opponent descriptions add about +33 bb/100. The interval just excludes zero and is narrower than ±40, so the stopping rule does not call for more seeds.

**Secondary (do the model's own notes help?): not resolved.** +24 bb/100, about 70% of the oracle gain as a point estimate, but the interval includes zero. Split by session half, the notes gain does not grow with more hands: +29 in hands 1 to 60, +17 in hands 61 to 120, both with wide intervals. There is no sign of learning across the session.

Context: the field's exploitation ceiling is about +288 bb/100. The model captures 24% of it without memory and 35% with oracle notes.

**Caveats:**

- **Notebook rejections.** As notebooks grew, 22% of replacements exceeded the 2,000-character cap. The compression follow-up rescued 43% of those; the rest (701 updates, 12.5%) were rejected and the previous notebook kept. The `notes` arm therefore updated on about 87% of hands. The smoke test (24 hands) showed only 1%; the failure grows with session length.
- **Three sessions ended early** on a reply without a reported cost (2 `notes`, 1 `oracle`; 254 hands). The batch absorbed them under its allowance and kept going.
- **Cost by arm:** `notes` $3.34 (68% of the spend), `none` $0.91, `oracle` $0.63.

## Notebook fix: structured profiles (2026-11-14, implemented, not yet run live)

`notebook_style: "structured_profiles"` (prompt version 5). The model no longer rewrites a free-text notebook. It returns `operation` plus a list of `{seat, profile}` edits; the host merges them into per-opponent profiles and renders the notebook. Each profile has its own limit (`notebook_chars` divided by the number of opponents; 400 characters at the 2,000 default with five opponents). An overlong or out-of-range edit is ignored for that seat only (the old profile stays, and the audit log records `ignored_profiles`); an empty profile deletes the seat. The notebook can therefore never exceed the cap and a single overrun never discards the other seats' updates. Versions 2 to 4 are unchanged, so earlier runs stay comparable. `examples/experiment-memory-structured.json` runs `none` against structured `notes` on 8 seeds (about $2.50 estimated, not yet run).

## Claude feedback (memory results, 2026-11-14)

- The oracle result is the useful one: the model can turn correct opponent reads into money, but only modestly (+33 on a +68 base). The bigger gain in this project so far came from the `features` aid (+120), not from memory.
- The notes arm is the expensive part, and its implementation still loses an eighth of updates. Before spending more on it: raise the cap (3,000), or switch to a structured notebook (one field per seat with a per-field length) so overruns cannot discard everything.
- Resolving `notes` vs `none` at this effect size needs roughly 3x the seeds (about $10 for the notes and none arms alone at current prices).

## Claude feedback (2026-11-12)

- I dropped the reflection-without-persistence control I proposed earlier. With stateless model calls it cannot differ from the no-memory condition.
- In the one live trial the profile prompt produced real per-opponent profiles and read the calling station correctly. It also hit the size cap three times in 18 hands. A cap of 3,000 characters, or telling the model its current length, would reduce rejected updates; changing either is a new condition.
- The mixed-model example needs real model IDs. I did not guess any. Models that reject `json_schema` or the reasoning switch will need per-seat request settings, which do not exist yet.
