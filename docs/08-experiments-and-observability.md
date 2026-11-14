# Reports, controlled opponents, and memory experiments

The milestone 4 follow-up adds Rust-only hand reports, fixed opponent policies, multi-session runs, and independent notebook/statistics conditions. These are exploratory experiments, not a validated model ranking.

## Read a run

```sh
cargo run -- report /tmp/poker-openrouter-run
cargo run -- report /tmp/poker-openrouter-run 0
```

The first command prints public history. The optional seat selects that player's private cards, reconstructed by the evaluator; do not share that output with other players. Both commands verify replay before rendering. Each hand shows starting stacks, button/blinds, actions (RaiseTo means total street commitment), board streets, publicly revealed cards, pot eligibility and awards, refunds, final stacks, per-hand net chips, and fallback reasons. Reports omit seeds and unrevealed opponents' cards. New model runs automatically write `hand-history-private.txt` for the model's seat with mode 0600.

Model traces retain exact requests/responses, latency, usage, resolved model/provider, errors and applied actions via the linked evaluator record. `purpose` distinguishes action and notebook requests. Prompt version 2 identifies support for memory treatments. The original version-1 engine replay format remains unchanged.

## Fixed opponent policies v1

Set `opponents` to `mixed`; `random` preserves the old mechanics fixture. The mixed list is tight, loose, aggressive, tight, loose, with the model inserted at its configured seat. `opponent_offset` rotates this list, so batch runs change neighbors as well as model seat. Stable seat numbers are opponent aliases within each session; provider and policy identities are not sent to the model.

These policies are intentionally simple, deterministic and inspectable. Preflop score is 30 plus rank for a pair; otherwise both ranks summed plus three if suited. Tight plays score >=25 and raises score >=38 or AK/AQ; loose plays >=17 and raises >=40; aggressive plays >=21 and raises >=26. Ranks are 2 through 14. Strong hands raise to at least six preflop, or half the pot plus own street contribution postflop, clamped to legal bounds. A legal short all-in raise is not selected by these heuristics; they can still call all-in.

Postflop the best-five hand category determines strength: tight continues with at least a pair and raises two pair or better; loose continues with a pair or a jack-or-higher hole card and raises trips or better; aggressive continues with a pair or a queen-or-higher hole card and raises a pair or better. A free check is taken when not raising. Non-strong hands call only up to one quarter (tight), one half (loose), or one third (aggressive) of their remaining stack; otherwise they fold. These heuristics include board-made hands, have no equity/range calculations, and are not GTO or strong reference players. Tests verify legality and deterministic behavior, not profitability.

## Larger baseline and experimental conditions

```sh
cargo run -- batch examples/openrouter-baseline.json runs/baseline-v1
cargo run -- batch examples/openrouter-memory.json runs/memory-v1
```

Create the parent `runs` directory if needed, and choose a fresh output directory. Baseline config runs two seeds across all six model seats, ten hands per session (120 total), against mixed opponents. All state, notes, stacks, and statistics reset between sessions. Each seed/seat condition has an individual run directory with the usual audit files. `sessions.jsonl` is flushed after each session; `batch-summary.json` contains per-session outcomes and clean/contaminated status. A session is clean only if it completes the requested hands with no fallback, notebook error, or halt. The existing busted-only rebuy policy is unchanged.

Global request and reported-spend limits apply across the whole batch; per-session limits cannot exceed the remaining allowance. Any halt or unknown cost stops the batch rather than moving on with a fresh allowance. As before, reported spending is known only after a request; the last request can exceed the dollar threshold. The baseline has a $0.50/600-request threshold, the memory pilot $0.25/400 requests. No automatic retries or silent model substitution.

The memory config crosses four conditions at seed 42/seat 0: none, notes, stats, both, ten hands each. This is a functionality pilot, not a powered memory-effect experiment. Seeds and seat schedules are matched across conditions, but model decisions are not deterministic, and behavior changes can alter subsequent stacks and actions.

Notebook mode makes one extra model call after each completed hand, unless already halted. It receives only that seat's authorized final view and prior notes. The model chooses keep or full replacement; replacement is limited to 4,000 Unicode characters by default, and generation to 1,024 tokens. Invalid/oversized/truncated updates retain the old notes and count as memory errors. Before/after text and revision are logged privately. Notes are not supplied to opponents or shared across sessions. No hidden summarizer edits them.

Statistics mode exposes only completed-hand counts per seat: dealt hands; preflop opportunities (hands with at least one preflop action); VPIP (hands with a voluntary preflop call/raise); PFR (hands with a preflop raise); public showdowns. VPIP/PFR use preflop opportunities as denominator, so players all-in solely from blinds are excluded. Showdown uses dealt hands. These are explicitly named conditional definitions, not a claim of compatibility with every poker tracker. No future hand, private cards, or current incomplete-hand aggregates enter the statistics. Notes and statistics are independent switches.

