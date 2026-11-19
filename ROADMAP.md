# Roadmap

What has been built and measured, and what is left, most important first. Results are in [docs/results.md](docs/results.md).

## Where the project stands (2026-11-19)

- **Built:** a verified six-seat engine; model seats on OpenRouter in-process or as supervised worker processes; a five-bot field with known exploits, now also in a less predictable version drawn per seed, with tuned ceilings for both; memory, decision-aid, sizing and thinking treatments; parallel batches with one budget, retries and resume; analysis with luck removal, call/fold, river (exact) and turn (approximate) grades and learning curves by hand number; cross-run comparison and a run catalog; a spectator table.
- **Measured:** 92,967 hands across 67 runs, $17.67 reported. DeepSeek V4.1 Flash beats the field but captures a fraction of what a tuned bot wins; computed facts, opponent information and an explicit sizing rule help in that order of certainty; thinking is the biggest lever. Against varied bots, GLM-5.3 Flash (the new workhorse) is weaker than DeepSeek, gains most from the facts sheet, and gets nothing measurable from its own notes. No model has yet been shown to get better over a session.

## Milestones

| # | Milestone | Status |
| --- | --- | --- |
| 1 | Rules and interfaces | Done |
| 2 | First playable hand | Done |
| 3 | Six-player rules and reliability | Done |
| 4 | Model agents (OpenRouter), notes, statistics | Done |
| 5 | Native subprocess participants | Done |
| 6 | Analysis and parallel sessions | Done (10,070-hand baseline) |
| 7 | Mixed-model tables and memory experiment | Memory run done; **no real mixed-model table yet** |
| 8 | Better opponents and exploitation ceiling | Done |
| 9 | Decision aids (scaffolds) | Done (pilot) |
| — | Thinking, grading, sizing, model lineup (Nov 15-17) | Done |
| 10 | Isolation: host-owned inference proxy, then containers | Backlog |

## Main goal: how models learn

The question the user cares most about is how each model *learns*: does it play better after more hands, and which mechanism gets it there? So far the lab measures mostly static help (facts, profiles handed to it, sizing rules); the learning arms (own notes, tracker) improve river decisions but show no resolved chip gain, and no model has yet faced an opponent that changes. The learning track, in order:

1. **Learning curves.** *Done 2026-11-19* (`analyze`, `compare`: 20-hand segments, late minus early, paired). Result: no arm on any model measurably improves over a session; the right control for notes is the cheat sheet, not no help. Next: replicate the one hint (GLM notes closing on the cheat sheet in chips) on fresh seeds, with more seeds and longer sessions.
2. **Within-session learning against a changing opponent.** The `adaptive` bot switches style after it sees how it is treated. Does a model with notes notice the switch and adjust? (The original "induce a counter-adjustment" idea.)
3. **Across-session learning.** Let notes persist from one session to the next (a defined identity per bot), and measure session 1 vs session 5 on fresh cards.
4. **Learning through training.** Fine-tune a model on graded decisions (the river and turn grades supply labels: the best action per spot), then test it at the table against its base model on fresh seeds. Local first (MLX on the Mac Studio).
5. **Compare models on the same learning curves**, not only on end totals.

## Requested additions (2026-11-18)

- **Play as a human at the table.** A HUMAN player choice in the spectator table: you take a seat, see only your own cards, and act with buttons and a bet-size input, against the bots and/or model seats. Your hands are saved in lab format, so `analyze` grades your decisions too (a human baseline on the same deals).
- **Local models through Pi.** Run the `qwen3.8-pi` model (set up in Pi) as a table and lab player. Needs an `endpoint` option in the model config so a seat can call a local OpenAI-compatible server instead of OpenRouter (the adapter already supports an endpoint override in tests). Pi's config lists local servers at `127.0.0.1:8000` (omlx) and `localhost:9337` (mesh); neither was running on 2026-11-18, so the model ID and server are still to be confirmed. Local runs cost nothing per hand, so they suit large seed counts.
- **Does the harness matter?** The same model through (a) the lab's direct, stateless request and (b) an agent harness such as Pi (its own system prompt, tools, conversation state). Paired on the same seeds: does the harness change win rate and grades, or is the difference within noise? Report noise explicitly (repeat the same arm twice to measure run-to-run variation).
- **Fine-tune a model and seat it at the table.** See learning track item 4.

## Model and field going forward (decided 2026-11-18)

