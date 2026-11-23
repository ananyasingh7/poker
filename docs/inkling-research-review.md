# Inkling poker research review

Repository evidence through the completed November 22, 2026 own-choice study. The assessment concerns Inkling 2 Mini IQ4_NL playing with the recorded prompts and runtime. “Regret” below means the expected big blinds given up by an action relative to the best available action in the drill. A sampled pot won or lost is a different quantity.

## What the overnight runs measured

In study 11 every controlled history contains 120 observed responses. Every 20 hands Inkling rewrites a notebook, estimates the current fold probability and chooses actions on four independent-of-the-notebook probe deals. A frozen arm uses its hand-60 estimate on precisely the same probes. Of 96 histories, 48 contain a hidden change after hand 60. The fresh estimate saved 1.003 bb per post-change decision, with a whole-history 95% interval of 0.729 to 1.281. Both directions improved. This passed the declared test. In the 24 own-choice pairs, splitting reading from betting saved 0.081 bb with an interval of −0.161 to 0.362. These contrasts answer different questions.

The following reconstruction uses the mean of the 24 notebook reads in each changed condition at each checkpoint. The true fold chance is 20% for a caller and 80% for a folder.

| Responses observed | 20 | 40 | 60 | 80 | 100 | 120 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Caller becomes folder, estimated fold % | 22.5 | 19.3 | 24.2 | 67.0 | 75.8 | 76.3 |
| Folder becomes caller, estimated fold % | 78.7 | 78.6 | 78.7 | 29.5 | 27.5 | 26.4 |

Study 12 lets each policy choose what it observes. A check reveals no response to a bet. Each matched seed supplies the same deals and random response draws across versions. Model notes update after each completed block of 20 hands except the final block. Frozen versions stop updating after hand 40. The hidden abrupt change comes after a randomly selected hand from 48 to 72. The comparison therefore includes the benefit of any additional pre-switch updates as well as post-switch evidence. The controlled study isolates the latter more cleanly. Own-choice fresh reads saved 1.189 [0.599, 1.884] and 1.066 [0.426, 1.798] in the two samples of 16 changed seeds each. These are descriptive results. Each arm follows its own trajectory after actions diverge.

### Where the read stops helping

These descriptive windows pool core and replication. The numbers are seed means averaged equally across the 16 matching seeds per direction. The final window uses the available hands through hand 120, so its length varies by seed.

| Hands after switch | Estimated fold % | Bet frequency | Regret, bb/decision |
| --- | ---: | ---: | ---: |
| Caller becomes folder, 1–20 | 27.1 | 71% | 1.69 |
| Same, 21–40 | 52.2 | 97% | 0.17 |
| Same, 41–60 | 72.9 | ~100% | 0.02 |
| Folder becomes caller, 1–20 | 69.9 | 99% | 4.46 |
| Same, 21–40 | 39.4 | 89% | 3.67 |
| Same, 41–60 | 28.9 | 82% | 3.70 |

The late model arm bet on 156 of 208 losing hands where its **own supplied estimate was below 50%** after a folder became a caller. This count pools decisions and has no independent-hand significance interpretation. In a pot-sized bluff with no showdown equity and no raises, the incremental bet value over checking is p × pot − (1 − p) × bet. Below 50% it is negative. This identifies an action error even without knowing the true opponent policy. The descriptive slice was selected during this review and needs prospective replication. One easy way to appear excellent against a chronic folder is to bet almost everything. Success in that direction alone would give a misleading account of skill.

Across all controlled histories in the final half, fixing the calculator while holding the read fixed saved 0.903 [0.660, 1.144], about 88% of the split policy's avoidable loss. Using recent exact counts with that calculator saved a further 0.125 [0.052, 0.219]. In study 12 the calculator alone missed its preregistered two-sample criterion. The core interval included zero. Counts plus calculator saved 0.866 [0.410, 1.345] and 1.257 [0.853, 1.710] on whole-game regret. Both steps of that arm are code. Its gains include its choices about which responses become observable. Counts refresh per decision using the last 20 observed bets and Laplace smoothing. Model notes refresh per 20 dealt hands, even when few bets took place. A matched-cadence ablation would identify how much of the difference comes from recency versus representation.

