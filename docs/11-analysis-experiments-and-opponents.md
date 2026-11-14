# Analysis, parallel experiments, mixed models, and better opponents (milestones 6–8)

What is implemented for milestones [6](../milestones/06-analysis-and-parallel-sessions.md), [7](../milestones/07-mixed-models-and-memory-experiment.md) and [8](../milestones/08-opponents-and-exploitation-ceiling.md), and what the first small live checks showed. The tooling is built and tested; the full-size experiments have **not** been run.

## Commands

```sh
cargo run --release -- batch examples/experiment-smoke.json runs/smoke      # 24 short sessions, 6 at a time
cargo run --release -- analyze runs/smoke                                  # add --json for machine output
cargo run --release -- ceiling station,overfolder,solid,adaptive,tight 8 40 runs/ceiling.json
```

## Parallel batches (milestone 6)

- `"parallel": N` in a batch config runs N independent sessions at once, each with its own directory, agents and (for `execution: "subprocess"`) its own workers. Hands inside a session stay strictly sequential.
- One host-owned budget covers the whole batch. A request slot is reserved before each provider call and the cost is settled after the response, so up to one in-flight request per concurrent session can land after the dollar threshold is crossed. A halt, unknown cost or session error stops new sessions from starting.
- Batches now refuse hand counts that are not a multiple of the seat count (`allow_partial_orbits: true` overrides). The two original batch examples were changed from 10 to 12 hands.
- `"reset_stacks": true` starts every hand at 100 BB in every seat, which makes hands much closer to independent. Net chips are now summed per hand from the replay records in both modes.
- `"normalize_call_amount": true` accepts `{"action":"call","amount":N}` when N is exactly the call amount and counts it in `normalized_actions`. This malformed-but-unambiguous answer has occurred in three of four live runs; without the option it becomes a fold. It is off by default and on in the new experiment configs.
- The session runner moved from `main.rs` to `src/runner.rs` so it can be tested with a mock provider.

## `analyze` (milestone 6)

Reads one run directory or a batch directory and replays every hand. It works on all earlier runs.

- **Playing style per seat label** (model ID or opponent style): VPIP, PFR, aggression factor (postflop bets and raises per call), fold-to-bet, showdown rate, fallbacks, bb/100.
- **Win rate with a 95% interval** per condition and label. The unit of replication is the **seed**: all seat rotations of a seed share its deals, so they are averaged into one block and the t-interval is taken over blocks. One block gives a mean and no interval.
- **Paired differences** between each condition and the first one, on the seeds both played.
- Requests, reported cost, and cost per hand.

## Mixed-model tables and memory conditions (milestone 7)

- `"models": [six IDs]` with `"all_models": true` seats a different model in each seat. In a batch, `model_seats` are rotations of that list, so every model plays every position on every seed. `analyze` then reports one row per model. The shipped `experiment-mixed-models.json` lists the same DeepSeek ID six times as a placeholder: put real IDs in before running it. All seats share one set of request settings (format, reasoning, token limit).
- `"notebook_style": "profiles"` switches the post-hand prompt to version 3: one evidence-counted profile per opponent seat, no hand log. The default remains the version-2 journal prompt so earlier runs stay comparable.
- `"memory": "oracle"` gives the model an accurate, experimenter-written description of each scripted opponent as its notebook, and never asks it to update. It needs one model seat and a scripted field. It measures whether the model can *use* correct reads, separately from whether it can form them.
- A notebook answer of `keep` with stray text is now treated as keep rather than an error.
- **Not built: reflection without persistence.** Every model call here is stateless, so a reflection that is discarded cannot affect any later decision. That condition is the no-memory condition by construction.

## Opponents and the ceiling (milestone 8)

Four new scripted styles, selectable with `"opponent_field": [...]` (five entries; the model takes the sixth seat). They run on both backends.

| Style | Behaviour | Intended exploit |
| --- | --- | --- |
| `station` | Never bets or raises. Never folds a pair or ace-high. | Value-bet big; never bluff. |
| `overfolder` | Loose and calls flop and turn, but folds anything below two pair to every river bet. | Bluff rivers. |
| `solid` | Compares Monte Carlo equity against one random hand with the pot odds; bets about two thirds of the pot for value; bluffs 15% of weak hands. | None large. |
| `adaptive` | `solid`, plus one published rule: after eight observed responses to its postflop bets, a fold rate above 60% raises its bluffing to 80%; below 30% it stops bluffing and value-bets thinner. | Depends on how the table has treated it. |