## Interpreting results and what remains

Net chips subtract starting chips and rebuys. Per-session bb/100 is net chips / 2 / hands × 100. Reports label the data descriptive: with only two seed blocks, uncertainty intervals would be fragile. Seats sharing a seed are matched observations, not independent deals. Do not turn a positive pilot result into a strength claim. A serious evaluation should use many independent seed blocks, prespecified opponents, and session/block-aware intervals before comparing model or memory effects.

The code now has the four requested capabilities and the six-seat same-model mode below. Heterogeneous model tables, statistical evaluation at scale, stronger opponent baselines, and container isolation remain unfinished. See the milestone 5 preparation document for the transport boundary and next implementation checks.

## Live validation, 2026-11-11

`runs/baseline-v1` completed all 12 sessions / 120 hands across seeds 42 and 314 and seats 0–5: 235 model requests, $0.01745677824 reported cost, zero fallbacks or early stops. Every hand passed deterministic replay.

`runs/memory-v1` completed all four conditions / 40 hands: 97 requests including 20 notebook requests, $0.014205532448 reported cost, zero action fallbacks or memory errors. All 20 notebook updates were accepted and every hand passed replay. Total reported spend across both batches was $0.031662310688. Logs remain local under Git-ignored `runs/`; inspect each session's private hand report and JSONL trace for details.

These runs validate operation, memory plumbing and auditability. They do not establish a profitable model or a beneficial memory effect. The automated suite passes 27 regular tests (including thousands of deterministic hands); the exhaustive five-card census remains available as an explicit ignored release test. Formatting and Clippy also pass.

## Six seats using the same model

```sh
cargo run -- openrouter examples/openrouter-six-models.json runs/six-models-v1
cargo run -- report runs/six-models-v1 0
```

`all_models: true` replaces every seat with a separate OpenRouter agent using the configured model and the same local API key. No scripted players participate. The supplied example uses six hands (one full button rotation), no memory, reasoning disabled, and a shared 300-request / $0.50 reported-spend threshold. The budget is for the whole table, not multiplied by six. A halt from any seat stops further network calls for all seats; the current hand finishes with explicit fallbacks and the run ends.

Each seat has its own observation, notebook/statistics state, and `model-seat-N-private.jsonl` trace. Sharing a key does not share conversation state: each request contains only the acting player's authorized context. Aggregate usage appears in `summary.json`; the manifest lists all model seats. `hand-history-private.txt` shows seat zero by default; use the report command with another seat number for that seat's cards. Agent state is logically separated within one trusted process, not protected by an OS sandbox. The batch comparison command intentionally rejects all-model mode because its results assume one focal model against a fixed field.

Live six-model validation (2026-11-11): `runs/six-models-v1` completed six hands with model calls from every seat, 45 total requests, $0.0031119512 reported cost, zero fallbacks, zero unknown costs, and successful replay of all six hands. No notebook/statistics condition was enabled. The automated suite now passes 28 regular tests, including a shared-budget/private-notebook regression test.

## Claude feedback (2026-11-11)

- **What the existing runs say.** Baseline: model net per session ranges from −36 to +115 chips while scripted seats swing by hundreds (one gained 775 in ten hands). The table's variance is dominated by the scripted opponents stacking off against each other, and the model is mostly a bystander: 29% VPIP, 88 folds in 235 decisions. Its mean of +29 bb/100 has a standard error of about 55, so it is indistinguishable from zero.
- **The memory pilot shows plumbing works and nothing else.** None, notes and stats were chip-for-chip identical; "both" moved the model by 2 chips. Details and suggestions are in the [memory feedback](04-memory.md).
- **The opponents are too wild to be a useful field.** The aggressive policy raises any pair and will commit a full stack with it; the loose policy calls up to half its stack with one pair. A model that simply waits for strong hands should beat them, which tests patience more than reads. Add at least one archetype with a specific, bounded leak (for example folds too often to river bets) and one reasonably solid baseline.
- **Add an `analyze` command.** Everything in this feedback was computed with ad-hoc scripts over `evaluator.jsonl` and the batch summaries. A Rust command that prints per-seat VPIP/PFR/aggression, fallback rate, bb/100 with a seed-block interval, and cost per hand would make every future run self-describing.
- **`clean` now means more.** Session summaries carry an explicit `clean` flag that also covers lifecycle errors, worker contamination and cleanup failures, and `batch` uses it.
- **Test count is stale above.** The suite is now 49 regular tests plus the ignored census.
