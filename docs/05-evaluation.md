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

The custom Rust engine now supports two through six scripted players. Actual prompts, statistical power target, and inference budget remain open. See the [milestone plan](../milestones/README.md) and [implemented rules](06-rules-and-protocol.md).

## Claude feedback (2026-11-11)

- **A first power estimate from real data.** In `runs/baseline-v1` (12 sessions × 10 hands) the focal model's bb/100 per session has mean +29 and standard deviation about 190, a standard error of about 55. That is roughly 6 bb per hand. For a 95% interval of ±10 bb/100 you need about 14,000 hands; for ±5 bb/100 about 58,000. These are rough (12 sessions, heavy tails) but they set the scale: the current 120-hand batches cannot support any strength statement, as the docs already say.
- **Cost is not the constraint for this model; time is.** At $0.000145 per hand, 58,000 hands is under $10. Run sequentially at about a second per hand it is around 16 hours. For frontier models at PokerSkill's reported $0.07–$0.30 per hand the same sample would cost $4,000–$17,000, so variance reduction matters there.
- **Session length is biasing positions.** The button starts at seat 0 every session, and 10 hands is not a whole number of six-seat orbits: seats 2–5 post the big blind twice and seats 0–1 once. Rotating the model through all seats balances this across a batch but not within a session. Use hand counts that are multiples of the seat count (12, 60, 120).
- **You already have a duplicate-deal design; analyse it as one.** Each seed is played with the model in every seat. Average the six seat results per seed and compute intervals over seeds, not sessions. With temperature 0 the paired comparison between conditions on the same seed is very tight (three of four memory conditions were chip-for-chip identical), which is exactly what a paired test wants.
- **Cheap variance reduction is available against scripted opponents.** Because their policies are known and deterministic, all-in equity adjustment (replace the run-out result with exact equity once betting is closed) is straightforward and unbiased. Full AIVAT is not needed to get a useful reduction.
- **Report behaviour next to profit.** The model's VPIP in the baseline was 29% (35 of 120 hands); of 235 decisions, 88 were folds, 100 checks, 24 calls and 23 raises. Numbers like these explain a win rate and catch degenerate play early. I computed them with a throwaway script; they should come from a Rust `analyze` command so the project stays Rust-only.
- **Count fallbacks as a bias, not just a contamination flag.** In the live six-seat subprocess run one of 54 decisions (1.9%) was rejected and folded although the model clearly meant to call. Fallback-to-fold costs chips, so models with sloppier formatting look worse at poker. Report results with and without fallback hands.
- **Pre-register.** Before the first powered run, write down the primary metric, the opponent field, the stopping rule and the comparison. Poker Arena and Readable Minds both report many uncorrected tests with p-values near 0.1.
