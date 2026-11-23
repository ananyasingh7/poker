# Session 002 · Does Inkling price a bluff?

Status — planned. Parent — Session 001, step 140. The new experiment can use generated examples without another overnight gameplay run.

**Question.** With a losing hand, does Inkling risk a small bet yet decline a large one against the very same opponent? In the first local transfer check it got both decisions right on 7 of 12 pairs with hand strength given and 6 of 12 with cards alone. The plain practice quiz admitted a shortcut because 30 of 31 fold percentages predicted a single action on losing hands.

1. Generate independent pot sizes, bet sizes and fold probabilities. For each losing hand build a cheap and an expensive bluff around break-even. Include near-threshold decisions, several board textures and value hands. Use the chip formula to grade each choice. Exclude exact ties.
2. Keep matched pairs and related boards together when splitting training, development and fresh evaluation data. Keep Session 001’s sealed exam and reserved games out of development.
3. Freeze the new evaluation and score the original model and step 140 locally before training. Record the prompt, seed, decoding and hashes. Mix in earlier river and hand-reading examples during refinement.
4. Compare all three players on identical inputs. Count pairs where both actions are right, expected chips lost, profitable bluffs taken, bad bluffs taken, value bets and invalid answers. Report uncertainty over independent situations. Select only on development. Retain step 140 if the new method fails to improve fresh sizing decisions or sacrifices earlier skills.
5. After freezing a release candidate, evaluate the sealed exam and 48 matched game seeds with the existing release criteria. Broader tables and consenting expert players come after that. Record adapter, loading instructions, limits and exact evaluations for a Hugging Face release.

[Session 001 results](../results/14-inkling-finetuning/session-001.md)