- **GLM-5.3 Flash is the workhorse.** It wins as many chips as DeepSeek at about a fifth of the cost; it makes more graded mistakes, which leaves room to show improvement. **DeepSeek V4.1 Flash stays as one anchor arm** in key comparisons so new results connect to the old ones. GLM has never been tested with the facts sheet, notes or cheat sheet: the first GLM batch covers that.
- **Varied bots are the default for new experiments** (`opponent_variation`: personalities, mixing 0.1, random field), so a model must learn each session's opponents rather than reuse a fixed profile. Old results were on the fixed field and are not directly comparable; the anchor arm bridges them.
- Templates: `examples/7-glm-varied/` (GLM: none, facts, oracle, stats + sharp rule, notes every 5; DeepSeek anchor; a 12-hand smoke test). First results: [results §7](docs/results.md#7-glm-against-varied-bots-2026-11-18-to-11-19).

## Open gaps (most important first)

1. **Simple opponents.** The bots now vary per session (2026-11-18), but they are still simple, and the varied field is slightly *easier* to exploit than the fixed one (ceiling +313 vs +288). Next: the `adaptive` bot with memory (does the model notice when it adjusts?), several models at one table, a human seat, and variation that makes bots tighter as well as looser.
2. **Only two thinking models.** DeepSeek and GLM agree; gpt-oss-20b failed on output format; Gemma had to run with thinking off. Widening needs models that honour low effort, or a reasoning budget the provider respects.
3. **Small samples.** 10-20 seeds per arm, with many arms compared. Use fresh seeds for every claim; prefer one pre-registered comparison over many. Learning curves need more: a 20-hand slice is ±300 bb/100 on chips at 20 seeds. GLM makes 40+ seeds affordable (about $0.004 per session).
4. **Grading is partial.** River exact, turn approximate, flop and preflop bets ungraded (call/fold only).

## Next experiments

Done 2026-11-18/19: learning curves, varied-field ceiling, GLM smoke, first GLM batch on varied bots, GLM notes vs none ($1.08). See [results §7](docs/results.md#7-glm-against-varied-bots-2026-11-18-to-11-19).

| Experiment | Why | Rough cost |
| --- | --- | --- |
| Sharp vs plain sizing rule on fresh seeds 21-30 | Leans better (river −60 [−126, +7]); settle it | $0.80 DeepSeek, about $0.15 with GLM |
| **Better notes for GLM**: host-kept per-seat counts inside the notebook (notes + stats), or updates every 2-3 hands | GLM's notes rest on 2-4 hands of evidence after 60; the clearest "improve the weaker model" target | $0.20 |
| **Pre-registered learning replication**: GLM notes vs cheat sheet, 40 fresh seeds (41-80), 60-hand sessions ([results §8](docs/results.md)) | Test the one learning hint (+393 chips late vs early) with the right control and enough power | $0.62 (running 2026-11-19) |
| Facts sheet + tracker + sharp rule together for GLM | Facts fix the turn, tracker + rule the river: do they add up? | $0.10 |
| Adaptive bot vs the model with notes | The counter-adjustment question from the original vision | $0.20 |
| Across-session notes (learning track item 3) | Notes carried from session to session: does session 5 beat session 1? Needs a persistent bot identity per seed | $0.30 + build |
| Real mixed-model table (DeepSeek, GLM, ...) | Milestone 7's open item; needs per-seat request settings | $0.50 |
| Per-feature ablation of the facts sheet | Which fact matters (position names and pot odds are suspects); cheap with GLM | $0.40 |
| Strong-advice arm (tuned ceiling policy as advice) | Can the model recognise good advice? | $0.20 |
| Quick (thinking-off) notes, live | Built, never run; would make notes cheap | $0.10 |
| Qwen 3.6 with `repair_illegal` | Is Qwen's loss only its illegal-check fault? | $0.20 |

## Tooling and engineering

- **Per-seat request settings** for mixed tables (response format, reasoning, provider per model).
- **Host-owned inference proxy** (milestone 10): holds the key, enforces the budget before each request, owns retries; then containers if third-party agents arrive.
- **Grade flop bets** (needs a model of the seat's own later play, as the turn grade does).
- **Provider per session in `analyze`** (the audit trace has it).
- **Private random number** in the observation so a temperature-0 model can mix strategies; a separate condition.
- **Engine confidence:** a dev-only differential test against an independent evaluator; golden hand histories in the repo; a PokerStars-style export; JSON Schema for the protocol types; rename `PlayerView.protocol` to `view_schema`.
- **Untested at scale:** more than 6 concurrent subprocess sessions, Linux, hand-level checkpointing (sessions are cheap to re-run).

## Decisions on record

- Rust only; no Python in the project.
- Thinking on with `reasoning_effort: low` is the default for new work (bounded deliberation; truncations were the costliest fault without it).
- Structured per-seat notebook profiles (prompt v5) are the default for notes.
- The table's BET SIZING is the sharp rule alone (the bet menu never showed an effect).
- Account privacy settings (zero data retention, no training) stay as they are; models they block are out of scope.
- Budget discipline: quote a cost and a hard cap before every live run; smoke-test new models for about $0.02.
- GLM-5.3 Flash is the workhorse; DeepSeek is the anchor (2026-11-18).
- New experiments run against varied bots on fresh seeds (2026-11-18).
- River and turn ranges consider both deal orders of a bot's cards (grader fix, 2026-11-18).
