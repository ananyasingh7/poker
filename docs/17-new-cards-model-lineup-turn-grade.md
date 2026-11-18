# New cards, a model lineup, a turn grade and a sharper sizing rule (2026-11-17)

Three questions, $1.70 of a $3 budget (OpenRouter usage $46.71 before, $48.41 after):

1. Do the helper "wins", tuned on seeds 1-10, hold on deals never used for tuning (seeds 11-20)?
2. Is any of it specific to DeepSeek? Three cheaper models on the same deals.
3. Free work: an approximate turn grade, a sharper sizing rule, a cross-run comparison command, and the table's out-of-date numbers.

Every run: 60-hand sessions, model in seat 0, the five-bot field (station, overfolder, solid, adaptive, tight), reset stacks, `max_tokens` 4,096 with thinking (256 without), no re-ask. Configs are in `examples/lineup/`, runs in `runs/fresh-*` and `runs/lineup-*`, the full comparison in `runs/lineup-comparison-v1.{md,json}`.

Seeds 11-20 were used once before, in the thinking-off 28-seed baseline (prompt v2); no helper was ever tuned on them.

## 1. The helper wins on new cards

DeepSeek V4.1 Flash, thinking on with low effort, seeds 11-20, paired against no help on the same seeds (95% intervals over seeds; bb/100; costs are chips given up, lower is better):

| vs no help | Chips | River cost | Turn cost (approx) |
| --- | --- | --- | --- |
| Tracker + plain rule, seeds 1-10 (tuning, 8 seeds) | +60 [-96, +215] | **-200 [-305, -95]** | -38 [-75, -1] |
| Tracker + plain rule, seeds 11-20 (new) | +130 [-10, +271] | -82 [-200, +35] | -4 [-28, +21] |
| Tracker + plain rule, both (18 seeds) | **+99 [+5, +193]** | **-135 [-213, -57]** | -19 [-40, +2] |
| Tracker + sharp rule, seeds 11-20 (new) | **+232 [+31, +434]** | **-142 [-265, -19]** | **-34 [-67, -1]** |

**Reading.** The plain rule's river effect on new cards is less than half what the tuning seeds showed (-82 against -200) and its interval crosses zero on those 10 seeds alone. It points the same way, and pooled over 18 seeds it is clear, but the tuning-seed number overstated it: the "we may be fooling ourselves" worry was partly right. The sharp rule, written after looking at seeds 1-10 and tested only on seeds 11-20, is the first helper whose effect is clear on fresh deals in both chips and river cost. Against the plain rule directly it is better but not yet resolved: river cost -60 [-126, +7], chips +102 [-81, +285].

The sharp rule did what it says. River bets with nothing facing, seeds 11-20:

| | River bets | Median size | All-in | All-in was the best bet |
| --- | --- | --- | --- | --- |
| No help | 22 | 0.69 pot | 7 | 15 |
| Tracker + plain rule | 36 | 0.87 pot | 11 | 25 |
| Tracker + sharp rule | 44 | 1.00 pot | 32 | 29 |

It now shoves more often than the grader says is best (32 against 29), and bluffs slightly more (7 river bets under 30% equity, against 5).

## 2. The model lineup

Rule: cheaper per decision than DeepSeek as we run it (DeepInfra fp8, about $0.00015 per request with thinking). Every model was smoke-tested first (10 hands, $0.02 cap):

| Candidate | Outcome |
| --- | --- |
| GLM-5.3 Flash (Wafer/Darkbloom) | Kept: honours low effort (about 57 output tokens), $0.00003/request |
| gpt-oss-20b (Darkbloom/AkashML/DeepInfra) | Kept: honours low effort, $0.00002/request |
| Gemma 4 26B A4B | Ignores the effort setting and a reasoning budget: 700-4,000 tokens, truncations, $0.00028/request. Kept with **thinking off** only |
| Qwen 3.7 Flash, Muse Spark 1.3 | Blocked by the account's privacy settings (zero data retention, no training); not changed |
| Qwen 3.6 35B, Nemotron 3 Super, Seed 2.0 mini, Mistral Small | Ignore low effort; $0.00024-0.00048/request, dearer than DeepSeek |
| Llama 4 Scout | Not tried: cannot think at all, and costs more per token than our DeepSeek route |

