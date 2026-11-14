# Milestones

The target is a six-player no-limit Texas hold'em arena with a custom, lightweight rules engine and separate agent interfaces. The Rust implementation covers milestones 1–3 and live-validated milestone 4 pilots, including a six-seat same-model table. Milestone 5, native subprocess supervision, is implemented and live-validated; Docker/OCI is deferred.

1. [Rules and interfaces](01-rules-and-interfaces.md)
2. [First playable hand](02-first-playable-hand.md)
3. [Six-player rules and reliability](03-six-player-rules.md)
4. [LLM agents and evaluation](04-llm-agents-and-evaluation.md)
5. [Native Rust subprocess participants](05-native-subprocesses.md)
6. [Analysis and parallel sessions](06-analysis-and-parallel-sessions.md) — done; baseline: model +56 bb/100 [+25, +88] over 10,070 hands
7. [Mixed-model tables and memory experiment](07-mixed-models-and-memory-experiment.md) — memory run (budget design): oracle notes +33 bb/100 [+2, +65]; own notes +24, unresolved
8. [Better opponents and exploitation ceiling](08-opponents-and-exploitation-ceiling.md) — done
9. [Scaffold treatments](09-scaffold-treatments.md) — done (pilot): descriptive features add +120 bb/100 [+37, +203]; equity and advice add nothing measurable beyond that

Qwen3.6-35B-A3B comparison: [milestone 9](09-scaffold-treatments.md#qwen-comparison). Everything still outstanding is in [next-steps](next-steps.md).

Backlog: 10. isolation (host-owned inference proxy, then containers).

The implementation language is Rust only. See the [implemented rules and protocol](../docs/06-rules-and-protocol.md), [experiments](../docs/08-experiments-and-observability.md), and [deferred container notes](../docs/09-container-preparation.md) for supported behavior and remaining work. See [native subprocess supervision](../docs/10-native-subprocess-supervision.md) for milestone 5 results. Milestones 6–8 are described in [docs/11](../docs/11-analysis-experiments-and-opponents.md); milestone 9 and the baseline results are in its own milestone file. Real mixed-model tables and container deployment remain outstanding; see [next-steps](next-steps.md).

## Claude feedback (2026-11-11)

Proposed next milestones, in the order I would do them (6–8 have since been started; see the list above):

6. **Analysis and parallel sessions.** A Rust `analyze` command (per-seat VPIP/PFR/aggression, fallback rate, bb/100 with seed-block intervals, cost per hand) and a scheduler that runs independent sessions concurrently under one ledger. Fix the orbit bias (hands a multiple of seats). Done when a 10,000-hand baseline finishes in under an hour with a reported interval.
7. **Heterogeneous tables and a powered memory experiment.** Per-seat model configuration; notebook prompt v3 (profiles, not transcripts); reflection-without-persistence and oracle-notes controls; sessions of 100+ hands. Done when the pre-registered memory comparison has an interval that excludes or includes zero at a stated precision.
8. **Better opponents and an exploitation ceiling.** Archetypes with specific bounded leaks, one solid baseline, one adaptive opponent; a simulated best-response ceiling per archetype.
9. **Scaffold treatments.** Descriptive features, then equity tool, then advisory skills, each as a separate labelled condition.
10. **Isolation.** Host-owned inference proxy first; containers only when third-party agents are in scope.

The reasoning: milestones 1–5 built a trustworthy instrument. Nothing measured with it so far can support a claim, and the gap is sample size and analysis, not infrastructure.
