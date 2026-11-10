# PokerSkill: structured poker guidance for LLMs

**Paper:** Boning Li, Baoxiang Wang, and Longbo Huang, [PokerSkill: LLMs Can Play Expert-Level Poker without Training or Solvers](https://arxiv.org/html/2605.30094v1), arXiv:2605.30094v1. [Authors' code](https://github.com/lbn187/PokerSkill). This is a research preprint; results below are the authors' reported results, not an independent reproduction.

## Question and central idea

Can a language model play strong heads-up no-limit Texas hold'em without poker-specific model training or live solver queries? The authors argue that models can describe poker principles yet fail to bind the right principle to a particular decision, misread state, or choose invalid actions. They test a system that gives the model expert-written, situation-specific guidance and restricts its choices to a strategically plausible set. This causal explanation is their hypothesis; the experiment demonstrates a combined-system improvement, not which cognitive mechanism caused it. [Paper, sections 1 and 3](https://arxiv.org/html/2605.30094v1#S1).

“Solver-free” describes deployment: no solver is called during play. It does **not** mean the system lacks outside poker expertise or that its rules are proven equilibrium strategies. The authors say experts designed the library using their knowledge of GTO theory and solver study. The model is neither trained nor fine-tuned by this framework. [Section 3.2](https://arxiv.org/html/2605.30094v1#S3.SS2).

## How the agent works

1. A deterministic context engine reads cards, position, stack-to-pot ratio, board features, action line and accumulated betting pressure.
2. It retrieves only matching expert-written fragments. Five prompt layers cover the protocol; preflop ranges; general postflop principles; situation-specific board, hand and action-line guidance; and river blockers. The authors describe 12 preflop scenarios for 200-big-blind heads-up play, roughly 60 action-line scenarios, 23 hand classes and 46 pressure thresholds.
3. Attack and defense budgets encode how much pressure a hand class can apply or withstand over streets. These and other rules produce a set of strategically viable actions; draw defense uses separate pot-odds thresholds. Special board conditions can override ordinary budgets.
4. The LLM selects within the proposed choices using structured tool output; a validator checks the final action. Context is maintained within a hand. The experiment treats hands as independent, rather than evaluating a persistent opponent-learning notebook. [Sections 3–4](https://arxiv.org/html/2605.30094v1#S3).

The filter is not merely a formatting aid: it supplies poker judgment and can remove legal actions. Consequently, the measured agent is the **model plus expert scaffold**, not the model alone. A scaffold error can prevent a useful exploit or bias a model comparison.

## Experiment and results

The authors tested three frontier models in heads-up play against GTOWizard. Each PokerSkill and default-prompt evaluation used at least 5,000 hands. GTOWizard's AIVAT estimator reduces outcome variance using information about a reference policy; the authors report results in milli-big-blinds per hand, with the ± figures representing **one standard error**, not 95% confidence intervals. One bb/100 equals 10 mbb/hand. [Sections 4 and appendix F](https://arxiv.org/html/2605.30094v1#S4).

| Model | Default prompt, mbb/hand | PokerSkill, mbb/hand | Reported reduction in loss |
| --- | ---: | ---: | ---: |
| GPT-5.5 XHigh | −132 ± 25 | −57 ± 21 | 57% |
| Claude Opus 4.6 | −204 ± 44 | −80 ± 29 | 61% |
| Claude Opus 4.7 | −170 ± 28 | −87 ± 64 | 49% |

All three still lost to the benchmark. For intuition, −57 mbb/hand is −5.7 bb/100. A rule-based ablation choosing the first viable action without an LLM scored −132 ± 19 mbb/hand. That comparison suggests model choice within the scaffold matters, although it does not isolate each individual component or explain precisely why it helps. The paper also reports Slumbot at −194 ± 41 against the same benchmark, but explicitly identifies that comparison as **indirect**, not a head-to-head match against PokerSkill. The authors say the difference between scaffolded GPT-5.5 and Claude Opus 4.6 is not statistically significant at their sample sizes. [Table 1 and section 4.3](https://arxiv.org/html/2605.30094v1#S4).

The reported cost was approximately $0.30 per hand for GPT-5.5 XHigh and $0.07 for either Claude configuration, with the provider versions and reasoning settings used in April–May 2026. These are experiment-specific, not current price estimates. At those rates, one million hands would cost approximately $300,000 or $70,000 respectively, before other costs; this illustrates why a million-hand frontier-model tournament needs an explicit budget. [Appendix F](https://arxiv.org/html/2605.30094v1#A6).

## What this does and does not establish

**Evidence:** context-conditioned expert guidance, strategic action filtering and validation together substantially reduced losses versus the paper's default-prompt baselines against one strong heads-up opponent. The simple rule-only baseline also provides a useful, albeit narrow, comparison.

**Limits and interpretation:**

- The sole evaluation opponent is GTOWizard, chosen partly because its AIVAT support makes evaluation affordable. Performance against one opponent is not an exploitability measurement or a general ranking across fields, and no multiway or opponent-adaptation result follows.
- Heads-up and its 200-BB preflop guidance cannot simply be transplanted to a six-player table. Position, ranges, pot odds, and multiway incentives change.
- The comparison changes several things at once: feature extraction, expert advice, action filtering, and potentially presentation of the state. It does not establish that any one part causes the improvement, or that unconstrained LLMs lack strategic knowledge.
- AIVAT is valuable where the required reference-policy information and estimator assumptions are available; the authors' approximate 30× variance-reduction claim is specific to their setting. It is not a general substitute for raw results or uncertainty estimates in an arbitrary adaptive multiplayer arena.
- A fixed expert-authored filter can improve play while concealing important differences between models. It can also discard high-EV deviations against particular opponents. GTO-informed heuristics should not be labeled a proven GTO baseline.
- The rule-only comparator always selects the first viable action. A stronger rule-based or randomized policy might narrow the gap; this ablation alone does not establish the ceiling of non-LLM systems.

## Takeaways for our project

1. **Separate benchmark modes.** Run a common legal-action-only baseline to assess model behavior, and a separately labeled PokerSkill-like assisted mode to assess complete agent systems. Keep prompts, information and resources comparable within each mode.
2. **Start with objective grounding.** Compute legal actions, pot, stacks and card/hand facts deterministically. Retain the actual legal action set independently of any optional strategic recommendations so errors and excluded options can be audited.
3. **Ablate in stages.** Compare state formatting, descriptive features, advice, and action filtering independently; include a rule-only baseline and log invalid outputs, fallback rates and action exclusions. The paper's bundled improvement motivates this experiment but does not replace it.
4. **Preserve room to exploit.** If a strategic filter is used, first test an advisory version and log what a hard filter would exclude. Evaluate whether constraints prevent profitable adjustments against known leaky opponents.
5. **Use the paper as a heads-up reference, not a multiplayer specification.** Validate any adapted heuristics and ranges for the actual player count, effective stacks, rake and betting structure; consult the authors' code and license before reuse.
6. **Plan sample efficiency and costs.** Report raw chip outcomes with uncertainty, use matched deals or other justified variance reduction when compatible with the design, and record inference cost and latency alongside strength. Do not assume AIVAT is available for all opponents.
7. **Test memory separately.** This paper is strong evidence for the value of a particular decision scaffold, not for persistent opponent notes or deliberate counter-exploitation. Those are separate hypotheses requiring fixed and adaptive opponents and controlled memory ablations.
