# Milestone 4: LLM agents and evaluation

**Implementation status:** The Rust OpenRouter baseline and a six-seat same-model table have been live-validated. Readable reports, fixed heuristic opponents, multi-seed/seat batches, and independent notes/statistics conditions are implemented. See the [experiment guide](../docs/08-experiments-and-observability.md). Heterogeneous model tables and statistically powered comparative evaluation remain outstanding.

**Goal:** Connect models through the same agent boundary and run observable six-player experiments.

Begin with one model against scripted agents, then expand to six model seats. Define a versioned structured request and action response, legal-action validation, timeout and fallback policies, and logs of what each agent actually saw. Keep provider adapters outside the rules engine. Record model versions, settings, costs, latency, errors, and seeds. Never send evaluator-only information to agents.

Add capped agent-controlled notes and optional coarse statistics as independently configurable conditions. Later add skill guidance and solver-informed references as separately measured treatments. Compare agents against fixed archetypes, rotate seats, record reproducible sessions, and report outcomes with uncertainty; see [evaluation](../docs/05-evaluation.md) and [memory](../docs/04-memory.md).

## Done when

- A model can play complete hands without violating the information boundary, and invalid responses are visibly recorded and handled consistently.
- Six model participants can complete a small session with auditable inputs, actions, results, and resource use.
- Baseline comparisons and memory conditions can be configured and reproduced as far as provider behavior permits.

## Claude feedback (2026-11-11)

- The "done when" list is met except for its spirit: "report outcomes with uncertainty" is not implemented anywhere, and the data so far (120 baseline hands) could not support an interval worth reporting. I would close this milestone as "integration complete" and move evaluation into its own milestone with a hand-count target.
- Heterogeneous tables need per-seat model settings in `Config`. The subprocess `init` message is already per seat.
- The live runs surfaced three concrete issues to carry forward: notebooks are used as transcripts, notes changed no actions over ten hands, and strict parsing folded an unambiguous call. Each is described in the corresponding `docs/` feedback.
