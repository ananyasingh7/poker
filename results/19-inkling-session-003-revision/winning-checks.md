# Why did the revised model check winners?

The representative missed 17 of 64 winning bets on 15 distinct deals. For every miss it chose “bet” when explicitly told the hand would win. The separate cards-only question split the misses almost evenly. Misreading the cards and failing to act consistently on a correct read both warrant attention.

| Model and test | Checked a winner | Read the cards as losing (count / expected bb forgone) | Read the cards as winning and priced the bet correctly (count / expected bb forgone) |
| --- | ---: | ---: | ---: |
| Step 140, fixed development | 9 | 9 / 369.98 | 0 / 0 |
| Revision 1, fixed development | 15 | 12 / 611.41 | 3 / 59.18 |
| Revision 2, fixed development (representative) | 17 | 8 / 526.43 | 9 / 626.56 |
| Revision 3, fixed development | 21 | 17 / 936.14 | 4 / 363.63 |
| Revision 2, earlier diagnostic | 9 | 5 / 273.04 | 4 / 226.74 |

Each fixed-development winning deal appears at two prices. The earlier diagnostic includes one winning decision per deal. Each comparison matches the full decision to the cards-only answer on the identical deal and the pricing-only answer at the identical price and fold chance. These were separate conversations. They reveal inconsistent answers across prompts; they cannot tell us what the model internally recognized while choosing its full action.

**Largest miss.** In group 10, tens and threes beat the opponent’s single pair of queens. Revision 2 said “win” on the cards and “bet” on the standalone price. It checked the full hand. The 597.2-big-blind bet gets called 30.6% of the time, so the check cost 182.74 expected big blinds. The three costliest missed bets all had correct standalone answers.

**A different failure.** In group 9, the model held 7♣ 6♦ on A♣ K♣ 9♥ 10♠ 8♥. It called its ten-high straight a loser against one pair. It correctly chose “bet” when supplied the winning label. Its full checks at the two prices cost 112.79 and 100.35 big blinds. These are two mistakes on the same card read.

**Would listening to its own read help?** In an exploratory offline replay, keep each recorded action unless the standalone card answer says “win”, in which case bet. Score every full decision, including false claims of winning. For Revision 2 on fixed development, mean loss fell from 9.87 to 5.41 bb per decision. The override introduced three costly bets and remained above step 140’s 4.75. On the earlier diagnostic it reduced Revision 2’s mean loss from 6.35 to 3.99. Both tests have already informed development.

**Training implication.** Keep step 140 as incumbent. For a next experiment, start from its observed weakness: recognizing winning hands in matched winning/losing deals. Test a read-then-act version alongside direct action, including losing hands that are easily mistaken for winners. Require the full policy to save chips on fresh development groups before advancing. The evidence does not support blindly betting whenever the model claims to win.

Reproduce with `python3 -B experiments/replay_inkling_winning_checks.py`. [All missed bets and matched answers](missed-winning-bets.jsonl.gz) · [Exact metrics](winning-checks.json)
