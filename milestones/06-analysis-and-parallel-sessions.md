# Milestone 6: Analysis and parallel sessions

**Status:** Done. Tooling implemented 2026-11-12; 10,070-hand baseline run 2026-11-13.

**Goal:** Make it practical to collect enough hands to say something, and to read a run without ad-hoc scripts.

- Run independent sessions concurrently under one host-owned request and spending budget. Keep hands within a session sequential.
- Add a Rust `analyze` command: per-seat playing style, fallback rate, cost per hand, and bb/100 with intervals computed over independent seed blocks, plus paired differences between conditions.
- Remove the orbit bias: batches require hand counts that are a multiple of the seat count. Offer per-hand stack resets.
- Move the session runner into the library so batches can be tested against a mock provider.

## Done when

- [x] A batch can run N sessions at a time on either backend with one shared budget, and stops cleanly when it is exhausted.
- [x] `analyze` reports style, cost and seed-block intervals for any existing run or batch directory.
- [x] Partial-orbit batches are rejected unless explicitly allowed.
- [x] A 10,000-hand baseline (`examples/experiment-baseline.json`) completes in under an hour and its interval is recorded.

## Baseline result (2026-11-13)

28 seeds × 6 seat rotations × 60 hands = 168 sessions, 10,070 hands, 21,970 requests, **$1.57**, about 20 minutes of play at 12 parallel sessions. 156 sessions clean; the other 12 each contain one model fallback (15 in total: illegal raise sizes and one malformed reply, about 1 per 1,500 decisions).

| Seat | bb/100 | 95% CI (28 seed blocks) |
| --- | --- | --- |
| adaptive | +110 | [+75, +146] |
| solid | +71 | [+32, +111] |
| **DeepSeek V4.1 Flash** | **+56** | **[+25, +88]** |
| tight | 0 | [-32, +32] |
| overfolder | -18 | [-57, +21] |
| station | -220 | [-267, -173] |

The model wins money from this field, but less than the two equity-based bots and far below the tuned ceiling (+288). Its style: VPIP 37%, PFR 18%, postflop aggression 0.43 (it calls far more than it bets), folds to 58% of postflop bets. It agrees with the `solid` rule's fold/passive/raise choice 60% of the time.

Variance for planning: one seed block (360 seat-hands) has SD ≈ 81 bb/100. An unpaired comparison needs about (2 × 1.96 × 81 / d)² seeds per arm to resolve a difference d: about 40 seeds for d = 50, 160 for d = 25.

**Incident:** the first attempt stopped at session 95 when one request got an HTTP 503 from OpenRouter's edge with no usage, and the strict unknown-cost rule halted the batch. Added `unknown_cost_allowance` (batches absorb a few such requests at a conservative $0.01 each; only the affected session stops) and `poker-lab batch CONFIG DIR --resume` (keeps finished sessions, re-plays incomplete ones on the same seeds, moves the old attempts to `superseded/`, counts earlier spend against the cap). The baseline was completed with `--resume`.

See [what was built and the live checks](../docs/11-analysis-experiments-and-opponents.md).

## Claude feedback (2026-11-12)

- (2026-11-13) The baseline is done; see above. The unknown-cost halt was the right default for a single run and the wrong one for a 20-minute batch; both behaviours now exist.
- The first live parallel batch ran 24 sessions in 59 seconds with no cross-session interference and every hand replaying. Throughput beyond six concurrent sessions is untested and will depend on provider rate limits, not on this code.
- I added one thing that was not in the proposal: opt-in acceptance of a `call` that repeats the call amount. It kept turning clear calls into folds in live runs.

## Qwen baseline (2026-11-14)

Same config and seeds 1 to 28, model `qwen/qwen3.6-35b-a3b` (`examples/qwen/baseline.json`): 168 sessions, 10,080 hands, 21,691 requests, **$0.46**, no session lost to infrastructure (3 failed requests, all retried).

| Seat | Qwen run, bb/100 | 95% CI |
| --- | --- | --- |
| adaptive | +141 | [+102, +179] |
| solid | +72 | [+29, +115] |
| tight | +54 | [+8, +101] |
| overfolder | +44 | [+6, +81] |
| **Qwen3.6-35B-A3B** | **-104** | **[-149, -60]** |
| station | -206 | [-264, -148] |

Qwen loses money here, against +56 [+25, +88] for DeepSeek on the same seeds: a paired difference of **-160 [-213, -108]**. Its style is very loose and aggressive (VPIP 48%, PFR 46%) and 2.7% of its decisions were fallbacks (DeepSeek 0.07%), nearly all "check while facing a bet". Details and the aid experiment are in [milestone 9](09-scaffold-treatments.md#qwen-comparison).

Harness changes made along the way: a model's own repeated faults now end only its session, not the batch (`max_consecutive_failures`, 0 disables; set to 0 for the Qwen runs so each plays all 60 hands), and provider preferences (`provider`: order, only, quantizations, allow_fallbacks, sort) are sent with every request.
