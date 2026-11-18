# Roadmap

What has been built and measured, and what is left, most important first. Results are in [docs/results.md](docs/results.md).

## Where the project stands (2026-11-18)

- **Built:** a verified six-seat engine; model seats on OpenRouter in-process or as supervised worker processes; a five-bot field with known exploits and a tuned ceiling; memory, decision-aid, sizing and thinking treatments; parallel batches with one budget, retries and resume; analysis with luck removal, call/fold, river (exact) and turn (approximate) grades; cross-run comparison and a run catalog; a spectator table.
- **Measured:** 85,755 hands across 60 runs, $16.64 reported. DeepSeek V4.1 Flash beats the field but captures a fraction of what a tuned bot wins; computed facts, opponent information and an explicit sizing rule help in that order of certainty; thinking is the biggest lever; one other thinking model (GLM-5.3 Flash) behaves the same way.

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

## Open gaps (most important first)

1. **Easy, fixed opponents.** Every result is about exploiting five deterministic bots. Next: run the model against the `adaptive` bot with memory (does it notice when the bot adjusts?), and seat several models at one table (milestone 7's unfinished item).
2. **Only two thinking models.** DeepSeek and GLM agree; gpt-oss-20b failed on output format; Gemma had to run with thinking off. Widening needs models that honour low effort, or a reasoning budget the provider respects.
3. **Small samples.** 10-20 seeds per arm, with many arms compared. Use fresh seeds for every claim; prefer one pre-registered comparison over many.
4. **Grading is partial.** River exact, turn approximate, flop and preflop bets ungraded (call/fold only).

## Next experiments

| Experiment | Why | Rough cost |
| --- | --- | --- |
| Sharp vs plain sizing rule on fresh seeds 21-30 | Leans better (river −60 [−126, +7]); settle it | $0.80 |
| GLM-5.3 Flash with the sharp rule and with the cheat sheet | Does the river fix carry over to a second model? | $0.30 |
| Adaptive bot vs the model with notes | The counter-adjustment question from the original vision | $0.50 |
| Real mixed-model table (DeepSeek, GLM, ...) | Milestone 7's open item; needs per-seat request settings | $0.50 |
| Per-feature ablation of the facts sheet | Which fact matters (position names and pot odds are suspects) | $3 thinking off |
| `equity` vs `features`, `advice` vs `equity` with 4x seeds | Resolve whether a calculator adds anything | $8 thinking off |
| Strong-advice arm (tuned ceiling policy as advice) | Can the model recognise good advice? | $1 |
| Own notes vs none with thinking, about 15 more seeds | The one memory question still open | $2 |
| Quick (thinking-off) notes, live | Built, never run; would make notes cheap | $0.30 |
| Qwen 3.6 with `repair_illegal` | Is Qwen's loss only its illegal-check fault? | $0.20 |
| Gemma tracker arm, seeds 16-20 | Finish the 5-seed arm only if Gemma matters | $0.05 |

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
