# Session 003 revision · Practice checking expensive bluffs

The first Session 003 representative bet on all 128 full development decisions. This revision tests whether changing how often checking is rewarded produces a player that can still collect winning bets.

Use the original 128 training groups. For each losing hand, retain the cheap profitable bluff and expensive unprofitable bluff, then add a second distinct unprofitable price around the same break-even point. For each winning hand, retain one of its two prices, alternating cheap and expensive across groups sorted by fold rate. Make the same replacements in both pricing formats. Preserve the two hand-reading questions per group. This changes the 1,024 training action labels from 768 bets and 256 checks to 512 bets and 512 checks. The other 256 training questions still ask who wins. The method changes label frequency and the distribution of prices and hand strengths together.

Use the original step-140 parent, shuffle seeds, 40 updates, optimizer settings, tokenizer and verified Tinker free model ID. The original Session 003 development questions and baseline answers are read directly from their original files. The test still rewards betting 75% of the time. Because the previous results motivated the revision, this test measures development progress. It cannot establish independent generalization.

The reference policy answers bet to every betting question. On the 128 full decisions it collects 64 of 64 winning bets, skips zero of 32 costly bluffs and gives up 2.516879 big blinds per decision. Compare every replica against that policy and step 140. Keep the earlier median selection rule and all Session 003 controls. Additionally require lower mean loss than always betting on the same full decisions. The release challenge remains reserved.

Run `python3 -B experiments/inkling_session003_revision.py prepare` to reproduce the mix. Use the training environment's Python with `train` and local Python with `score`.

[Previous experiment](../18-inkling-session-003/report.md) · [Fixed development questions](../18-inkling-session-003/dev.jsonl.gz) · [Training data](train.jsonl.gz) · [Always-bet benchmark](always-bet.json)
