# River grading against the bots, leak report, tracker stats and cheaper notes (2026-11-16)

Measurement and cost work, no API spend. The point: chip results needed thousands of hands to separate conditions (own notes vs none was +2 [-148, +152] bb/100 at 300 hands); the river grade separates them with the same hands.

## River grading (`src/river.rs`)

Every river decision of a model seat is graded against the scripted field:

1. **Ranges.** For each live opponent, every hole-card pair among the cards the model cannot see is kept if that bot's policy would have made every move it made in the hand so far. The bots are deterministic (their only randomness is seeded from the spot), so this is exact: the range a player who knows how the bot plays would put it on.
2. **Alternatives.** Check or fold, call, bets or raises of 33, 50, 75, 100 and 150% of the pot, all-in, and the action actually taken.
3. **Play-out.** Each alternative is replayed against every holding in the ranges (all combinations when there are at most 400, otherwise 400 deterministic samples), with the bots answering by their real policies, `Adaptive` with what it had learned from earlier hands in the session. Hole cards are dealt with `Hand::with_deck`.
4. **Later replies.** If a bot raises after the model acts, the model answers fold or call, whichever does better given what it saw (one choice per sequence of opponent actions, not per hidden card).
5. **Result.** The expected final stack of each alternative; the best one, the expected chips the chosen action gave up (`loss`), and the kind of mistake: missed bet, bad bet, sizing, bad call, bad fold, missed raise, bad raise.

The benchmark is a player who knows exactly how each bot plays but not its cards, which is what the cheat sheet gives. River cost is therefore a distance from perfect exploitation, not from sound poker: against a station that never folds a pair, the best river bet is often all-in. Use it to compare conditions on the same deals, not as an absolute verdict.

**First design rejected.** Grading against the opponents' actual cards (the convention of the call/fold grade) reported 770 bb/100 of river cost for the no-help arm, mostly "missed bets" where shoving would have won only because the model's hand happened to be best. Grading against ranges halves that and removes the hindsight.

**Tests.** The real holding of every bot is always inside its computed range, including `Adaptive` after it has adapted (400 hands); replaying the real deal with the action taken reproduces the recorded final stack exactly; checking a winner to the station is a missed bet. A run whose bots do not match its manifest is skipped with a reason, not fatal.

## In `poker-lab analyze`

- **River cost** per model and condition (bb/100), with seed-block intervals and paired differences beside the chip results.
- **River decisions** table: graded decisions, total cost, and cost and count per kind.
- **Leaks**: missed river bets, bluffs (postflop bets or raises under 30% equity against the cards held) and how many were called down to showdown, and truncated answers.
- `--decisions` rows carry their river grade (`river`: kind, best action, loss, chosen and best values, range sizes).

Grading a 1,200-hand batch takes about 30 seconds.

## Results on the existing runs

Thinking on, effort low ([docs/14](14-thinking-low-effort-and-cheat-sheet.md)):

| | No help | Cheat sheet |
| --- | --- | --- |
| River decisions graded | 411 | 351 |
| River cost (bb/100) | 389 | 225 |
| of which missed bets | 190 (155) | 157 (93) |
| of which sizing | 147 (56) | 35 (51) |
| Bluffs called down | 82% of 76 | 43% of 86 |

Paired, cheat sheet minus no help, 10 seeds: chips +107 [+54, +159]; river cost **-164 [-245, -82]**.

Own notes, seeds 1-5, seat 0 (300 hands per arm):

| | No help | Own notes | Cheat sheet |
| --- | --- | --- | --- |
| River cost (bb/100) | 419 | 313 | 163 |
| missed bets | 255 (47) | 73 (22) | 109 (21) |
| sizing | 92 (7) | 167 (23) | 46 (13) |

Paired, notes minus none: chips +2 [-148, +152]; river cost **-106 [-155, -57]**. The chips could not tell the arms apart; the river grade can. Own notes stop the model checking winners (missed bets fall from 255 to 73) but it then bets too small for a station (sizing rises from 92 to 167): it knows who calls, not yet how much to bet. Reading its notes also showed it sometimes profiles the over-folder as a "calling station".

## Tracker statistics (`memory: stats`)

The stats condition now also gives, per seat, postflop bets/raises and calls, postflop bets faced and folded to, and the same on the river: the numbers a real tracker shows and the ones that expose a station (aggression near zero, almost never folds) or a river over-folder. No extra provider calls, so it costs what no help costs. Never run yet; ready config `examples/experiment-thinking-low-stats.json`. In the table it is the TRACKER help.

## Cheaper notes (`notebook_every`)

`notebook_every: N` rewrites the notebook after every Nth hand, sending the hands since the last update together (`earlier_hands`); 1 (the default) keeps the original per-hand requests byte for byte. Notes cost about $0.0012 per hand with thinking on because of the per-hand rewrite; N = 5 should cut that close to the no-help cost. Ready config `examples/experiment-thinking-low-notes-every5.json`.

## Table

NOTES tab (key `5`), TRACKER help, river grades in HAND and SESSION, LEAKS block: see [docs/13](13-spectator-table.md).