So two models ran under DeepSeek's exact conditions (thinking low) and one ran with thinking off, with a fresh DeepSeek thinking-off control on the same seeds.

| Arm | Seeds | Chips | River cost | River decisions /100 hands | Turn cost | VPIP | $/hand | Fallbacks /100 hands |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DeepSeek · think low · none | 20 | +175 [+87, +264] | 362 | 34 | 140 | 36% | 0.00032 | 1.3 |
| DeepSeek · think low · tracker + rule | 18 | +292 [+154, +430] | 224 | 31 | 114 | 32% | 0.00055 | 2.1 |
| GLM-5.3 Flash · think low · none | 20 | +180 [+79, +280] | 434 | 34 | 190 | 40% | 0.00006 | 0.7 |
| GLM-5.3 Flash · think low · tracker + rule | 20 | +334 [+164, +503] | 342 | 35 | 239 | 46% | 0.00015 | 0.7 |
| gpt-oss-20b · think low · none | 18 | -744 [-984, -504] | 1196 | 73 | 475 | 87% | 0.00007 | 14.9 |
| gpt-oss-20b · think low · tracker + rule | 20 | -221 [-456, +14] | 45 | 4 | 67 | 43% | 0.00006 | 11.3 |
| DeepSeek · think off · none | 10 | +15 [-63, +93] | 574 | 36 | 148 | 36% | 0.00011 | 0.0 |
| DeepSeek · think off · tracker + rule | 10 | -8 [-122, +106] | 559 | 32 | 135 | 32% | 0.00023 | 0.0 |
| Gemma 4 26B · think off · none | 10 | -10 [-64, +44] | 659 | 40 | 227 | 47% | 0.00007 | 0.3 |
| Gemma 4 26B · think off · tracker + rule | 5 | -47 [-216, +122] | 914 | 45 | 245 | 56% | 0.00014 | 14.4 |

Help effect within each model (tracker + rule minus none, paired):

| Model | Seeds | Chips | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| DeepSeek, think low | 18 | **+99 [+5, +193]** | **-135 [-213, -57]** | -19 [-40, +2] |
| GLM-5.3 Flash, think low | 20 | **+154 [+22, +286]** | -92 [-190, +6] | **+49 [+7, +90]** |
| gpt-oss-20b, think low | 18 | +537 [+178, +896] | (not comparable, see below) | |
| DeepSeek, think off | 10 | -22 [-122, +78] | -15 [-76, +46] | -13 [-58, +32] |
| Gemma 4 26B, think off | 5 | -63 [-151, +25] | +259 [-211, +730] | +25 [-305, +354] |

**Reading.**

- **GLM-5.3 Flash plays about as well as DeepSeek** without help (chips +4 [-85, +94] paired over 20 seeds) at a fifth of the cost per hand, and the help lifts its chips by a similar amount. The river-mistake cut is smaller and not resolved, and its turn mistakes go *up* with help (+49): it plays more hands with the tracker (VPIP 40% to 46%). So "help wins chips" carries over to a second model; "help fixes river mistakes" is only clearly shown for DeepSeek.
- **gpt-oss-20b is a bad poker player and an unreliable protocol partner.** Without help it plays 87% of hands and loses 744 bb/100; about 4-9% of its answers are empty (it reasons, then returns no content), played as fallbacks. With the tracker it collapses into folding: 4 river decisions per 100 hands against 73. Its tiny river cost there (45) is an artefact of rarely reaching the river, not skill. Its help "effect" is a change of style, not better decisions.
- **Thinking is the biggest lever measured.** DeepSeek with thinking off wins 238 bb/100 less on the same deals ([-434, -43]), and with thinking off the helpers do nothing (river -15 [-76, +46]). The helpers work through the model's reasoning.
- **Gemma 4 with thinking off plays like DeepSeek with thinking off** (-10 against +15, both around break-even). Its tracker arm has only 5 complete seeds (the spend cap stopped it) and 14 fallbacks per 100 hands; no conclusion.

