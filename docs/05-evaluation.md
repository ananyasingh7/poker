# Evaluation and roadmap

## Questions to answer

- How profitable is each model-and-scaffold combination against a specified field?
- Can an agent identify known opponent tendencies, update its beliefs, and exploit them without overreacting to noise?
- What do notes, coarse stats, skills, or tools add individually and together?
- Does deliberate image manipulation earn more than it costs across an entire session?
- How do performance, latency, token consumption, and monetary cost trade off?

The evaluation may require very large samples, potentially a million hands or more. Candidate methods include bb/100, mixed fields, fixed archetypes, duplicate deals, logging of beliefs, factual errors, and memory ablations. Large samples help, but a fixed hand count is not itself evidence of precision: uncertainty depends on variance and on the experimental design.

## Recommended protocol

Define the game, opponents, model versions, prompts, information, bankroll rules, and primary comparison before a run. Rotate seats and opponents; use independent sessions and seeds. Where feasible, use matched deals or other variance-reduction methods without leaking hidden information. Report bb/100 with uncertainty intervals and the number of hands and independent sessions. Account for dependencies caused by persistent memory and repeated play; hands are not necessarily independent observations.

Use scripted opponents with known tendencies to assess calibration and response to evidence. Also test against strong baseline agents and mixed fields. Keep opponent design and evaluation cases separate from tuning where feasible. For direct model comparisons, match budgets and scaffolding and report exactly what each agent saw. Results are conditional on the field and format; multiplayer outcomes and changing opponents do not produce a universal model ranking.

Record invalid actions, fallback rates, timeouts, latency, tokens and cost alongside performance. Log stated beliefs and available evidence, but do not assume model-provided reasoning faithfully explains its internal decision process. Compare predicted tendencies with known policies or properly measured observations; unrevealed cards and selective showdowns limit what can be inferred.

## Phased plan

1. **Correctness first:** choose the rules; validate the engine and information boundaries with deterministic scripted agents. Implement structured actions, simple notes, coarse stats, and auditable logs.
2. **Small model pilots:** establish cost and latency, check legal actions and memory updates, and compare against simple fixed opponents. Use preliminary uncertainty estimates to plan scale.
3. **Controlled ablations:** add skills or tools and vary memory and information conditions independently. Test whether improvements come from the model, the scaffold, or their interaction.
4. **Scaling:** parallelize independent sessions, rotate seats and opponents, and expand samples according to desired precision and budget. Report uncertainty and costs.
5. **Advanced experiments:** fine-tuning, solver-informed baselines, and deliberate counter-exploitation only after a reliable baseline exists. Evaluate on held-out opponents and scenarios.

The discussion does not settle the implementation language, engine, actual prompts, statistical power target, or inference budget. Those are design decisions for the next phase.
