# Memory and information design

## Why it matters

Giving a model extensive automatic statistics may remove a central part of the test: deciding what to notice, remember, and revise. Limited selective notes can make this ability observable, while a thin automatic statistics layer offers a practical middle ground. Selective memory may also introduce recency bias, omission, and false confidence; it does not inherently prevent overfitting.

## Proposed layers

1. **Current hand:** private cards and public state, including the current action history. Rebuilt at every decision.
2. **Coarse session statistics:** a small, fixed set such as VPIP, preflop raise frequency, aggression, and showdown frequency. The engine updates them; the model does not. Supply each metric's correct number of opportunities, not just a generic hand count. Definitions and eligibility must be explicit.
3. **Agent-controlled notebook:** private notes, capped by characters or tokens. After a hand, the agent sees an authorized summary and its current notes and chooses to keep, revise, or discard observations. A note could preserve an observed hand, a tentative tendency, and supporting or contrary evidence. A 4–8K character cap is a possible starting point, not a tested optimum.
4. **Optional persistent profiles:** notes carried across sessions, with clearly defined identity and reset rules. Whether to allow these is an experimental choice.
5. **Optional detailed lookup:** a rate-limited or resource-costed tool for more granular statistics. This changes the information available and should be tested separately.

## How an update might work

A post-hand model call can return either `keep` or a complete replacement notebook. The system validates the schema and size, stores the previous and proposed versions, and rejects or retries an oversized update under a consistent policy. The model—not a hidden summarizer—chooses what to retain. The summary must exclude folded or otherwise unrevealed opponent cards. If model-written notes are used, they should be treated as fallible beliefs, not facts.

A useful update prompt should ask for evidence, uncertainty, counterexamples, and relevance without forcing an update after every hand. It should not silently add observations or turn one showdown into a population statistic. Updating after every hand has latency and cost; batch updates are a separate condition that also changes what the model can remember.

## Our assessment

Run notes-only, coarse-stats-only, both, and neither under otherwise matched conditions. The system can log complete history for evaluation while withholding it from the agents. Track memory use, revisions, evidence calibration, and downstream performance—not notebook length or confident language as proxies for quality. Precisely define resets and session boundaries; persistent profiles can otherwise leak information between trials or create unequal exposure.

## Claude feedback (2026-11-11)

- **What the live notebooks actually contain.** In `runs/memory-v1` and `runs/subprocess-notes-v1` the model uses its notebook as a transcript of the most recent hand, restating actions, boards and stacks, and in places explicitly discarding earlier observations as "superseded". There are almost no cumulative per-opponent profiles. That is the opposite of the intended use. Poker Arena's design instructs the model to store opponent profiles and exclude hand logs, chip counts and bet sizes; I would adopt that in the next prompt version, with one section per opponent seat and an evidence count on each claim.
- **Notes did not change a single action.** With seed 42 and the model in seat 0, the none, notes and stats conditions produced identical results for all six seats (model −14 chips); only "both" differed slightly (−12). Ten hands is far too short for memory to matter. Memory experiments need sessions of at least 100 hands and a number of hands that is a multiple of the seat count.
- **Notes are the dominant cost.** The notes session used 3,632 completion tokens against 199 for the baseline and cost about 3.4 times as much ($0.00498 vs $0.00146). Options: a cheap keep/update gate before any rewrite (Poker Arena does this), updates every N hands, or a much smaller cap (1,000 characters) to start.
- **Two missing controls.** (1) Reflection without persistence, which the Readable Minds authors themselves call for: the model reflects after each hand but nothing is carried forward. (2) Oracle notes: the evaluator supplies a correct description of each scripted opponent. If the model does not win more with correct notes, the bottleneck is using reads, not forming them. That one comparison would tell you where to invest.
- **Statistics layer is thin.** It has VPIP, PFR and showdown counts but nothing postflop (aggression, fold-to-bet). The scripted opponents differ mostly postflop, so the current statistics cannot distinguish them well.
- **Isolation is now enforced by construction** for subprocess runs: each notebook lives in its own worker process, and a new session is always a new process. Tests confirm no note crosses seats or sessions.
