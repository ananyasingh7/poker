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

## Tracker vs notes vs cheat sheet (live, 2026-11-16, $0.77)

Two new arms on the deals of docs/14 (seeds 1-10, model in seat 0, 60 hands, thinking on with effort low, `max_tokens: 4096`, no re-ask): `examples/experiment-thinking-low-stats.json` (`runs/thinking-low-stats-v1`, $0.318, all 10 sessions) and `examples/experiment-thinking-low-notes-every5.json` (`runs/thinking-low-notes-every5-v1`, $0.452, cap reached: 8 complete sessions, the 2 partial ones left out). Compared with the seat-0 sessions of the earlier arms on the same seeds; paired against no help, 95% intervals over seeds.

| vs no help | Seeds | Chips (bb/100) | River cost (bb/100) | Cost per hand |
| --- | --- | --- | --- | --- |
| No help | 10 | (+116 overall) | 435 | $0.00033 |
| Tracker stats | 10 | +8 [-43, +58] | **-75 [-141, -9]** | $0.00053 |
| Own notes, every hand | 5 | +2 [-148, +152] | **-106 [-155, -57]** | $0.00112 |
| Own notes, every 5 hands | 8 | -6 [-72, +60] | **-144 [-186, -102]** | $0.00077 |
| Cheat sheet | 10 | +144 [+27, +262] | **-274 [-435, -113]** | $0.00057 |

Where the river cost goes (bb/100, count in brackets):

| | Missed bets | Sizing | Bluffs called down |
| --- | --- | --- | --- |
| No help | 224 (81) | 159 (23) | 84% of 38 |
| Tracker | 137 (59) | 172 (35) | 82% of 45 |
| Notes, every hand | 73 (22) | 167 (23) | 67% of 21 |
| Notes, every 5 hands | 103 (37) | 153 (28) | 81% of 21 |
| Cheat sheet | 127 (43) | 30 (22) | 36% of 39 |

**Reading.** Every kind of opponent information helps on the river, in the expected order: tracker < notes < cheat sheet. Notes every 5 hands do as well as notes every hand, for about 30% less. But only the cheat sheet fixes **bet sizing** (30 against 150-170 for all the others) and **bluffing the station** (36% called down against 67-84%). The cheat sheet says in words "bet your good hands big; never bluff it"; stats and notes tell the model who calls, not what to do about it. When it bets the river with its own notes, the model bets a median of 0.56 of the pot; the best bet was all-in in 19 of 31 cases.

**Cost of notes every 5 hands.** Only 31% cheaper than every hand, not 5x: reasoning about five hands took a median of 4,548 tokens per update, and 23 of 117 updates (20%) truncated even at 9,216 tokens (4,096 + 1,024 per hand, raised after the smoke test truncated at 5,120). Writing notes probably does not need deep reasoning; a thinking-off note update would be far cheaper and is worth a try.

## Bet-sizing help on the tracker deals (live, 2026-11-16, $0.995)

Three arms on the tracker arm's deals (seeds 1-10, seat 0, 60 hands, same settings): `sizing_hint` (one general sentence, docs/16 section 2e), `size_menu` (a `bet_sizes` menu of raise_to amounts) and both. Configs `examples/experiment-thinking-low-stats-{hint,menu,hint-menu}.json`, runs `runs/thinking-low-stats-{hint,menu,hint-menu}-v1`, $0.33 each. Each hit its cap within the last hands: 9 of 30 sessions stopped 1 to 6 hands short and are left out, so every comparison uses complete sessions on the same seeds. A $0.005 smoke test checked the prompt text and menu first.

Paired against the tracker alone, 95% intervals over seeds:

| | Seeds | Chips | River cost | Missed bets | Sizing | Bluffs |
| --- | --- | --- | --- | --- | --- | --- |
| Tracker alone | 10 | | 360 | 137 (59) | 172 (35) | 45, 82% called down |
| + rule | 8 | +34 [-132, +200] | **-111 [-173, -49]** | 77 (42) | 137 (21) | 23, 78% |
| + menu | 9 | +52 [-57, +161] | -28 [-97, +42] | 107 (51) | 190 (30) | 37, 78% |
| + rule and menu | 9 | +42 [-118, +201] | **-100 [-185, -16]** | 73 (49) | 142 (24) | 25, 84% |

River bets in each arm:

| | River bets | Median size | All-in | All-in was best |
| --- | --- | --- | --- | --- |
| Tracker alone | 41 | 0.60 pot | 2 | 25 |
| + rule | 32 | 0.99 pot | 4 | 25 |
| + menu | 42 | 0.51 pot | 4 | 29 |
| + rule and menu | 41 | 0.73 pot | 8 | 28 |

**Reading.** The one-sentence rule works: it cuts river cost by about 100 bb/100 on the same deals, taking the tracker from 360 to about 250, close to own notes (300) at a third of the notes' cost. Most of that comes from betting winners it used to check (missed bets 137 to 77) and bluffing half as often. The menu alone does nothing (and its median bet is smaller: it lists the small sizes first), and adds nothing to the rule. **Sizing is still the big leak:** the rule moves the median river bet from 0.6 to 1 pot, but all-in was the best bet in about 25 of 40 river bets and the model shoved in 4 to 8. The cheat sheet's sizing cost is 30; every realistic arm is still at 137 to 190.

**Next idea:** a sharper rule (for example: against a player whose river fold rate is near zero, a strong hand should bet most or all of the stack), or a menu that lists the largest sizes first and includes 2 and 3 pots.

