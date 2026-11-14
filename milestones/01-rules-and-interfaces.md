# Milestone 1: Rules and interfaces

**Implementation status:** Initial Rust types and [written rules/protocol](../docs/06-rules-and-protocol.md) are implemented, including legal-decision examples, information boundaries, and verification commands. Remote deadlines remain a later adapter responsibility.

**Goal:** Specify the rules and boundaries before implementing the engine. The final target is six models at a table, using our own rules, arena, and agent interfaces. Start with a two-player playable slice while representing seats and players in a way that can grow to six.

## Initial game

- No-limit Texas hold'em cash game; fixed blinds and starting stacks of 100 big blinds.
- Two scripted players for the first playable slice; six seats are the target.
- Terminal output and structured logs are sufficient initially.
- Document assumptions for blind/button rotation, heads-up order, minimum raises, all-ins, ties, rounding, showdown, and hand termination. Specify rake and rebuy policy before using results for evaluation.
- Use integer chip units. Define explicitly whether an action amount is a total commitment or an increment, and define the units and legal bounds.

## Boundaries

The engine is the authoritative referee: it owns the deck, complete state, rules, legal actions, chip accounting, and outcomes. The arena coordinates seats and agents, drives the engine, and records events. Agents receive an explicit player-specific view and return an action; they do not mutate engine state or decide legality.

The player view contains the agent's own cards, public cards and actions, positions, pot, stacks, and current legal actions and amounts. It must not expose opponents' unrevealed cards, future deck order, private notes, or complete engine state. The evaluator may keep a privileged record, stored separately from agent-visible data. Define which cards are revealed at showdown and what each player may observe.

Use an interface that can support scripted agents first and remote or containerized agents later. Define action validation, invalid responses, timeouts, fallback behavior, deterministic seeds for tests, and a structured event log. Avoid exposing privileged debugging output to agents.

## Done when

- A written rules specification covers the first slice and identifies deferred multiplayer cases.
- State, player view, legal actions, agent response, and event interfaces have concrete schemas or types.
- Examples show a legal decision and which information is withheld from each player.
- A plan exists to verify the custom rules independently, including adversarial edge cases.

## Claude feedback (2026-11-11)

- Complete. The decision to fix "total street commitment" early paid off: it is the one protocol point the model has not got wrong in any live run.
- In hindsight the interface should have included timeouts and explicit lifecycle results from the start; both were retrofitted in milestone 5 by changing the `Agent` trait.
- Still open from this milestone's own list: rake (none, by choice) and a rebuy policy suitable for evaluation. Busted-only rebuy to 100 BB means stack depths drift within a session, so later hands are not comparable to early ones. Consider resetting all stacks every hand for evaluation runs; it makes hands closer to independent and simplifies the statistics.
