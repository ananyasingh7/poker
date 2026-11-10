# Readable Minds: emergent opponent modeling in LLM poker agents

**Paper:** Hsieh-Ting Lin and Tsung-Yu Hou, [*Readable Minds: Emergent Theory-of-Mind-Like Behavior in LLM Poker Agents*](https://arxiv.org/html/2604.04157v1), arXiv:2604.04157v1 (5 April 2026). [Platform and analysis code](https://github.com/htlin222/agentic-holdem). This is an analysis of the linked preprint, not an independent replication.

## Why it matters here

This paper directly tests the idea that an agent needs persistent, selective memory to form and use opponent reads across hands. It is useful for designing memory ablations and instrumentation. It does **not** establish that the memory-equipped agents were more profitable, that their reads were reliably correct, or that the findings generalize across model families.

## Experimental setup

The authors ran a 2 × 2 experiment: persistent memory available or absent, crossed with structured poker strategy guidance present or absent. There were five sessions per condition, with three instances of Claude Sonnet (`claude-sonnet-4-20250514`) playing up to 100 hands per session, for approximately 6,000 agent-hand observations. Nineteen sessions reached 100 hands; one ended at 86 after elimination. Blinds were 50/100 and initial stacks 10,000 chips. Agents communicated through MCP tools with a TypeScript/Express poker server. The memory condition allowed agents to read and write a persistent JSON file; the no-memory condition prohibited file access. Skill guidance covered topics such as pot odds, position and ranges. “No skill” does not mean no poker knowledge: pretrained models may already know the game.

The authors coded memory text on an ordinal scale: 0, no opponent-specific content; 1, numeric statistics; 2, behavioral traits; 3, conditional predictions; 4, opponent-specific action plans; and 5, reasoning about an opponent's model of the agent. They call these *theory-of-mind-like* behaviors; text at a higher level is evidence of a stated model, not proof of a correct belief or human-like mental process.

## Reported results

- The session-level maximum ToM score averaged 3.0 in memory + skill and 2.8 in memory without skill; both no-memory conditions scored 0. The authors report Cliff's delta 1.0 and exact p = 0.008 for the memory comparison. Some individual agents reached level 5; the reported condition averages and individual maxima are different quantities.
- Opponent-model-grounded deception was reported in 18.2% of hands in memory + skill and 11.6% in memory without skill, versus 0% in both memoryless conditions. Simple bluffs occurred in all conditions. These are pooled hand rates, and hands within a session are dependent.
- Adherence to a simplified preflop tight-aggressive (TAG) chart was 67% with memory + skill and 79% with skill but no memory. The paper describes deviations as exploitation, but its methods explicitly say this chart is a conservative norm, **not a GTO solution**, especially in three-handed play. Lower adherence alone cannot establish better exploitation or higher EV.
- Final chip spread was 45,246 ± 19,702 in memory + skill versus 22,535 ± 23,147 in skill without memory; the paper reports p = 0.15. Spread is the difference between richest and poorest player, not an individual agent's win rate, and this comparison did not establish a significant profit advantage.
- Among 56 coded opponent labels, directional accuracy was 55.4%, with binomial p = 0.50. Label intensity correlated with magnitude of deviation from the table mean (Spearman r = 0.36, p = 0.005); that association does not establish directional prediction accuracy. In before/after behavior analysis, the paper reports a large standardized shift, but the paired Wilcoxon test did not reach significance (p = 0.26). These findings warrant cautious interpretation.
- A second model, GPT-4o, coded 127 sampled snapshots with weighted Cohen's kappa 0.81 against the primary Claude coder. This is agreement on the text rubric, not independent confirmation that the opponent reads were true.

## Assessment and limitations

The factorial design is a good starting point: it separates the availability of strategy guidance from the memory treatment, and the paper makes agent notes inspectable. It also offers a concrete hierarchy for studying how notes evolve from observations to predictions and intended adaptations.

The causal claim that memory is *necessary and sufficient* should be kept within this specific setup and measurement. Memory-equipped agents could write persistent notes, while controls were explicitly prohibited from using files; the authors acknowledge this instructional asymmetry and call for a reflection-without-persistence control. Furthermore, the primary rubric is applied to memory snapshots, so the no-memory condition lacks the same persistent artifact on which opponent-specific content is assessed. A zero score on this measure should not be interpreted as proving that no opponent reasoning occurred within a hand. Note text is observable agent output, not a direct window into a model's internal beliefs.

There are only five sessions per condition, all using one model family and same-model opponents. Session-level replication matters more than the number of correlated hand observations; pooling hands in Fisher tests can overstate precision if dependence is ignored. The authors report no multiple-comparison corrections. Natural language notes and model judges can both introduce biases; judge agreement measures consistency, not validity. Finally, a stated read, an action after a note, and a successful individual bluff do not isolate the causal effect or EV of the read.

## What we can use

1. **Make memory an experimental variable.** Compare no cross-hand memory, reflection without persistence, model-written notes, coarse statistics, and notes plus statistics. Keep current-hand information, instructions, budgets, opponents and game rules comparable. Treat an optional persistent cross-session profile as a separate treatment.
2. **Capture notes as versioned artifacts.** Record what was observable, the notebook before and after each hand, and which changes the agent chose. Keep private cards hidden unless actually revealed. Track note capacity, age and revisions; don't inject the evaluator's ground-truth opponent statistics into a notes-only condition.
3. **Evaluate reads independently of fluency.** Ask for testable, timestamped predictions and confidence where appropriate; score calibration against later eligible opportunities and known scripted opponents. Avoid hindsight and account for selection effects: folded hands are usually not revealed.
4. **Test whether notes change actions.** Replay the same legally observable decision with different controlled note states, or randomize memory availability, and compare actions. Then estimate profit against fixed opponents over independent sessions and rotated seats/deals, with uncertainty intervals. Behavior change by itself is not EV improvement.
5. **Measure strategy against appropriate baselines.** Record legality, win rate and uncertainty, action frequencies and opponent-specific results. A basic TAG chart is useful as a descriptive reference, not a GTO oracle. A solver baseline needs matching game rules, stack sizes, action abstractions and player count.
6. **Replicate across models and opponents.** Include different model families and deliberately varied, fixed archetypes, plus adaptive opponents. Track inference cost and latency alongside poker results. Report the unit of replication and use session-aware statistics.

The key takeaway for this project is that agent-authored memory is a promising, observable experimental interface. Whether it helps an agent predict opponents accurately and make more money remains an empirical question that our evaluation should test directly.