Carryover versus reset across sessions gave −0.021 [−1.029, 0.708] bb saved across 12 identities, with only three per condition. Both used a calculator to act. Stale reads are a plausible failure mode worth testing. Occasional paid probes had unresolved net value. In 400 additional paired seeds the code-only last-20 policy beat last-60 and cumulative counts. That is evidence about this rate of opponent change. In real play, 20 observed bets of one size on one street can take a long time to collect.

## What to reuse

| Material | Role in the next study |
| --- | --- |
| Studies 1–4, pilots, thinking-off DeepSeek, Qwen and spectator sessions | Replay corpus, legal-action tests, comparative facts-sheet baselines, truncation and bet-sizing cases. The successful local Inkling table checks are only eight hands on seed 901. |
| Studies 5–8, thinking, fresh-deal lineup, varied bots and GLM replication | Templates for matching opponents and cards, checking helper effects on new deals, and recognizing a false learning curve. GLM's original notes-versus-cheat-sheet learning signal of +393 bb/100 reversed to −72 [−396, +253] on 40 new seeds. |
| Studies 9–10, local Qwen and Inkling | Prompt regression suite. The old five-point numeric grammar distorted reader outputs. Native JSON prefill resolved short-budget action failures. Preserve runtime and prompt settings in comparisons. |
| Study 11, tape, checkpoints, probes, online hands | A frozen benchmark for changes in reading, betting and exposure. Keep controlled and own-choice histories out of training. |
| Study 12, hands, notes, episodes and extras | Frozen closed-loop evaluation, split audit, calibration and discovery of failure classes. Use the code-only trajectories as baseline results. |
| Allowed training exports | Study 11 has 720 rows from 60 groups. Study 12 has 384 from 48 groups. Within each study a group repeats related cards, strengths or prompt variants. Reconstruct prompts from visible data and the executed template. Recompute rewards and allow ties. |
| Rust engine, player view, CLI and spectator frontend | Deterministic dealer, legal moves, seat rotation, replay, matched experiments and viewing. The human-seat client is still roadmap work. |

Re-running the table catalog gave 77 rows and 101,386 recorded hands summed over its populated hand-count fields. Scanning the physical evaluator files gave 104,316 hand records in 1,793 files, all successfully replayed. Keep cataloged outcomes, raw attempts and completed independent games distinct. The two Python overnight databases contain 16,716 and 26,878 unique cached requests respectively. Each contains five unique failed responses. These were note truncations. Study 12 reused one, causing six fallback updates. Study 12's main export contains 61,440 arm-hand rows over 96 seeds. Another 240,000 decisions came from code-only memory policies. The size of the archive is mostly an advantage for evaluation.

## A training dataset worth building