`solid` is a baseline with no deliberate leak, not a strong or game-theoretic player. Equity is against a single random hand regardless of how many opponents remain.

`ceiling` searches 162 settings of the same tunable policy against a field (hero rotated through all six seats, stacks reset every hand), picks the best on training seeds, and re-measures it on held-out seeds, because the training maximum is biased upward by selection. The result is a **lower bound** on the true best response: only this policy family is searched.

| Field | Best found, held out (40 seeds, 14,400 hands) | Default `solid`, same seeds |
| --- | --- | --- |
| Original (tight, loose, aggressive, tight, loose) | +360 bb/100, 95% CI [+315, +405] | +152 [+123, +181] |
| New (station, overfolder, solid, adaptive, tight) | +288 bb/100, 95% CI [+252, +325] | +109 [+84, +135] |

Against both fields the best setting opens wide, value-bets thin and large, and calls tightly. Against the original field it also bluffs 80% of the time; against the new field it never bluffs. Result files: `runs/ceiling-default-v1.json`, `runs/ceiling-field-v2.json`.

## Live checks, 2026-11-12

DeepSeek V4.1 Flash, reasoning off. Total reported spend $0.0378.

**`runs/experiment-smoke-v1`** — 2 seeds × 6 seats × {none, oracle}, 6 hands each, new field, 6 sessions at a time.

- 24 of 24 sessions, 144 hands, 296 requests, $0.0277, 59 seconds wall-clock. Every hand replays.
- 22 sessions clean. The two fallbacks were both `{"action":"call","amount":4}`; this run predates the normalization option.
- Model style: VPIP 38%, PFR 16%, fold-to-bet 61%.
- Win rates are meaningless at this size: the model's interval with two seed blocks is roughly ±1,600 bb/100. The run shows the pipeline works end to end, nothing more.

**`runs/profile-notes-v1`** — one model with profile notes against the new field, 18 hands, subprocess backend.

- 40 requests (18 notebook), $0.0101. 14 notebook updates accepted, 4 rejected: three exceeded the 2,000-character cap and one sent `keep` with text (now accepted).
- The notes are per-seat profiles with hand counts, not transcripts. The profile of the station seat reads "loose-passive, calls too much … value-bet thin, avoid bluffing", which matches its actual policy.
- Weaknesses: the model labels seats with positions that change every hand, and it runs into the size cap because it cannot count characters.

## Retries and outages (2026-11-13)

Infrastructure faults must not change the game; model faults are part of what is measured.

- **Per-decision retry.** `max_attempts` (default 6; 1 disables), `retry_backoff_ms` (500) and `retry_backoff_max_ms` (30,000) in the pilot config. Retried: connection errors, timeouts, HTTP 408/429/5xx, and upstream error bodies. Backoff doubles with 75–125% jitter and honours `Retry-After`. Never retried: malformed, truncated or illegal replies, and 400/401/402/403/404.
- **Billing.** A refusal that carries an HTTP status and no usage generated nothing and counts as free. A timeout or dropped connection may have been billed, so it is charged an assumed $0.001 (`assumed_cost_usd`, counted against the spend limits) instead of halting the run. A reply that succeeds but omits its cost is still a strict halt.
- **Both backends.** The host now grants up to `max_attempts` slots per operation, refunds the unused ones, and sets the operation deadline to the worst case (all attempts time out, all backoffs at maximum jitter). Every attempt is a line pair in the audit trace with its attempt number.
- **Outage.** If every attempt fails on infrastructure, the decision returns `provider_outage`. The hand is abandoned (no fallback decision is recorded, nothing partial is written), the session ends incomplete, and its summary has `"outage": true`.
- **Batch circuit breaker.** On an outage the batch pauses new sessions (`outage_pause_ms`, default 30 s, doubling on repeats, shared by all sessions), moves the interrupted session to `superseded/session-NNN-outage-K`, and re-runs it. After `outage_max_wait_s` (default 900) of pausing it stops with `provider_outage`; `--resume` continues.
- **Invariant, tested on both backends:** the same seed against a provider that refuses every third and seventh request produces a byte-identical `evaluator.jsonl` and the same reported spend as a clean run.
- **Not done:** hand-level checkpointing (sessions cost about a cent, so re-running one is cheaper than the machinery), and moving the retry loop into a host-owned inference proxy (milestone 10).

