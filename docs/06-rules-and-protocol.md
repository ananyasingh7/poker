# Implemented rules and protocol v1

All implementation and tests are Rust. The custom referee is in `src/engine.rs`, card evaluation in `src/cards.rs`, agent/session orchestration in `src/arena.rs`, and the terminal runner in `src/main.rs`. Serde and serde_json provide serialization; no external poker engine is used at runtime.

## Cash-game rules

- Two through six fixed, occupied seats, numbered clockwise from zero. Default blinds are 1/2 integer chips, starting stacks 200 (100 BB). No rake, antes, straddles, tournament levels, seat changes, or mid-hand rebuys.
- The button starts at seat zero and advances one seat per hand. Heads-up: button posts the small blind and acts first preflop; the big blind acts first afterward. Multiway: small and big blinds are the next two seats; preflop action starts left of the big blind, later streets left of the button. Skip folded and all-in players.
- Blinds are capped at the player's stack. A short big blind does not reduce the nominal preflop bring-in while multiple players can still bet. When only one player has chips, they only need to match actual outstanding commitments; no dry side-pot betting is offered.
- Players receive two cards, dealt one at a time clockwise starting left of the button. Burn one card before each board street; reveal three flop cards, then one turn and river card. The evaluator chooses the best five of seven. Aces may be low in a five-high straight; suits never break ties.
- `RaiseTo(amount)` means **total chips committed on this street**, including blinds, not an increment or lifetime hand contribution. A bet on an unopened street uses the same action. `Call` pays the smaller of the amount owed and the remaining stack. No separate all-in verb is needed.
- A full opening bet is at least one big blind. Minimum raises increase the current wager by the last full bet/raise increment, initially the big blind. Smaller increases are permitted only when committing the player's entire stack. Short all-ins do not change that increment. A player who already acted may raise again only after facing at least a full increment since their previous action, including cumulative short all-ins. A check records action at zero.
- Betting closes once all nonfolded players with chips have acted and matched the wager, or only all-in players remain. The big blind retains its option after preflop calls. Run the board automatically when further betting is impossible. One surviving player ends the hand immediately without revealing cards or dealing unnecessary board cards.
- Pots are constructed from successive total-contribution levels. Folded contributions remain in pots but folded players are ineligible. A singly contributed tier is refunded. Each contested tier is awarded only among eligible players. Ties divide chips evenly; remaining odd chips go clockwise starting left of the button, separately for each pot. Uncalled money is refunded at settlement; the displayed in-hand pot includes it until then.
- At showdown all nonfolded hands are revealed, even losing hands. No optional mucking. Folded cards and uncontested winning cards remain hidden. All authorized views retain the owner's own cards.
- Sessions carry stacks forward. Only a busted seat automatically rebuys to 100 BB before the next hand; other stacks are not topped up. Rebuy chips are counted separately. Any future profit calculation must subtract starting chips and rebuys. Session API callers must retain the configured fixed seats and blinds.

## Types and information boundaries

Concrete, serde-compatible protocol types live in `engine.rs`: `Rules`, `Action`, `LegalActions`, `PublicPlayer`, `Event`, and `PlayerView`. `PlayerView.protocol` and evaluator `Record.version` are 1. Cards are integers 0..51, rank `card % 13 + 2`, suit `card / 13` (clubs, diamonds, hearts, spades). Invalid cards and unknown action fields are rejected during deserialization.

A decision contains the player's seat and hole cards, button, street, public board, pot, public stack/contribution/fold status for every seat, complete public event history, and legal bounds. Inactive seats and completed-hand summaries have no legal decision. Opponents' hole cards enter history only through a public showdown `Reveal` event. No seed, future cards, opponent notebook, or complete engine state is present. The authoritative `Hand` is neither serializable nor printable with Debug.

Example: at the initial heads-up decision with blinds 1/2 and stacks 200, seat zero can fold, call one chip, or raise to any integer from 4 through 200. `{"type":"raise_to","amount":6}` commits six chips on the street (five additional chips after posting the small blind). `{"type":"check"}` is illegal. `{"type":"call"}` is legal. The view supplies only seat zero's two cards; seat one's cards and the shuffle seed are withheld.

`Agent::decide(&PlayerView) -> Result<Action, String>` is the agent boundary. Illegal actions and returned errors are logged with the proposal and replaced by check if available, otherwise fold. The engine independently rejects illegal and out-of-turn actions without changing state. Trusted in-process scripted agents execute synchronously: **there is no wall-clock timeout or panic containment yet**. Future remote adapters must enforce a deadline and return an error through this boundary; hung or hostile agents must not be run in-process.

