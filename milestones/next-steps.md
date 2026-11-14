# Next steps (consolidated 2026-11-13)

Everything intended but not yet done, including items from earlier plans. Update as items complete.

## Experiments
1. ~~Qwen3.6-35B-A3B across all milestones~~ **done 2026-11-14**: pilots (M4/M5), mixed smoke (M7), 28-seed baseline (M6) and scaffold experiment (M9) are in `runs/qwen-*`; results in [milestone 9](09-scaffold-treatments.md#qwen-comparison) and [milestone 6](06-analysis-and-parallel-sessions.md). Headline: unaided Qwen loses -104 bb/100 (DeepSeek +56), the `features` aid lifts both to about +100 to +150.
2. ~~Memory experiment~~ **run 2026-11-14 on a $6 budget** (none, notes, oracle; 8 seeds; $4.92): oracle +33 [+2, +65] over none; own notes +24 [-5, +52], unresolved. See [milestone 7](07-mixed-models-and-memory-experiment.md#results-2026-11-14-runsmemory-budget-v1). Notebook overrun fix is **implemented** (`structured_profiles`, prompt v5; needs a live check). Remaining: run `examples/experiment-memory-structured.json` (about $2.50), then about 3x seeds for notes vs none; the dropped `stats` and `both` arms.
3. **Per-feature ablation** (milestone 9): each decision aid alone (position, pot odds, stack depth, hand notation, made hand/draws), about 5 arms x 8 seeds.
4. **Resolve `equity` vs `features` and `advice` vs `equity`**: about 4x the seeds (32).
5. **Strong-advice arm**: advise with the tuned ceiling policy instead of the untuned `solid` rule, to test whether the model can recognise good advice.
6. **Real mixed-model table** (milestone 7): several different models in one table. Needs model IDs; candidates from the research are glm-5.3-flash, gemma-4-26b-a4b, llama-4-scout, a Meta Muse model, qwen3.6-35b-a3b, deepseek-v4.1-flash-1017.
7. **Smoke-test remaining candidates** (about $0.02 each): gemma-4-26b-a4b, llama-4-scout, Muse models, glm-5.3-flash, nemotron-3.5-lightning, gpt-oss-20b. Check fallback rate, truncation, whether `reasoning_enabled: false` is accepted.
8. **Adaptive opponent vs a model with memory**: does a model that calls the adaptive bot down early stop its bluffing? Nothing run yet.

2b. (Paused: user chose DeepSeek only, 2026-11-14.) Memory experiment on Qwen.
2c. **Repair for the "check while facing a bet" fault** (Qwen: 2.7% of decisions): clamp to call/fold or one re-ask, as an opt-in labelled treatment, then re-measure Qwen's unaided score.
2d. **Qwen with more seeds / DeepSeek pinned**: re-run DeepSeek pinned to one fp8-class provider so both models are compared with provider held fixed.

## Tooling and measurement
9. **Variance reduction**: all-in equity adjustment against the scripted field; AIVAT-style estimators from the GTO Wizard paper.
10. **Per-seat request settings** for mixed tables (reasoning switch, response format, provider preferences per model).
11. **Position labels and blinds in `PlayerView`** itself (the `features` scaffold supplies them as an aid, but the raw observation still lacks them).
12. **Raise-size clamp / repair re-ask** as opt-in, labelled treatments for illegal-size replies.
13. **Notebook length**: models overrun the 2,000-char cap; tell the model the length or raise the cap.
14. **Provider mix in analysis**: report which provider served each session (the audit trace has it; `analyze` does not).
15. **Scale checks not yet done**: more than 6 concurrent live subprocess sessions, Linux.

## Reliability
16. **Hand-level checkpointing** (only if sessions get long or models expensive); sessions are re-run whole today.
17. **Retry loop in a host-owned inference proxy** (milestone 10).

## Backlog milestones
- **Milestone 10: isolation.** Host-owned inference proxy (holds the key, enforces the budget and retries), then containers only when third-party agents are in scope.

## Housekeeping
- Nothing is committed. `Cargo.lock` and `.gitignore` are untracked. Needs atomic Conventional Commits (feat/test/docs split).
- Decide whether `runs/` stays git-ignored (it is) and whether the Qwen example configs in `examples/qwen/` should be kept.