1. **Pick the decision.** Begin with a visible fold estimate, pot, legal bet, hand or legal hand facts, and a check/bet output. Generate both profitable and costly bluffs, values and ties. Weight or sample by economic significance while preserving a realistic distribution in evaluation. Give the baseline and tuned model identical information. Add matched situations where only the estimated probability, bet size or one card changes. Include tests where the correct choice reverses.
2. **Own the labels.** Store visible inputs, teacher action values, valid-action mask, optimal action set, actor, generator revision, group, seed and split as separate fields. Train on a grader-supported target. In study 11's development export, 112 recorded choices differ from the canonical best action but only 75 have positive loss. In study 12's development export all 53 positive-loss choices are on weak hands. Realized winnings, oracle bot profiles, hidden hole cards, future actions, true scripted probability and private model traces are unsuitable player inputs.
3. **Remove shortcuts.** The generator makes strong hands with pocket pairs and weak hands with distinct ranks. A model can pass a card-reading task using only whether its hole cards match ranks. Generate pocket pairs that lose, unpaired holes that win, board plays, ties, straights, flushes, blockers and plausible ranges. Split by underlying situation before rendering variants. Balance threshold side and margin, pot, sizing, strength, board texture and target action within each split. Hold out entire opponent families and generators too. Study 12's original held-out exam contains only the −10-point margin. Historical held-outs have now been examined repeatedly. Reserve new sealed tests.
4. **Train and attribute.** First use exact-payoff questions to verify the training pipeline and raw-probability questions to test learning. Compare original, tuned, original with helpers, tuned with identical helpers, and code alone. Use a common renderer, action schema, precision and effort within each comparison. Test multiple adapter seeds. Log training curves, legal-action failures, regret by group and input format, and worst-performing slices. Run each policy online on paired seeds. Logged trajectories cannot reveal how a different betting policy would have gathered evidence.
5. **Broaden the teacher.** The two-action label is exact in this narrow drill. For earlier streets and uncertain holdings, target expected values and mixed action frequencies from an information-set teacher with specified ranges, stack size, action abstraction and continuation policy. For mixed targets, train a distribution over legal actions and verify sampling frequencies. The Rust river grader tries a candidate size menu and samples up to 400 joint holdings when the product of ranges is large. Turn continuation is approximate. Record label uncertainty and downweight close calls. The hindsight call/fold grade uses actually held cards and needs a range-based replacement before it can supervise ordinary decisions.

## Release and live evaluation

The upstream checkpoint is thinkingmachines/Inkling-2-Mini, licensed Apache 2.0. Record the base revision, tokenizer, renderer, checkpoint hashes and local GGUF hash. The archived service identifies IQ4_NL by filename and its model-list digest is blank. An inference deployment and a trainable upstream checkpoint can behave differently. On a small fixed prompt set measure the original training-runtime base against local IQ4_NL before training. Tinker officially lists the Mini and provides an Inkling renderer and LoRA workflow. Verify a tiny train–export–reload round trip on the intended serving stack before allocating a full run. Preserve license and applicable notices. State separate licenses for code and dataset. Publish the adapter, loader, example legal player view, test manifest, evaluation command, model card and helper configurations. A raw model score and a score with externally calculated action values answer different buyer or research questions.

