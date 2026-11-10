# Proposed architecture

This is a proposed design, not a description of an existing implementation.

## Decision loop

1. A deterministic no-limit Texas hold'em engine manages the deck, blinds, betting, stack accounting, side pots, and showdown. It exposes only information the acting player may know and the exact legal actions and amount limits.
2. A context builder formats the player's private cards, public board, positions, pot, stacks, current-hand action history, legal actions, and optional derived labels such as pot type and board texture.
3. A stats service supplies a small permitted set of opponent aggregates, with appropriate denominators. A private notebook supplies the acting agent's own retained observations.
4. An optional skill retriever selects situational poker guidance. Other experimental variants may use charts, equity tools, or solver outputs; these are distinct treatments, not assumed parts of a fair model-only comparison.
5. The model returns a structured action and amount. The validator checks the engine's rules, records any invalid response, and applies a documented fallback if necessary.
6. After the hand, each agent receives only a summary it is entitled to see and may update its own capped notes. The engine updates permitted stats.
7. The evaluation harness records the state, model-visible input, configuration, outputs, validation, memory updates, and outcomes, including information withheld from agents for later analysis.

Candidates include PokerKit or an equivalent engine, context labels such as stack-to-pot ratio, a layered skill library, structured tool calls, and cached stable prompt sections. An optional exploitation gate could be used to limit strategic deviations when evidence is weak. These are candidates to test, not proven requirements.

## Design boundaries

- **Information isolation:** never expose opponents' unrevealed cards, future deck order, or private notebooks. Showdown summaries must respect what was actually revealed. Separate each player's view from the evaluator's complete record.
- **State and accounting:** the engine is authoritative for legal moves and money. Clearly define whether a bet amount is a total commitment or an increment. Include all-in and side-pot handling.
- **Agent identity:** use stable per-session opponent aliases so notes remain useful while avoiding provider-name cues. Specify whether identities persist across sessions.
- **Reproducibility:** record model/provider version, prompt and skill versions, rules, seeds, seats, stack sizes, fallback behavior, token use, and costs. Hosted models can still change or be nondeterministic.
- **Fair comparison:** standardize information and resource budgets where possible. A shared scaffold measures the model *plus that scaffold*; compare with minimally scaffolded agents to understand what it contributes.
- **Action history:** compression can omit strategically relevant details. Preserve the full current-hand public action sequence initially if it fits; compare any lossy summary against it before relying on compression.

## Scope and next decisions

Start with a small, correct, observable table and simple scripted agents before scaling model calls. Choose cash game or tournament, number of seats, blind/stack structure, rake, session length, and rebuy policy explicitly. Build the interface so memory, stats, skills, and opponents can be swapped independently. The components still need schemas, full prompts, and a working engine.
