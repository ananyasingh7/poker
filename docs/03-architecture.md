# Proposed architecture

This describes the broader proposed design. The rules engine, scripted arena, player views, and replay logs now have a Rust implementation; see the [implemented rules and protocol](06-rules-and-protocol.md). Model, memory, and evaluation components below remain proposals.

## Decision loop

1. A deterministic no-limit Texas hold'em engine manages the deck, blinds, betting, stack accounting, side pots, and showdown. It exposes only information the acting player may know and the exact legal actions and amount limits.
2. A context builder formats the player's private cards, public board, positions, pot, stacks, current-hand action history, legal actions, and optional derived labels such as pot type and board texture.
3. A stats service supplies a small permitted set of opponent aggregates, with appropriate denominators. A private notebook supplies the acting agent's own retained observations.
4. An optional skill retriever selects situational poker guidance. Other experimental variants may use charts, equity tools, or solver outputs; these are distinct treatments, not assumed parts of a fair model-only comparison.
5. The model returns a structured action and amount. The validator checks the engine's rules, records any invalid response, and applies a documented fallback if necessary.
6. After the hand, each agent receives only a summary it is entitled to see and may update its own capped notes. The engine updates permitted stats.
7. The evaluation harness records the state, model-visible input, configuration, outputs, validation, memory updates, and outcomes, including information withheld from agents for later analysis.

The rules engine will be custom-built. Potential later additions include context labels such as stack-to-pot ratio, a layered skill library, structured tool calls, and cached stable prompt sections. An optional exploitation gate could be used to limit strategic deviations when evidence is weak. These are candidates to test, not proven requirements.

## Design boundaries

- **Information isolation:** never expose opponents' unrevealed cards, future deck order, or private notebooks. Showdown summaries must respect what was actually revealed. Separate each player's view from the evaluator's complete record.
- **State and accounting:** the engine is authoritative for legal moves and money. Clearly define whether a bet amount is a total commitment or an increment. Include all-in and side-pot handling.
- **Agent identity:** use stable per-session opponent aliases so notes remain useful while avoiding provider-name cues. Specify whether identities persist across sessions.
- **Reproducibility:** record model/provider version, prompt and skill versions, rules, seeds, seats, stack sizes, fallback behavior, token use, and costs. Hosted models can still change or be nondeterministic.
- **Fair comparison:** standardize information and resource budgets where possible. A shared scaffold measures the model *plus that scaffold*; compare with minimally scaffolded agents to understand what it contributes.
- **Action history:** compression can omit strategically relevant details. Preserve the full current-hand public action sequence initially if it fits; compare any lossy summary against it before relying on compression.

## Scope and next decisions

Start with a two-player, correct, observable table and simple scripted agents, then complete six-player rules before scaling model calls. Choose cash game or tournament, number of seats, blind/stack structure, rake, session length, and rebuy policy explicitly. Build the interface so memory, stats, skills, and opponents can be swapped independently. The components still need schemas, full prompts, and a working engine.

## Claude feedback (2026-11-11)

- **Status.** Steps 1, 3 (coarse statistics and notebook), 5, 6 and 7 of the decision loop are implemented; step 4 (skills) is not. Participants can now run as supervised worker processes; see [native subprocess supervision](10-native-subprocess-supervision.md).
- **The observation is missing things a player would be told.** `PlayerView` has no blind sizes and no position names; the model has to infer both from the blind events. Add explicit `small_blind`/`big_blind` and a derived position label (button, small blind, …) before any strength claims. That requires a view-schema version bump.
- **No belief channel yet.** The model returns only an action, so there is nothing to analyse for the "reads" questions in the vision. Add an optional reasoning/prediction field as a separate, labelled condition, since asking for reasoning also changes how the model decides.
- **Naming.** `PlayerView.protocol` (1), the engine replay `Record.version` (1) and the participant protocol (2) are three different version numbers. Rename the first to something like `view_schema` to avoid confusion.
- **Per-seat model selection is the main structural gap.** `Config` has a single `model`, so heterogeneous tables are impossible. The subprocess `init` message already takes model settings per seat, so the remaining work is configuration and manifest, not transport.
- **Budget enforcement is post-hoc and self-reported.** The host grants one request at a time and trusts the worker's usage report. A small host-owned inference proxy would hold the key, enforce the cap before the request, and remove the credential from workers entirely. I would build that before any container work.
- **Parallelism.** Decisions within a hand must stay sequential, but independent sessions do not share state. A session scheduler with a shared ledger is now the highest-value architecture change.