## Logging and reproducibility

`cargo run -- 6 100 42 /tmp/poker-run` creates a new log directory (fails rather than overwriting an existing directory). Console output contains hand number, button, action count, stacks and session rebuy totals.

- `public.jsonl`: one public event list per hand; no shuffle seeds or unrevealed cards.
- `seat-N.jsonl`: that seat's decision views and final summary, private to that seat.
- `evaluator.jsonl`: privileged versioned replay records containing rules, shuffle seed, starting stacks, button, proposals/errors/applied actions, public events, and final stacks. The seed reconstructs all hidden cards. Never give this file to participants.

Unix log directories use mode 0700 and files 0600. File separation is an application boundary, not a sandbox between untrusted processes running as the same OS user. No notes or models are implemented in these milestones.

`cargo run -- replay /tmp/poker-run/evaluator.jsonl` reconstructs each hand and verifies legal applied actions, fallback selection, exact events, termination, and final stacks. The specified SplitMix64 generator and rejection-sampled Fisher–Yates shuffle are reproducible across platforms; this predictable RNG is for experiments, not real-money security. Future changes to shuffle, dealing, or rules require a replay version change. Seed and action replay is independent of agent implementation.

## Verification and limitations

`cargo test` exercises heads-up order, blind options, illegal-action atomicity, folds, showdown categories and kickers, multiple all-ins, side pots, folded money, odd-chip ties, cumulative short raises, short blinds, replay tampering, fallback, and seat rotation. It runs 2,500 deterministic session hands across two through six seats and another 2,000 hands with varied stacks/buttons; every transition checks chip conservation. Information tests check the serialized view structure and that folded hands are never revealed.

`cargo test --release -- --ignored` enumerates all 2,598,960 five-card combinations and compares category counts with the independent [Wolfram MathWorld poker census](https://mathworld.wolfram.com/Poker.html), combining royal and other straight flushes. Category counts do not by themselves prove every kicker ordering; constructed ranking cases supplement them.

The short-all-in fixtures use the full-raise and cumulative-reopening rule described in the [Poker TDA rules](https://www.pokertda.com/view-poker-tda-rules/). TDA is a tournament reference, not a claim that this cash-game house specification implements all TDA procedures. Representative fixtures check a raise from 2 to 10, a short raise to 15 that does not reopen the original raiser, and cumulative raises to 20 that reopen with a minimum of 28. Side-pot and odd-chip fixtures have manually derived expected payouts. These checks are verification aids, not a formal proof or a complete differential test against another engine.

Engine scope limits: fixed funded seats, automatic busted-only rebuys, no disconnections or live-dealer procedures, no optional mucking, no notebooks/statistics, and no performance or strategic-strength claims. The subsequent [OpenRouter pilot](07-openrouter-pilot.md) adds one model adapter with HTTP deadlines; arbitrary in-process agents still have no timeout or panic containment. Financial evaluation and untrusted agents should wait for the later milestones and broader independent review.

## Claude feedback (2026-11-11)

- **Two statements above are now out of date.** `Agent::finish_hand` and `start_hand` return `Result`, and failures are collected in `PlayedHand.lifecycle_errors` without touching the replay record. And agents can now run out of process with deadlines and kill/reap; the "no wall-clock timeout or panic containment" warning applies only to the in-process backend.
- **Orbit bias.** The button always starts at seat 0 and sessions default to 10 hands, so blinds are posted unevenly across seats in six-handed play. Either randomise the starting button from the session seed or require hand counts that are multiples of the seat count.
- **A bet is displayed as a raise.** `RaiseTo` for an opening bet is a sound protocol choice, but older reports print "RaiseTo(2)" for a flop bet of 2. The current report code prints "raises to N (adds M)"; consider "bets N" when the street is unopened, for human readers and for the model's history.
- **Independent verification is still thin.** The category census and hand-built fixtures are good, but there is no differential test. Two cheap additions: compare the seven-card evaluator against a brute-force best-of-21 enumeration on random hands, and add an independent poker crate as a dev-dependency only, to cross-check showdown winners and side-pot splits on thousands of random hands. Property-based tests with shrinking would also suit the raise-reopening rule.
- **Export a standard hand-history format.** A PokerStars-style text export would let existing trackers compute VPIP, PFR and positional statistics and would give a human-checkable view of the engine's behaviour for free.
- **Publish the schemas.** If agents in other languages are ever expected, generate JSON Schema for `PlayerView`, `Action` and the participant protocol from the Rust types so the contract is not only the Rust source.
