# Design

What the lab is for, and how it is built. Merges the original vision, strategy, architecture, memory, rules, protocol, subprocess and container notes (git history has the originals).

## Goals

**The question:** when a language model plays poker, *which capabilities and supporting tools explain how well it does, against which opponents, and at what cost?* Not "which model wins": poker rewards probabilistic judgment, opponent modelling and adaptation, and a lab that controls what the model is told can separate those.

Principles that follow from it:

1. **Judge decisions, not outcomes.** A good bluff can lose and a bad one can win. Results are reported as expected value (bb/100) over many deals, with luck removed where possible and decisions graded against the best available choice.
2. **Controlled treatments.** Every kind of help (facts sheet, calculator, advice, notes, tracker, cheat sheet, sizing rules, thinking) is a labelled switch, measured against a matched control on the same deals.
3. **Opponents with known answers.** Scripted bots whose exploits are written down, plus a tuned-bot ceiling, so "the model exploited the station" is checkable, not a story the model tells.
4. **Cost is a result.** Tokens, dollars and latency are reported beside win rates.
5. **Everything replayable.** Seeds, exact requests and responses, and the engine's own record are kept, so any number can be recomputed.

**Hypotheses** (from the original plan; status in [results](results.md)):

- H1. A model wins more against leaky scripted opponents than against a balanced one. *Supported (station and overfolder are where its money comes from).*
- H2. Notes raise winnings against fixed opponents over sessions of 100+ hands. *Unresolved: notes cut river mistakes but no chip gain is resolved.*
- H3. A model with notes identifies an opponent archetype more often than chance. *Supported informally: profiles name the station within 1-4 hands.*
- Extra: correct reads (cheat sheet) are worth more than the model's own (yes), and the model misreads facts it could compute (yes: the facts sheet is the largest thinking-off gain).

**Scope.** The core result is memory and opponent exploitation. Skills libraries, solvers, fine-tuning and deliberate image manipulation ("show a cheap leak, exploit the adjustment", which needs an adapting opponent) are later treatments. A poker benchmark cannot show general intelligence; results hold for this game format, field, prompts and model versions.

## Game and engine (`src/engine.rs`, `src/cards.rs`, `src/arena.rs`)

- No-limit Texas hold'em cash game, 2 to 6 seats, blinds 1/2, stacks 200 (100 BB), no rake. The experiments use 6 seats, 60-hand sessions (whole orbits), and reset every stack to 100 BB each hand so hands are close to independent.
- `RaiseTo(n)` is the **total committed on this street**, also for opening bets. Minimum raise is the last full increment; short all-ins do not reopen betting unless cumulative raises reach a full increment (Poker TDA). Side pots by contribution level; odd chips go clockwise from the button.
- Deterministic: SplitMix64 and Fisher-Yates from a seed. `poker-lab replay` rebuilds every hand from its record and checks every action, event and stack.
- Verification: constructed edge cases, thousands of seeded hands with chip conservation checks, an exhaustive 2,598,960-hand census (`cargo test --release -- --ignored`), and byte-identical records across the in-process and subprocess backends.

## Information boundary

The engine is the referee: it owns the deck, legality and money. Each seat gets a `PlayerView`: its own cards, the board, pot, every seat's public stack and contributions, the hand's public history and its legal moves. Never opponents' unrevealed cards, the deck, the seed, other notebooks or (except through the cheat sheet) who the bots are. The exact text sent to a model is in [model input](model-input.md).

Logs per session (directories 0700, files 0600): `public.jsonl`; `seat-N.jsonl` (that seat's views); `evaluator.jsonl` (privileged replay records with seeds: never shown to a participant); `manifest.json`, `summary.json`; `model-private.jsonl` (exact requests and responses, key redacted); `hand-history-private.txt`.

## Participants

`Agent::decide(&PlayerView) -> Action` is the only boundary. An illegal or malformed answer is replaced by check-if-legal-else-fold and logged; the fallback is part of what is measured.

- **Model seat** (`src/openrouter.rs`): one OpenRouter chat request per decision, temperature 0, strict JSON schema answer, optional reasoning. Provider routing can be pinned (order, quantization) and is recorded. Per-decision retries for infrastructure faults (timeouts, 429, 5xx) with backoff; model faults are never retried. Opt-ins: re-ask after an illegal move (`repair_illegal`), quick re-ask after a truncated answer (`reask_truncated`), accept a `call` that repeats the amount (`normalize_call_amount`).
- **Worker processes** (`src/supervisor.rs`, `src/worker.rs`, `"execution": "subprocess"`): one process per seat, protocol v2 (JSON lines, strict identity echo, 1 MiB frames, deadlines), killed and reaped on any transport fault so a late answer can never be read on a later turn; no mid-session restart. Workers get a cleared environment, their own directory, and the API key only if they are model seats. The host holds the only request and spend ledger and grants one provider call at a time. This is supervision of trusted, project-owned code, not a sandbox.
- **Containers**: deferred. Build a host-owned inference proxy first (holds the key, enforces the budget before a request, single egress point); containers only when third-party agents arrive.

## Opponents (`src/opponents.rs`)

The current field, each with a written exploit:

| Bot | Behaviour | Exploit |
| --- | --- | --- |
| `station` | Never bets or raises; never folds a pair or ace-high | Value-bet big; never bluff |
| `overfolder` | Calls flop and turn loosely, folds anything below two pair to a river bet | Bluff rivers |
| `solid` | Monte Carlo equity against one random hand vs pot odds; bets 2/3 pot for value, bluffs 15% | None large |
| `adaptive` | `solid` plus one rule: after 8 responses to its bets, folds above 60% raise its bluffing to 80%; below 30% it stops bluffing | Depends on the table |
| `tight` | Plays strong hands only, folds below a pair to postflop bets | Steal |

All are deterministic (their randomness is seeded from the spot), which is what makes exact range grading possible. The original `tight / loose / aggressive` and `random` fixtures remain for older runs. **Ceiling** (`poker-lab ceiling`): the best of 162 settings of a tunable policy, selected on training seeds, measured on held-out seeds; a lower bound on what a competent exploiter wins (+288 bb/100 against this field).

**Weakness:** the field is fixed and simple. Every result is about exploiting these bots; none is about play against strong or adapting players.

## Treatments the model can be given

All opt-in, all labelled in the condition name that `analyze` reports.

| Kind | Switch | What it adds |
| --- | --- | --- |
| Memory | `memory: none / stats / notes / both / oracle` | Tracker statistics per opponent; the model's own notebook rewritten after hands (`notebook_style`, `notebook_every`, `notes_thinking_off`); or the cheat sheet (accurate bot descriptions) |
| Decision aids | `scaffold: none / features / equity / advice` | Facts sheet (position, pot odds, made hand, draws, stack depth); + Monte Carlo equity; + the `solid` rule's move |
| Sizing help | `sizing_hint`, `sharp_sizing`, `size_menu` | A sizing sentence, the sharper sizing sentence, or a menu of raise amounts |
| Thinking | `reasoning_enabled`, `reasoning_effort` | Provider reasoning; `low` effort bounds deliberation |
| Observation | `label_positions` | Position name and big blind in the raw view |

The notebook is the model's choice of what to keep (keep or replace; capped; structured per-seat profiles since prompt v5 so one overrun cannot erase the rest). Statistics are computed by the host with explicit denominators. Notes and statistics never cross seats or sessions.

## Spectator table

A separate workspace crate (`table/`) that plays with the same engine, bots, model agent and graders, and shows what the model was sent, its reasoning, replays and grades. See [table](table.md).