## 3. Free work

### Approximate turn grade (`src/turn.rs`, `analyze --turn`, `compare --turn`)

Every turn decision of a model seat is graded against the bots, like the river grade, with two approximations:

- **The river card is sampled**: every river card the seat cannot rule out, each with 12 opponent holdings drawn from the ranges (so each river card is judged over the same number of deals).
- **The seat's later play is a stand-in**: at each later choice it may check, bet the pot or go all-in (when it can check), or fold or call (when facing a bet), and takes the option that does best over all deals in which it saw the same things (river card and the bots' actions). That is the river grade's benchmark (knows how the bots play, not their cards) limited to three bet sizes and no raises. With 12 deals behind each river card its later choices fit those deals somewhat (an optimistic continuation); every alternative gets the same continuation, so comparisons between alternatives and between conditions on the same deals are fair, absolute values are not.

Checks: the real holdings and river card rebuild the real deck; turn grading leaves every river grade unchanged; values and kinds are consistent. Grading takes about 1 minute per 1,200 hands with turns (sessions graded in parallel). The table grades turns live (HAND marks them `turn ~`, SESSION shows the turn cost and turn leaks).

On the tuning runs (seeds 1-10), every helper also cuts turn cost, in the same order as on the river, but by much less: cheat sheet -58 [-104, -11], notes every 5 hands -35 [-62, -9], tracker + rule -38 [-75, -1], tracker -28 [-52, -4]. The helpers are mostly river fixes.

### Sharp sizing rule (`sharp_sizing`)

Replaces the plain rule's sentence ([docs/16](16-what-the-model-sees.md) 2f): against a player who rarely folds, on the river bet at least the pot and usually all-in, never half the pot; never bluff that player. Label `+sharp`; cannot be combined with `sizing_hint`. Results above. The table's BET SIZING now sends this rule alone (no bet menu, which never showed an effect).

### `poker-lab compare` (reading runs of different models apart)

```sh
poker-lab compare runs/fresh-deepseek-none-v1 runs/thinking-low-none-v1 runs/lineup-glm-none-v1 ... --seat 0 --turn --save runs/lineup-comparison-v1.json
```

- One row per arm, named `model · thinking · condition` (for example `glm-5.3-flash · think low · stats+hint`), with chips, luck-adjusted chips, river and turn cost, river decisions per 100 hands, VPIP, cost per hand, fallbacks per 100 hands, truncations and providers.
- The same treatment run in several directories (different seeds) is merged into one arm.
- Only complete seeds count, so a session cut short by its spend cap never enters a comparison.
- Paired differences on shared seeds: each helper against the same model's no-help arm, and each model against the first directory's model in the same condition.
- `--seat N` keeps only sessions with the model in that seat (older batches rotated seats); `--save` writes the full JSON (per-seed values included) beside the printed table.

### Table

- The settings sheet's HELP ALLOWED column no longer mixes eras: every value is the thinking-on river-mistake change for DeepSeek against no help; help never measured with thinking on says so.
- SESSION's WHAT THE LAB MEASURED shows three labelled blocks: DeepSeek thinking on, the model lineup, and the older thinking-off results.
- Turn grades in HAND and SESSION; BET SIZING is the sharp rule.

## Weaknesses that remain

- 10-20 seeds per arm. The sharp rule has 10 seeds and is not yet separated from the plain rule.
- The bots are fixed and simple; every result is about exploiting them.
- River and turn costs are per 100 hands played: a model that folds more shows smaller costs (gpt-oss). Read them with VPIP and river decisions per 100 hands.
- The turn grade is an estimate with an optimistic stand-in for later play.
- Only DeepSeek and GLM ran with thinking; Gemma had to run with thinking off, so "is it an AI thing" is answered for two thinking models, not for a broad set.
