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

