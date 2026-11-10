# Poker Arena: strategic reasoning and memory

**Paper:** Pratham Singla, Shivank Garg, and Vihan Singh, [*Poker Arena: Multi-Axis Profiling of Strategic Reasoning and Memory in LLMs*](https://arxiv.org/html/2606.13815v1), arXiv:2606.13815v1, 11 June 2026. This is a research preprint. Section references below refer to that version.

## Why it matters here

The paper is close to the proposed project: seven LLMs play no-limit Texas hold’em, maintain selective notes, and are evaluated on both chips and separate aspects of play. Its strongest contribution for us is a concrete **memory and measurement design**, not proof that any model is an optimal poker player. Its experiments are much smaller than the proposed large-sample benchmark.

## Experiment and architecture

According to [Sections 2 and 4](https://arxiv.org/html/2606.13815v1), the main experiment used 50 seven-player sessions, each lasting 20 hands, for **1,000 total table hands** and 9,115 logged actions. Each player began with 1,000 chips; blinds rose every five hands from 5/10 to 50/100. This is a short, changing-stack tournament setting, so outcomes and strategy cannot simply be interpreted as cash-game bb/100.

- **Engine:** deterministic hand ranking, side-pot accounting, and game rules. The paper says it estimates decision equity using Monte Carlo sampling; Appendix I specifies 1,000 random run-outs, with equity absent from some early logs.
- **Within-hand context:** the agent receives its private cards, public cards, betting history, session memory, and its previous reasoning for the hand. Opponents are anonymized with stable aliases.
- **Session notebook:** each agent owns a text buffer capped at 16,000 characters. After a hand it receives an anonymized summary and chooses whether to update its notebook. This makes selection a model decision rather than storing every hand verbatim.
- **Cross-session memory:** a lifetime store and seeding policy can initialize later notebooks. The main tournament starts each session with empty session memory; cross-session effects are studied separately in the ablation.
- **Interface:** structured action tool calls are parsed first, then free text is parsed, then check/fold is used as a fallback. The paper reports a fallback rate under 2%. Full prompt templates and implementation notes are in [Appendices E and I](https://arxiv.org/html/2606.13815v1).

The notebook mechanism is a useful starting point for our selective-memory idea. It does not, by itself, establish that this memory is human-like, accurate, or necessary for successful opponent reading.

## What the authors measured

The paper adds nine normalized diagnostic axes to tournament results: bet sizing, bluffing/deception, opponent reading, composure, adaptability, prediction accuracy, strategic mixing, factual accuracy, and positional awareness. Five use action-log statistics, two use regex-based checks of reasoning text, and two combine counts with LLM judges. The authors average the axes equally, but also report the individual scores. The bluff judge uses a non-contestant family; the reading panel excludes the contestant’s model family. See [Section 3 and Appendix D](https://arxiv.org/html/2606.13815v1) for definitions.

These are **proxies**, not direct measurements of mental abilities. For example, higher action entropy in broad street/position buckets is not necessarily correct strategic mixing for the actual hand and range. A persuasive verbal read is not necessarily accurate, and the equal-weight mean embeds a choice about which capabilities matter. Solver reference distributions are from similar stack depths, not necessarily the precise tournament situations studied.

## Results and interpretation

In the main experiment, Claude Opus 4.6 led the paper’s chip table at **+15,730 chips**, with 14 first-place finishes. Grok 4 was second at **+3,705** and led the mean diagnostic score (0.6137); Claude ranked fifth by that mean (0.5754). The paper says middle-field chip confidence intervals overlap. These outcomes show that the chosen diagnostics and chips rank this sample differently; they do **not** establish that the diagnostic ranking is a more correct poker ranking, nor that one model is generally best. The paper reports a Spearman correlation of 0.571 with p = 0.180 for seven models, too uncertain to establish a robust relationship.

In the memory ablation, the authors report 10 paired sessions per cell, 600 ablation hands total, and estimated per-session chip differences of +114.6 for GPT, -109.4 for Kimi, and -42.5 for Claude. None of the three reported tests reaches p < 0.05. Thus the data are compatible with model-dependent effects but **do not establish that persistent memory helps or harms each model**. The paper’s explanation that models use or over-weight priors is a hypothesis, not an experimentally established mechanism. See [Section 4.4 and Appendix A](https://arxiv.org/html/2606.13815v1).

There are reporting inconsistencies worth resolving before replicating: the abstract says “50 sessions of 1,000 hands,” whereas the methods and results repeatedly specify 50 × 20 = 1,000 hands total. Section 2.2 labels one ablation condition as loading past memory and another as fresh, while Appendix I.7 describes conditions differently. Use the methods and primary artifacts to reconcile these rather than assuming the labels are interchangeable.

## Limits that affect our project

- **Variance and sample size:** 1,000 hands and the small ablation make precise rankings and memory effects uncertain. The authors acknowledge this. Observations within a table/session are dependent, so uncertainty should account for sessions and deal structure.
- **Field dependence:** seven models play a particular short tournament against one another. A ranking can change with opponents, seating, stack depth, blinds, prompts, and inference configuration.
- **Measurement validity:** judge-based axes may be biased; regex checks may miss or misclassify reasoning. Reasoning text is not a reliable complete account of how a model decided. The GTO references are approximate for these conditions.
- **Model drift:** the authors report versions changing over the 27-day collection window, limiting version-specific comparisons.
- **Memory treatment:** showing that a notebook exists is different from demonstrating that its contents improve decisions. Cross-session identity mapping and what a player was allowed to observe must be defined carefully.

## Recommendations for our implementation

1. **Adopt the separation of state, notebook, and optional persistent profile.** Store a versioned, capped notebook per agent and provide a post-hand update opportunity. Log the before/after notes and update cost; expose only public information and the agent’s own private information.
2. **Make memory an experimental variable.** Compare no cross-hand memory, notes only, coarse statistics only, and notes plus statistics. Keep the same game and prompt conditions otherwise; define whether identities persist across sessions.
3. **Use multiple metrics without collapsing them prematurely.** Report profit and uncertainty alongside mechanical errors, note update rates, and pre-registered behavioral diagnostics. Define denominators and sample sizes. Treat all cognitive labels as hypotheses until validated.
4. **Test the actual read, not just its wording.** Against controlled opponents with known policies, record predictions before outcomes and evaluate calibration and profitable response. Record only cards visible to an agent in its prompt, even if the evaluator has access to hidden ground truth.
5. **Control noise and implementation bias.** Rotate seats, repeat seeds or duplicate deals when valid, log fallbacks by model, and estimate uncertainty at the session or independent-run level. A matched random seed alone does not guarantee identical experienced situations once actions diverge.
6. **Separate benchmark conditions.** The paper uses escalating-blind tournaments. If the goal is bb/100 and comparable deep-stack strategic play, implement a separately specified cash-game mode; do not mix its ranking with tournament chips.
7. **Use the paper’s prompts as references, not proof of a perfect design.** Its notes and prior reasoning can consume tokens and carry stale assumptions; test shorter memory limits and whether retained information actually improves decisions.

These are our design recommendations and interpretations, not claims the paper experimentally verified. A larger sample is valuable, but careful controls and valid measurements are necessary as well.
