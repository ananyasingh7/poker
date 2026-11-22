# Roadmap

Current direction as of 2026-11-21. The full experiment history and numbers are in [results](docs/results.md).

## Where we are

- **Current model:** Inkling 2 Mini IQ4_NL, running locally on port 9932. Qwen/Pi, GLM and DeepSeek remain historical comparisons.
- **Built:** a verified six-seat poker engine, scripted opponents, model players, notes and action counts, resumable experiments, decision grading and a spectator table.
- **Latest finding:** Inkling used fresh notes to adjust to changing opponents in a controlled river test. Choosing the bet was the larger weakness: a fixed calculator using the same reads removed about 88% of avoidable chip loss. Exact recent counts added a smaller gain.
- **Still open:** when Inkling chose its own bets, separating reading from betting had no clearly resolved benefit. General learning at a six-seat table and fine-tuning gains remain unproven.

The overnight study finished all 96 controlled histories, 1,200 skill questions and 24 paired games. [Plain-language findings](results/11-inkling-overnight/takeaways.md), [detailed report](results/11-inkling-overnight/summary.md), [data index](results/README.md).

## Next experiment

**Let Inkling read the opponent. Let a calculator compare the bets. Keep a reliable count of what happened.**

For example, Inkling might say, “This player folds about one time in four.” The calculator checks whether a bluff would earn or lose chips on average. A counter remembers the actual calls and folds, so evidence does not disappear when the model rewrites its notes.

Test four versions on fresh river games:

| Version | What changes |
| --- | --- |
| Current setup | Inkling reads the opponent and chooses the bet |
| Calculator only | Keep Inkling's read; code chooses the bet with the higher expected payoff |
| Counts only | Supply a fold estimate from recent action counts; Inkling chooses the bet |
| Both | Supply the count-based estimate and let the calculator choose the bet |

In these games, Inkling's own choices determine which opponent responses it sees. That is the gap the overnight controlled test could not settle.

Before running, fix the new seeds, prompts, failure rules and main comparison. Measure chips given up through bad decisions and how quickly play changes after the opponent changes. Report uncertainty across whole games, including cases where the helper does not improve play.

**Fine-tuning comes after this test.** If useful mistakes remain, train on the saved, correctly graded training candidates and evaluate on untouched histories. Training has not been tested yet.

## Learning track

1. **Within-session adaptation.** Controlled Inkling test passed in both directions; benefit in own-choice play remains unresolved. Run the four versions above next.
2. **Across-session memory.** Keep notes for the same opponent across several sessions, then test on fresh cards. Open.
3. **Fine-tuning.** Train on decisions with justified labels, then compare the trained and original models on held-out data. Open.
4. **Full-table transfer.** Check whether gains survive harder opponents and six-seat play. River drills supply hand strength and use simple opponents; that help must be accounted for.

Earlier GLM learning hints failed replication. Qwen's local pilot did not show reliable adaptation. The old five-point percentage response format could distort reads; new tests use ordinary numeric percentages. [History and correction](docs/results.md)

## Other open work

- Human play through the spectator table.
- Local-model selection in the spectator UI; CLI support is already built.
- Compare direct model calls with the Pi agent harness on the same deals.
- Real mixed-model tables, with separate settings for each model.
- Harder opponents, including opponents that adjust to the model's play.
- Better grading before the river; river grades are exact and turn grades approximate under the lab's opponent assumptions.
- Host-owned inference proxy and stronger isolation for third-party players.
- Engine reference histories, an independent evaluator comparison and a PokerStars-style export.
- More than six concurrent subprocess sessions, Linux coverage and hand-level recovery for full-table runs. The standalone overnight runner already saves each request.

## Decisions on record

- Rust powers the engine, lab CLI and spectator table. Python runs the standalone river experiments.
- Inkling is the current workhorse. Local tests use temperature 0, ordinary numeric percentages and the native JSON answer prefix. See each saved protocol for exact settings.
- Local API fees are $0; runtime, output limits and failures are still recorded. Paid runs need a stated budget and cap.
- Fresh test seeds and an independent comparison are required for new claims. Test hands and hidden grading answers never enter notebooks.
- The overnight data separates training candidates from evaluation histories. Fine-tuning must preserve that separation.
- API studies used low-effort thinking, structured profiles and varied bots where their configs specify them. Those settings remain part of the historical results.
- The spectator table uses the sharp sizing rule. River and turn graders consider both orders of an opponent's cards.
- Account privacy settings stay unchanged; models blocked by those settings remain out of scope.
