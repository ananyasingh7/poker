# Milestone 3: Six-player rules and reliability

**Implementation status:** Initial implementation supports six-player sessions, all-ins, side pots, ties, short raises, seat rotation, and busted-seat rebuys. Tests include constructed edge cases, thousands of reproducible hands, and an exhaustive five-card category census. See [exact rules, verification scope, and limitations](../docs/06-rules-and-protocol.md); this is not a formal proof or a complete independent engine comparison.

**Goal:** Complete the custom rules needed to run six scripted players fairly before introducing models into the main experiment.

Support six seats, button and blind rotation, action order, folds, all-ins, main and side pots, eligibility, tied pots and odd chips, minimum raises and short all-in raises, and the configured cash-game session and rebuy rules. Agree on exact rules for edge cases in the specification before implementing them.

Test constructed edge cases and many reproducible hands, asserting legal transitions and chip conservation. Cross-check hand rankings and representative rule outcomes against trusted independent references or implementations as a verification aid; this does not make them the runtime engine. Ensure each player receives only their own permitted view and that logs distinguish public, private, and evaluator-only information.

## Done when

- Six scripted agents can run repeated sessions without invalid state or lost chips.
- Tests cover multiple all-ins, side-pot eligibility, folded contributions, split pots, short raises, and seat rotation.
- Deterministic replay reproduces outcomes and information-isolation checks pass.
- Known limitations and the exact rules are documented before results are used for evaluation.

## Claude feedback (2026-11-11)

- Complete, with the stated caveat that verification is by fixtures and a category census, not a second implementation. A dev-only differential test against an independent evaluator or engine is the remaining cheap gain in confidence; see the [rules feedback](../docs/06-rules-and-protocol.md).
- New evidence since this was written: 1,000 six-seat hands with side pots, busts and rebuys produced identical records on the in-process and subprocess backends, and every live model hand has replayed.