## Provider preferences and model faults (2026-11-14)

- `provider` in the pilot config (`order`, `only`, `quantizations`, `allow_fallbacks`, `sort`) is forwarded to OpenRouter on every request and recorded in the manifest. Without it OpenRouter routes freely, so one run can mix providers and weight precisions; unpinned DeepSeek cost about 3.6x the cheapest compatible provider.
- `max_consecutive_failures` (default 3, 0 disables): after this many model faults in a row a session stops. That halt no longer stops the rest of a batch. Set it to 0 to measure what a model's faults cost it (each becomes a fallback fold) instead of truncating its sessions.

## Running the full experiments

Not run; these cost real money and time. Estimates use the per-hand costs measured above and the smoke run's throughput, and assume the provider does not rate-limit 12 concurrent sessions.

| Config | Size | Estimated cost | Estimated time |
| --- | --- | --- | --- |
| `experiment-baseline.json` | 28 seeds × 6 seats × 60 hands = 10,080 hands | **ran 2026-11-13: $1.57, about 20 minutes; see milestone 6** | |
| `experiment-scaffold.json` | 4 scaffolds × 8 seeds × 6 seats × 60 hands = 11,520 hands | **ran 2026-11-13: $2.17; see milestone 9** | |
| `experiment-memory.json` | 5 conditions × 4 seeds × 6 seats × 120 hands = 14,400 hands | about $5–6 (cap $12) | 1–2 hours |

The memory experiment has four seed blocks per condition. That is enough to see whether the paired differences are large, not to resolve a small one; add seeds if the first pass is inconclusive.

## Verification

`cargo test` runs 78 tests plus the ignored census (59 before milestone 9; the rest cover scaffolds, resume, the unknown-cost allowance, retries, outage handling and the batch breaker). New coverage: parallel batches on both backends against a mock provider, with the shared ledger matching the provider's request count; the batch budget stopping concurrent sessions; model rotation in mixed tables; oracle notes and the profile prompt reaching only the model; every opponent style being legal, deterministic and replayable; both leaks behaving as documented; the adaptive rule; equivalence of the new styles across backends, including the stateful adaptive worker; ceiling determinism; and interval arithmetic.

## Known limits

- `analyze` has no variance reduction yet. All-in equity adjustment against the scripted field is the obvious next step.
- Mixed tables share one set of request settings; a model that does not support `json_schema` or the reasoning switch needs its own.
- The profile prompt has had one 18-hand live trial.
- The parallel scheduler uses threads in one process. It has run 12 concurrent live sessions (in-process backend) without problems; subprocess batches above 6 are still untested live.
- Interrupted batches resume with `poker-lab batch CONFIG DIR --resume`; set `unknown_cost_allowance` so one edge 503 does not halt a long batch (it did once, at session 95 of 168).

## Claude feedback (2026-11-12)

- **Run `experiment-baseline.json` first.** It is the cheapest way to replace the current guesswork (one 120-hand batch) with a real variance figure for this field, and it tells you how many seeds the memory experiment actually needs.
- **Compare the model with the ceiling, not with zero.** A model at +30 bb/100 against a field where a simple tuned bot makes +290 is leaving most of the money on the table. "Fraction of ceiling captured" is the headline number I would report.
- **The oracle condition is the most informative single comparison available.** If correct notes do not move the model's win rate, better note-taking prompts will not either, and effort should go into how reads are presented and used.
- **`solid` lost to `station` in the smoke run.** Over 14,400 hands `solid` is clearly profitable against this field, so that was noise, but it is a reminder that 144 hands can show anything.
- **Positions in notes.** The model wrote "Seat 1 (SB)" as if the position were permanent. Adding an explicit position label to the observation (already suggested in the architecture feedback) would likely fix it.
