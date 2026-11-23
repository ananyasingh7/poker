# Roadmap

Current direction as of 2026-11-23. The full experiment history and numbers are in [results](docs/results.md).

## Next experiment

[Session 002 plan](docs/inkling-session-002.md). Session 001 trained and exported a poker adapter. Step 140 improved local river decisions and got both sides of 6/12 fresh cards-only bet-size pairs correct. The next experiment targets this gap. [Session 001 report](results/14-inkling-finetuning/session-001.md)

## Research plan before Session 001

- **Current model:** Inkling 2 Mini IQ4_NL, running locally on port 9932. Qwen/Pi, GLM and DeepSeek remain historical comparisons.
- **Built:** a verified six-seat poker engine, scripted opponents, model players, notes and action counts, resumable experiments, decision grading and a spectator table.
- **Latest finding:** updating reads beat freezing them in own-choice games, in both switch directions and both samples. Recent counts plus a calculator performed best among the four main setups; that version uses code for both steps. These comparisons are descriptive.
- **Still open:** a calculator alone missed the predeclared success rule. Keeping notes across sessions and occasional test bets had no clear overall benefit. General learning at a six-seat table and fine-tuning gains remain unproven.

The latest run finished all sections in 7.7 hours: 32 main and 32 fresh-repeat games across six versions, 12 memory identities, 512 exam questions, scarce-evidence and gradual-change tests, and 400 seeds across five code-only policies. [Plain-language findings](results/12-inkling-own-choice/takeaways.md), [detailed report](results/12-inkling-own-choice/summary.md), [data index](results/README.md).

## Earlier assisted-player proposal

**Build an assisted Inkling player, then identify what still needs training.** No new run or training is scheduled.

Supply reliable hand facts, recent counts and calculated chip values, then let Inkling choose. Use the code-only counts-plus-calculator player as the reference. The full assisted combination still needs testing in sequential play.

The completed study compared these versions, plus two that froze their reads before the opponent changed:

| Version | What changes |
| --- | --- |
| Current setup | Inkling reads the opponent and chooses the bet |
| Calculator only | Keep Inkling's read; code chooses the bet with the higher expected payoff |
| Counts only | Supply a fold estimate from recent action counts; Inkling chooses the bet |
| Both | Supply the count-based estimate and let the calculator choose the bet |

Each version's bets determined which opponent responses it saw. The fresh repeat used the same fixed comparison, not a winner selected from the first sample.

Priorities:

1. **Reliable help first.** In the limited betting exam, supplied chip values eliminated errors. A rule alone did not reliably fix them.
2. **Memory that can change its mind.** Across-session notes had mixed results. Compare recent evidence against stale notes rather than assuming more memory helps.
3. **Small fine-tuning pilot later.** Target repeatable residual mistakes and use untouched evaluation groups. The exam split accidentally separated bluff-threshold margins as well as board ranks; balance fresh groups before training. Keep the completed study's original split unchanged.

## Learning track

1. **Within-session adaptation.** Controlled Inkling test passed; fresh reads also beat frozen reads in own-choice play, descriptively, in both directions and samples.
2. **Across-session memory.** Completed across 12 identities; no clear overall benefit. Stable and changing opponents differed, with only three identities per condition.
3. **Fine-tuning.** Train on decisions with justified labels, then compare the trained and original models on held-out data. Open.
4. **Full-table transfer.** Check whether gains survive harder opponents and six-seat play. River drills supply hand strength and use simple opponents; that help must be accounted for.

Earlier GLM learning hints failed replication. Qwen's local pilot did not show reliable adaptation. The old five-point percentage response format could distort reads; new tests use ordinary numeric percentages. [History and correction](docs/results.md)

## Other open work

- Human play through the spectator table.
- Broader local-model selection beyond the Session 001 spectator preset.
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
