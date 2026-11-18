# Related papers

Three 2026 preprints that shaped the design. Summaries of the authors' reported results, not reproductions.

## Poker Arena (arXiv 2606.13815)

Singla, Garg, Singh, [*Poker Arena: Multi-Axis Profiling of Strategic Reasoning and Memory in LLMs*](https://arxiv.org/html/2606.13815v1). Seven LLMs, 50 seven-player tournament sessions of 20 hands (1,000 hands), each model keeping selective notes, scored on chips and on judge-based diagnostic axes.

- Chips and the diagnostic score ranked models differently (Spearman 0.571, p = 0.18): fluent poker reasoning and winning were not shown to be the same thing.
- Memory ablation (600 hands): +115, −109 and −43 chips per session for three models, none significant. The data do not show memory helping or harming.
- Limits: tiny sample, escalating-blind tournament (not cash-game bb/100), model versions drifted during collection.
- **What we took:** a capped per-agent notebook with a post-hand update, storing opponent profiles rather than hand logs; memory as an experimental variable; profit reported beside fallbacks and update rates.

## PokerSkill (arXiv 2605.30094)

Li, Wang, Huang, [*PokerSkill: LLMs Can Play Expert-Level Poker without Training or Solvers*](https://arxiv.org/html/2605.30094v1). Heads-up against GTOWizard with AIVAT variance reduction, at least 5,000 hands per arm: expert-written situational guidance, action filtering and state extraction cut losses by about half (GPT-5.5 −132 → −57 mbb/hand, Claude Opus 4.6 −204 → −80). All still lost. A no-LLM rule baseline scored −132, the same as the best unscaffolded model. Cost $0.07-0.30 per hand.

- Limits: one opponent, heads-up only, several changes bundled together.
- **What we took:** compute state facts deterministically and offer them as a labelled treatment (our facts sheet, the largest thinking-off gain); ablate aids in stages; keep a rule-only baseline (`solid`) in every table; never let a strategic filter remove legal options silently.

## Readable Minds (arXiv 2604.04157)

Lin, Hou, [*Readable Minds: Emergent Theory-of-Mind-Like Behavior in LLM Poker Agents*](https://arxiv.org/html/2604.04157v1). 2 × 2 of persistent memory × strategy guidance, five sessions per cell, three Claude Sonnet agents, about 6,000 agent-hands.

- Memory agents wrote opponent models and opponent-grounded deception (11-18% of hands, 0% without memory), but stated reads were directionally right only 55.4% of the time (p = 0.50), and the profit difference was not significant (p = 0.15).
- Limits: five sessions per cell, one model family, no multiple-comparison correction, note quality judged by another model.
- **What we took:** score reads against opponents with known policies rather than against the model's own words (our cheat sheet and range-based grades); test whether notes change actions and money, not just text.

## Common lesson

All three find that models can *talk* about reads and strategy convincingly; none shows that talking translates into money at a significant level. This lab's design (known opponents, paired deals, decision grades, cost tracking) is aimed squarely at that gap.
