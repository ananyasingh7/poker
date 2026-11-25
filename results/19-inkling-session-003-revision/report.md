# Session 003 revision results

**Step 140 remains the table model.** All three replicas started from step 140 and completed the same 40 updates on the free Inkling 2 Mini tier. The earlier 75% bet mix was replaced with equal counts of bets and checks. The evaluation deals, legal actions, prompts and chip values are identical to the first Session 003 experiment.

| Policy | Winning bets taken | Expensive bluffs skipped | Both bluff prices right | Expected big blinds lost per decision ↓ |
| --- | ---: | ---: | ---: | ---: |
| Always bet | 64/64 | 0/32 | 0/32 | 2.517 |
| Step 140 | 55/64 | 8/32 | 6/32 | 4.754 |
| Original Session 003 representative | 64/64 | 0/32 | 0/32 | 2.517 |
| Revision 1 (183001) | 49/64 | 19/32 | 13/32 | 6.137 |
| Revision 2 (183002) | 47/64 | 16/32 | 14/32 | 9.869 |
| Revision 3 (183003) | 43/64 | 26/32 | 25/32 | 10.443 |

The median-loss rule selected seed 183002. Its average savings versus always betting were -7.352 big blinds per decision. Against step 140 the savings were -5.115, with a paired 95% interval of [-9.886, -1.199] across 32 situations. A positive savings value favors the revision. Always betting gives the correct answer on 96 of these 128 decisions.

On the earlier diagnostic, the representative handled both prices on 14 of 32 pairs and gave up 6.348 big blinds per decision. On the 96 historical controls, the betting-format averages were 100.0% of good bluffs taken and 91.7% of winning hands bet.

| Development rule | Result |
| --- | --- |
| Lower loss on winning hands | Fail |
| Positive paired savings over step 140 | Fail |
| Retained hand-reading accuracy | Pass |
| Retained step-140 bluff pairs | Pass |
| At least 18 earlier bluff pairs correct | Fail |
| Earlier mean loss at most step 140 | Fail |
| Retained historical good bluffs | Pass |
| Retained historical winning bets | Fail |
| Lower loss than always betting | Fail |

## Largest gains and losses on individual decisions

- Group 10, holding 5d 3s on Th Qh 4c 3c 2s. The pot was 186.5 big blinds, the allowed bet was 597.2, and the opponent folded 69.4% of the time. Step 140 chose to bet; the revision chose to check. This saved 53.31 expected big blinds relative to step 140.
- Group 10, holding Tc 3s on Th Qh 4c 3c 2s. The pot was 186.5 big blinds, the allowed bet was 597.2, and the opponent folded 69.4% of the time. Step 140 chose to bet; the revision chose to check. This gave up 182.74 expected big blinds relative to step 140.

The results apply to repeated development tests of synthetic river decisions. The sealed release challenge and reserved games remain available.

[Protocol](protocol.md) · [Full scores](results.json) · [First Session 003 results](../18-inkling-session-003/report.md)