- [Upstream model and license](https://huggingface.co/thinkingmachines/Inkling-2-Mini)
- [Inkling renderer and training guidance](https://tinker-docs.thinkingmachines.ai/cookbook/inkling/)
- [Supported training models](https://tinker-docs.thinkingmachines.ai/tinker/models/models_and_pricing/)
- [Export workflow](https://tinker-docs.thinkingmachines.ai/cookbook/deployment/export-hf/)
- [Hugging Face model cards](https://huggingface.co/docs/hub/model-cards)

Assume six-handed no-limit cash for the exhibition because the lab is built for that format. First play complete hands against the existing varied field on fresh seeds. Cross base/tuned with fresh/frozen memory. Rotate seats, preserve action legality, tabulate raw bb/100, adjusted bb/100, opportunities by street, sizing, regret and latency. Test the ability to decline bad bets and the ability to take good ones. Include static opponents as a negative control. Then add hidden ranges that respond differently by size, adaptive opponents that exploit predictable frequencies, unfamiliar stacks, multiway play and unrestricted opponent sizes. Reserve a stronger independent opponent or evaluation pool the training team cannot tune against.

Elite play requires a competent default strategy when observations are scarce. Pluribus used a self-play strategy and search and achieved its six-player human result without opponent-specific adaptation. ReBeL's convergence argument concerns two-player zero-sum games. A heads-up solver is a good teacher for heads-up decisions with compatible ranges and sizes, and a poor authority for a six-way preflop pot. PokerSkill provides a nearer LLM benchmark for testing whether help changes competitive losses. A model can improve measurably against a strong reference and still be a losing player.

- [Pluribus, original six-player study](https://noambrown.com/papers/19-Science-Superhuman.pdf)
- [ReBeL, original paper](https://arxiv.org/abs/2007.13544)
- [PokerSkill, original study](https://arxiv.org/abs/2605.30094)

Build a private human-seat interface that separates each player's hole cards from dealer and spectator views. Rehearse reconnects, clocks, timeout actions, arbitrary legal raises, hand histories and operator controls with consenting players. Freeze the exact policy, memory rules and aids for the match. For one AI versus five pros, preregister lineups and seat rotation, stakes or scoring, rake, stack reset or rebuy, primary win-rate metric, minimum relevant effect, stopping rule, and uncertainty over sessions and lineups. Estimate the required number of sessions from pilot block variance. Keep repeated boards from being recognized by the same human. Show both chip results and any validated variance-reduced estimate. AIVAT requires known action probabilities for any player strategy used in its corrections, plus appropriate value estimates. The lab's all-in adjustment only removes part of the luck. Pluribus played 10,000 hands over 12 days in its five-human condition. A short live event can demonstrate the system and reveal weaknesses, while a strength claim needs its own powered study. [AIVAT](https://arxiv.org/abs/1612.06915)

## Reconstructing the new findings

- Forced-observation curve: group study 11 checkpoint rows by condition and hand, average reads.notes. For the primary value, average post-60 probe loss per seed and arm and subtract fresh_read from frozen_read for the 48 switched histories.
- Own-choice windows: select study 12 main hand rows with module core or replication, arm model and a changed condition. Compute hand minus switch, bin into 1–20, 21–40 and 41–60, average each metric within seed and then average seeds. For the bluff count, select folder_to_caller in the last bin, strong=false and estimate below 50, then count bets and opportunities.
- Grade cross-check: with pot and bet both 10, bet minus check equals 10 × (1 − p) when strong and 20p − 10 when weak. Regret is the positive part of that difference for a check and of its negative for a bet. All 61,440 stored grades agreed to numerical tolerance. Applying frozen-minus-fresh after the switch per seed reproduced both own-choice learning estimates.
- Candidate audit: count distinct group within each export. Count action versus best_action disagreements separately from loss_bb or regret_bb exceeding 1e-8.
- Integrity: checked both overnight executed-source manifests against their private source snapshots and the two large compressed exports against their uncompressed SHA-256 manifests. Replay validates consistency with the implemented engine. Independent rules validation remains worthwhile before a human match.

## Proposed first experiment

The immediate decision is whether tuning improves Inkling's action given the same estimate. On newly generated development cards, fit one small supervised adapter on legal visible inputs and exact action targets. Test its export locally. In parallel with dataset design, register two fresh evaluation distributions: the original river rules with balanced threshold margins, and a transfer set with unfamiliar boards, bet sizes, paired-hole losers and unpaired-hole winners. Keep a third closed-loop block for fresh own-choice histories. Freeze templates, sample sizes and metric definitions before evaluating the adapter.

Use paired base-minus-tuned mean regret per situation group as primary. Predefine a practical minimum saving in bb/decision and use a pilot on development groups to estimate the paired standard deviation. As an initial sample-size approximation for 80% power and two-sided 5% significance, use n ≈ (2.8 × paired SD / minimum saving)² independent groups, then simulate the planned clustered analysis. Equal weights for independent groups prevent families with more prompt variants from dominating. Report every seed, tied choices, cost, illegal responses and the two switch directions. Use a separate fresh replication before advancing the strength claim.

Advance a river-specialist release if the trained model beats the matched base on fresh games, the improvement repeats, and neither the checks on losing hands nor the bets on winning hands show a material regression against prespecified tolerances. Report its distance from the calculator. Advance to a six-seat research release only after a similarly matched evaluation on full hands. If a gain vanishes when changing quantization, board family or renderer, investigate that dependency and describe the narrower result. If supplying action values explains the entire improvement, release the measured hybrid system under that description.
