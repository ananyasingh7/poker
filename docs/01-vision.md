# Project vision

## The idea

The project envisions a table where different language models play poker against each other. The motivating question is whether their differences in mathematical reasoning, pattern recognition, opponent modeling, bluffing, and adaptation become visible when those abilities interact under uncertainty. The ambition is a demanding test of strategic intelligence, not simply a demonstration of fluent poker commentary.

The objective is to judge decisions by long-run expected value (EV), not whether a particular bluff succeeds. A profitable bluff can be called and lose; a bad bluff can work. Evaluating a decision requires assumptions about opposing ranges and responses, and those assumptions can be wrong.

Models can each provide persuasive justifications for mutually incompatible reads. A model might correctly exploit one player but misread another and lose by adapting; a more conservative model might miss profit against the first while avoiding the second mistake. Large samples, alongside appropriate controls, are needed to distinguish results from luck.

## An advanced idea: induce a counter-adjustment

One experimental strategy is to show a cheap, deliberate weakness in small pots, anticipating that observant opponents will adjust, and exploiting that adjustment in larger pots. For example, an agent might bluff too often early, then value-bet more when opponents start calling it down. This is a candidate for a separate experimental agent.

## Proposed direction

The proposed approach combines a strong balanced starting strategy, opponent-specific exploitation when supported by evidence, selective memory, structured actions, and a large evaluation harness. Detailed automatic statistics may bypass the skill of deciding what to notice and remember. One design combines a small automatic stats block with an agent-controlled, capacity-limited notebook, with experiments comparing different memory conditions.

Implementation details, a memory-selection mechanism, and complete prompts remain to be specified. The design remains a hypothesis, not a validated specification.

## Our assessment

The strongest research question is not simply “which model wins?” but **which capabilities and supporting tools explain its performance, against which opponents, and at what cost?** The system should preserve enough information to study those distinctions. Poker rewards adaptation and probabilistic judgment, but a poker benchmark alone cannot demonstrate AGI. Results will depend on the rules, field, available information, prompts, model versions, and resources.

## Claude feedback (2026-11-11)

- **Turn the vision into a few falsifiable hypotheses.** For example: (H1) a model wins more against a known-leaky scripted opponent than against a balanced one; (H2) notes raise winnings against fixed opponents over sessions of 100+ hands; (H3) a model with notes identifies an opponent archetype more often than chance. Each has a clear measurement and a clear way to fail. "Which capabilities explain performance" is the right question but cannot be answered until it is broken down like this.
- **The literature says the question is open.** Poker Arena found chips and its diagnostic score ranked models differently (Spearman 0.571, p = 0.18, seven models), and Readable Minds found stated opponent reads were directionally right 55.4% of the time (not distinguishable from chance). So fluent poker reasoning and winning poker are not yet shown to be the same thing. This project is well placed to test that directly.
- **The counter-adjustment idea needs an opponent that can adapt.** None of the current opponents do: the scripted policies are stateless and a memoryless model at temperature zero cannot change its behaviour across hands. The experiment only makes sense against (a) model opponents with notes enabled, or (b) a scripted opponent that tracks a statistic such as fold-to-bet frequency and adjusts. Build (b) first; it is cheap and its adaptation rule is known, so you can tell whether the induced adjustment actually happened.
- **Scope risk.** The vision lists reasoning, memory, bluffing, adaptation, skills, solvers, and fine-tuning. I would commit to the memory and opponent-exploitation question as the core result and treat the rest as later treatments.
