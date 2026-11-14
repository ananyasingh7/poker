# Milestone 2: First playable hand

**Implementation status:** Playable with `cargo run -- 2 10 42`. The Rust engine supports fold/showdown completion, scripted agents, structured logs, and deterministic replay. See [rules and verification](../docs/06-rules-and-protocol.md).

**Goal:** Run a complete heads-up hand with our own deterministic rules engine and scripted agents, with no LLM required.

Implement deck and dealing, blinds, betting rounds, legal-action checks, folds, board progression, showdown hand ranking, pot awards, and stack updates for the documented slice. Make the game state reproducible from a seed and actions. The arena repeatedly asks the acting scripted agent for an action using only its player view, validates it, applies it, and prints and records the resulting events.

Initial scripted agents can check or call, fold, and make simple legal raises. Keep their strategy deliberately simple so failures can be traced to the engine and interface.

## Done when

- Scripted agents can play hands to completion through a fold or showdown, with reproducible terminal output and structured logs.
- Tests cover turn order, legal and illegal actions, betting-round closure, hand rankings, ties, and chip conservation. Include heads-up blind and action order.
- Tests verify that player views and summaries never reveal hidden cards or the deck.
- Any unsupported all-in or betting scenario is explicitly rejected or documented; no unsupported scenario silently produces a result.

This is a vertical slice, not a claim that all multiplayer and side-pot rules are finished.

## Claude feedback (2026-11-11)

- Complete. Suggest adding a handful of golden hand histories (input seed and actions, expected full event list) checked into the repository, so an engine change that alters behaviour fails loudly rather than only when replay versions are compared.
- The scripted Caller/Folder/Raiser agents remain the most useful debugging tools in the project; the subprocess benchmark and fault tests are built on Caller.
