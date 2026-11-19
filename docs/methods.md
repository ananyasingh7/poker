# Methods

How experiments are run, measured and graded. Everything is a `poker-lab` subcommand (Rust only); `cargo run --release -- <command>` from the repository root.

## Running

```sh
poker-lab openrouter CONFIG.json NEW_DIR            # one session (single-run config)
poker-lab batch BATCH.json NEW_DIR                  # many sessions in parallel, one shared budget
poker-lab batch BATCH.json EXISTING_DIR --resume    # continue an interrupted batch
poker-lab report RUN_DIR [SEAT]                     # readable hand history (SEAT adds its cards)
poker-lab replay RUN_DIR/evaluator.jsonl            # verify every hand
```

- **Credentials:** `OPENROUTER_API_KEY` or the git-ignored `.openrouter-api-key`. Never in configs or logs.
- **Configs** live in `examples/<topic>/`. A batch config has a `pilot` block (the single-run settings) plus `seeds`, `model_seats` (seat rotations), `conditions` (memory modes), `scaffolds`, `parallel`, `max_requests` and `spend_limit_usd`. Hand counts must be whole orbits (a multiple of the seat count).
- **Budget:** one host-owned ledger per batch. A request slot is reserved before each call and the cost settled after, so the last in-flight requests can land slightly past the cap. Set a key-level limit on OpenRouter for a hard ceiling.
- **Faults:** infrastructure failures are retried per decision (`max_attempts`, exponential backoff, `Retry-After`). A provider outage abandons the hand and re-runs the session after a pause (`outage_pause_ms`). A few replies without a reported cost are absorbed at a charged estimate (`unknown_cost_allowance`). A model's own faults become fallback folds/checks; `max_consecutive_failures` ends only that session.
- **Resume caveat:** `--resume` counts the spend of finished sessions only. Sessions killed mid-way are re-run and their earlier spend is not counted, so track real spend with OpenRouter's `/api/v1/credits` when resuming.
- **Provider pinning:** `provider.order` / `quantizations` / `allow_fallbacks`. Unpinned routing mixed providers and cost DeepSeek about 3.6x the cheapest compatible one.
- **Long batches** in an agent's shell need a long timeout (background jobs are killed at 30 minutes by default), or use `--resume` after.
- **Smoke test first:** about $0.02 and 10 hands per new model or setting, checking faults, truncation, cost per decision and that the parameters are honoured.

## Measuring

```sh
poker-lab analyze RUN_OR_BATCH [--json] [--no-grades] [--turn] [--decisions]
poker-lab compare BATCH... [--seat N] [--turn] [--json] [--save FILE.json]
poker-lab catalog RUNS_DIR > results/runs.csv
poker-lab ceiling [default|current|STYLE,...] [TRAIN_SEEDS] [TEST_SEEDS] [OUT.json] [--varied] [--check-seeds A-B --check-seat N]
```

- **`analyze`**: per seat label, playing style (VPIP, PFR, aggression = postflop bets and raises per call, fold to bet, showdowns, fallbacks), bb/100 with intervals, luck-adjusted bb/100, call/fold mistake cost, river (and with `--turn`, turn) cost, a leak report, cost per hand, and paired differences of each condition against the first. `--decisions` prints one JSON line per model decision with its spot, equity and grades (evaluator-only: it reveals hidden cards).
- **`compare`**: several batch directories side by side, one row per arm named `model · thinking · condition`; the same treatment on different seeds is merged; only complete seeds count; paired differences of each helper against the same model's no-help arm and of each model against the first directory's model. Columns include VPIP and river decisions per 100 hands, because costs are per 100 hands *played*: a model that folds more shows smaller costs.
- **Learning curves** (both commands): every seat's luck-adjusted result, river and turn cost by 20-hand segment of the session (hands 1-20, 21-40, ...), and the last segment minus the first, paired by seed. The cards differ by hand number, so a single arm's curve mostly shows the deals; the learning measure is the **difference in differences**: an arm's (late minus early) minus the no-help arm's (late minus early) on the same seeds. `analyze` reports it for each condition against the first; `compare` in its "Learning relative to no help" table. A 20-hand slice is very noisy on chips (intervals of ±300 bb/100 at 10-20 seeds); the river and turn grades are the better signal. A model that learns within its first few hands (notes profile the station in 1-4 hands) shows that gain in every segment, not as a curve.
- **`ceiling`**: the best of 162 settings of a tunable bot, chosen on training seeds and re-measured on 40 held-out seeds, the hero rotating through all six seats. `default` is the *original* tight/loose/aggressive field; `current` is the station, overfolder, solid, adaptive, tight field every experiment since 2026-11-13 uses. `--varied` uses the experiments' less predictable bots (personalities, mixing 0.1, field drawn per seed from FIELD); `--check-seeds 21-40 --check-seat 0` also measures the chosen setting on an experiment's exact deals and seat.
- **`catalog`**: one CSV row per run or batch (model, thinking, bots, help, seeds, completion, requests, cost).
- **Varied bots** never merge or pair with fixed-bot runs: `compare` names such arms `... · varied (personalities, mix 0.1, random field) bots` and pairs them only with arms on the same kind of field.

