# What we learned about Inkling

**Complete:** all sections, including the optional gradual-change test, finished in 7.7 hours. There were 26,878 unique local requests. Five unique note answers were truncated; cached reuse produced six fallback note updates. No action or read answers failed.

## Main findings

- **Inkling adapted during play.** Updating its read beat freezing it before an opponent changed: 1.189 bb per decision saved in the first sample and 1.066 in the fresh repeat. Both intervals were above zero, and both change directions improved. These are descriptive comparisons in simple river games; weights did not change.
- **Counts plus a calculator were the strongest practical setup.** Compared with Inkling's own read and bet, they saved 0.866 bb per decision in the first sample and 1.257 in the repeat, with intervals above zero. This version uses code for both steps, so its gain is not model learning. These comparisons are descriptive.
- **The calculator alone missed our predeclared success rule.** First sample: +0.419, interval [−0.017, +0.869]. Repeat: +0.528, interval [+0.134, +0.943]. Both samples had to pass; they did not.
- **Keeping notes across sessions had no clear overall benefit.** Saving: −0.021 bb per decision, interval [−1.029, +0.708], across 12 identities. Stable and changing opponents behaved differently; each condition had only three identities.
- **Occasional test bets had no clearly resolved net benefit.** Their full chip costs were included.
- **Giving Inkling the calculated chip values removed all mistakes in this exam:** 32/32 held-out questions and 96/96 development questions. With only the fold chance and hand strength, it got 21/32 held-out choices right. This is a limited drill, not proof of perfect betting or full-table play.
- **More memory was not automatically better.** Among 400 fresh code-only games, keeping the last 20 observed responses beat keeping 60 or all observations. These are actual separately played code policies, not model learning.

## What to do with this

Use recent counts plus a calculator as the reference baseline. For Inkling, prioritize reliable hand facts and calculated chip values before fine-tuning its final choice. The data support in-game adaptation; lasting memory benefits and fine-tuning gains remain unproven. No further model runs or training were started during this review.

## Caveat found in review

The exam's every-fourth-group split accidentally put only questions 10 percentage points below the bluff threshold in the held-out set. Development questions used other margins; board ranks also differed by split. Comparisons between formats on the same held-out questions remain paired, but development versus held-out accuracy is not a clean generalization comparison. Balance fresh groups before a future training study; retain this run's original split and results unchanged. The 384 training candidates contain no evaluation examples.

[Detailed report](summary.md) · [Extra tests](extra-analysis.json) · [Independent audit](audit.json)