### Statistics

- **Unit of replication: the seed.** All seat rotations of a seed share its deals, so they are averaged into one block; intervals are 95% t-intervals over blocks.
- **Paired comparisons** difference two arms seed by seed on the same deals. With temperature 0 the model's play is close to deterministic, so pairing removes most card variance (control and facts-sheet blocks correlated at 0.55).
- **Power:** one 360-hand block has SD ≈ 81 bb/100 (thinking off). An unpaired difference of 50 bb/100 needs about 40 seeds per arm; paired designs and the grades below need far fewer.
- **Don't tune on your test cards.** Seeds 1-10 were reused across many helper arms; the best helper's river effect fell from −200 to −82 on fresh seeds 11-20. Hold out fresh seeds for any claim.

### Removing luck: all-in adjustment

When the money goes in before the river, the remaining board cards no longer depend on decisions. The adjusted result replays the hand's actions on every possible run-out (exact for one card, 400 seeded samples for more) and averages. Unbiased, never noisier; narrowed the baseline interval by about a quarter. Most remaining noise is river showdowns, which it does not touch.

### Grading decisions

All grades are evaluator-only (they use hidden information) and are meant for comparing conditions on the same deals, not as absolute verdicts.

| Grade | What is graded | Benchmark | Caveat |
| --- | --- | --- | --- |
| Call/fold | Every call or fold facing a bet | Pot odds vs equity against the cards opponents actually held | Hindsight about hands; ignores later betting; bets and raises not graded |
| **River** (`src/river.rs`) | Every river decision | A player who knows exactly how each bot plays but not its cards | Exact: bots are deterministic, so their possible holdings are computable |
| **Turn** (`src/turn.rs`, `--turn`) | Every turn decision | Same, approximate | Later play is a stand-in; optimistic |

**River grade.** For each live bot, every hole-card pair the model cannot see, in both deal orders, is kept if the bot's policy would have made every move it made (exact ranges; both orders because a bot whose randomness hashes its cards can play [A, B] and [B, A] differently: fixed 2026-11-18, moving earlier costs by 1-9 bb/100). With varied bots the grader rebuilds each bot with its session's draws. Each alternative (check/fold, call, bets of 33-150% pot, all-in, and the action taken) is played out over every joint holding (up to 400) with the bots' real responses; if a bot raises back, the model takes the better of fold and call for what it saw. The grade is the expected stack of the chosen action against the best one (the **cost**), and the mistake kind: missed bet, bad bet, sizing, bad call, bad fold, missed raise, bad raise. Against a station the best river bet is often all-in, so "river cost" is distance from perfect exploitation, not from sound poker.

**Turn grade.** As above, plus: every river card the model cannot rule out is tried with 12 opponent holdings each; the model's later play is the best of check, pot bet, all-in (when it can check) or fold, call (when facing a bet) for what it saw. With few deals behind each river card that continuation is optimistic, equally for every alternative. About 1 minute per 1,200 hands; sessions are graded in parallel.

**Leak report:** missed river bets, bluffs (postflop bets with under 30% equity against the cards held) and how often they were called down, and truncated answers.

## Default setup (since 2026-11-18)

New experiments use **GLM-5.3 Flash** (`z-ai/glm-5.3-flash`, thinking on with `reasoning_effort: low`, providers Wafer then Darkbloom) against **varied bots** (`opponent_variation`: personalities, mixing 0.1, random field from the five current styles), on **fresh seeds** (21 onward), with **one DeepSeek V4.1 Flash anchor arm** on the same deals so results connect to sections 1-6 of the results. Templates: `examples/7-glm-varied/`. The spectator table defaults to the same model and to VARIED bots.

## Picking models

A model can join a comparison when it (1) accepts the strict JSON schema or `json_object`, (2) honours `reasoning_effort: low` if thinking is on (otherwise its reasoning runs to thousands of tokens), and (3) is allowed by the account's privacy settings (zero data retention, no training: these block Qwen 3.7 Flash and Meta Muse). Per-decision costs measured so far: DeepSeek V4.1 Flash with thinking low about $0.00015 per request on DeepInfra; GLM-5.3 Flash $0.00003; gpt-oss-20b $0.00002 (but 4-9% empty answers); Gemma 4, Qwen 3.6, Nemotron 3 Super, Seed 2.0 mini and Mistral Small ignore low effort and cost $0.00024-0.00048.
