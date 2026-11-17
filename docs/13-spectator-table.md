# Spectator table (`poker-table`)

A retro pixel-art poker table for watching one model play six-handed against the lab's scripted field. Pure Rust, in the workspace crate `table/`. It plays with the lab's own engine, opponents, model agent, decision aids, memory and evaluators; nothing is re-implemented. Built 2026-11-14 and run live against DeepSeek the same day (results below).

![The spectator table during the first live run](images/spectator-table.png)

## Running it

From the repository root (so `.openrouter-api-key` and `runs/` are found):

```sh
cargo run --release -p poker-table                 # choose player and help, then START GAME
cargo run --release -p poker-table -- --mock       # model is a local mock: free, offline, canned reasoning
cargo run --release -p poker-table -- --open runs/memory-structured-v1/session-001   # replay a saved lab session
cargo run --release -p poker-table -- --snapshot /tmp/shots   # write a picture of every screen, no window
```

Keys: `space` pause/resume, `c` settings, `h` show all hole cards, `tab` or `1`-`5` panel tabs, `left`/`right` step through hands, `up`/`down` previous/next hand, `l` or `esc` back to live, `+`/`-` speed.

## What is on screen

- **Table**: the watched seat at the bottom (whale avatar for DeepSeek, robots for everything else), bets, pot, dealer button, last action per seat.
- **MIND**: for the decision on screen, what the model was sent (cards, legal moves, facts sheet, notes), its reasoning, its move, latency, cost and provider. Taken from the agent's audit stream, so it is exactly what was sent and received.
- **HAND**: result, all-in-adjusted result, each decision with equity against the cards actually held, pot-odds grade, EV of calling, and the reference rule's choice. Click a decision to jump to it.
- **SESSION**: the lab's `analyze` row for every seat (win rate, luck-adjusted, VPIP, PFR, AF, mistake cost, rule agreement), spend, totals per help setting, and the lab's measured reference numbers.
- **HANDS**: every finished hand; click to replay.
- **NOTES**: what the model knows about each opponent. With the cheat sheet, the fixed descriptions; with own notes, each opponent's current profile (in a replay, as it stood during that hand), when it last changed, and every change in order; with the tracker, the statistics table it is shown.
- **River grades** (HAND and SESSION): every river decision of the model is graded against the bots (see [docs/15](15-river-grading-and-leaks.md)); HAND shows the better choice and its cost per decision, SESSION the river cost and a LEAKS block (missed river bets, wrong sizes, bets that lost, bluffs called down, answers cut off).

## Settings (before the game and during a pause)

| Setting | Lab meaning |
| --- | --- |
| Facts sheet | `scaffold: features` |
| Cheat sheet | `memory: oracle` |
| Own notes | `memory: notes`, `notebook_style: structured_profiles` |
| Tracker | `memory: stats`: per-opponent VPIP, PFR, postflop aggression and folds to bets (flop to river and river alone), like a HUD |
| Baseline | none of the above |
| Show thinking | `reasoning_enabled: true`, `reasoning_effort: low`, `max_tokens: 12288`, 240 s timeout (limit raised from 4,096 / 90 s and effort added on 2026-11-15, after long reasoning truncated; see below) |
| Reset each | `reset_stacks: true` (the experiments' setting); default is carry-over |
| Cap | `spend_limit_usd` for the whole table session (default $0.25) |

Cheat sheet, own notes and tracker exclude each other, as in the lab's memory conditions. Changes apply from the next hand; the model's own notes are carried across a change. Field, request settings and provider pinning match `examples/experiment-memory-structured.json`.

**Thinking is not the measured setup.** Every lab result was run with reasoning off. With it on, the model may play differently and each decision costs about 19 times more (see the first live run). With it off, MIND shows what the model saw and its move, but no reasoning, because none exists.

## First live run (2026-11-14, `runs/table-1794705445`)

`deepseek/deepseek-v4.1-flash-1017`, baseline (no help), thinking on, carry-over stacks, seed 42, $0.15 cap. An earlier version of this section described the first 13 hands of this session; the session ran on to the cap.

| Measure | Value |
| --- | --- |
| Hands / model decisions | 309 / 575 (572 requests; the last 3 decisions fell back after the cap) |
| Stop | `reported_spend_limit` at $0.1501 |
| Provider | DeepInfra for every request, all first attempts, 0 infra failures |
| Fallbacks | 16: 13 truncations at `max_tokens: 4096`, 3 after the spend cap |
| Response time | median 3.6 s, p90 15.5 s, max 34.7 s (about 119 reasoning tokens/s) |
| Reasoning tokens | median 368, mean 718; by street median 291 / 431 / 477 / 927 (pre/flop/turn/river) |
| Cost | about $0.00026 per decision, $0.00049 per hand (about 19x thinking off; $1 buys about 2,000 hands) |

`poker-lab analyze` on the run:

| | Thinking on (this run) | Thinking off (`experiment-baseline-v1`, 28 seeds) |
| --- | --- | --- |
| bb/100 raw | -33 | +56 [+25, +88] |
| bb/100 all-in adjusted | **+110** | +50 [+27, +73] |
| VPIP / PFR | 33% / 25% | 37% / 18% |
| Postflop bet when checked to | 26% | 2% |
| Postflop decisions facing a bet | 6% (15) | 14% |
| Call/fold mistake cost (bb/100) | 24 | 67 [56, 78] |

Not paired (one seed against 28; carry-over stacks against reset), so these are leads, not results. The paired thinking on/off arm is next-steps item 8b.

### What the session shows (analysis 2026-11-15, no API spend)

- **Raw loss is one cooler.** Hand 307: a 449-chip preflop all-in with 72% equity against `tight`, lost. Without it the session is about +39 bb/100; the all-in adjustment gives +110.
- **The model became the aggressor.** Thinking off checks 98% of unopened postflop spots, even holding the best hand on 98% of rivers; thinking on bets 26% (median 0.67 pot). The bots bet into it rarely, so it faced only 15 postflop bets in 309 hands.
- **Its rare calls are not a leak.** The AF of 66 is a denominator effect (66 bets/raises to 1 call). In 14 of the 15 bets faced it had at most 24% equity against the cards actually held (mostly 0%): the scripted bots bet with strength, and its 12 folds among those were right. The one questionable line was hand 137 (a flop float, then a turn bluff-raise into `tight`, -42). The fifteenth, hand 251, was a truncation (below).
- **Truncation was the costliest fault.** 11 of 79 river decisions (14%) ran out of tokens while still reasoning and none were loops. The fallback folds when facing a bet and checks otherwise: hand 251 folded the winning hand facing a 48-chip river bet, and hands 85, 181, 253, 302 checked winning river hands (in 302 the reasoning had already settled on "bet 40" before running out). This motivated the 12,288-token limit.
- **Missed thin value against a calling station.** 14 of 21 rivers where it held the winning hand and was checked to (or first to act), it checked; 9 of those had `station` in the pot, which never folds a pair or ace-high. (Checks against `overfolder` alone can be right: it folds the river with less than two pair, so a medium winner gets no call.) Its reasoning assumes a competent opponent ("they called the turn, so betting for value is thin"); with no memory or labels it cannot know who calls everything. Roughly 600 chips of pots were checked down against `station` (an upper bound: it includes hand 190, a fair check with a straight on a four-flush board, and the truncated hands 85 and 302).
- **Bluffs into the station.** 18 postflop bets had under 30% equity (turn bluff share 39%); 12 of them were in hands `station` stayed in. Same cause: no cross-hand information. The `oracle` memory or `notes` condition is the natural test of whether thinking plus opponent knowledge fixes both station leaks.

## Cheat sheet + thinking run (2026-11-15, `runs/table-oracle-thinking-v1`)

Same as the first live run (seed 42, carry-over stacks, no scaffold, thinking on) except `memory: oracle` and the raised 12,288-token / 240 s limit; run with `poker-lab openrouter examples/table-oracle-thinking.json`. The lab's session deals the same cards as the table on the same seed (checked: every per-hand seed of hands 1-79 matches), so hands 1-79 pair with the first run.

**It stopped at hand 79 ($0.085 of the $0.15 cap) on `three_consecutive_failures`:** three truncations in a row in hand 79.

| Measure | Value |
| --- | --- |
| Requests | 147, all DeepInfra, 0 infra failures |
| Truncations at 12,288 tokens | 8 (5.4%), costing $0.030, **35% of the spend** |
| Decisions that finished between 4,096 and 12,288 reasoning tokens | 9 (rescued by the raise; at 4,096 this run would have truncated 17 of 147) |
| Cost | $0.00040 per completed decision; $0.0011 per hand including truncations |
| Truncated responses | 101-112 s each; median response 4.5 s |

**The truncations are not loops** (sentences 99-100% unique). With the cheat sheet in the prompt the model counts opponent hand combinations one by one ("A7s(3), K7s(3), Q7s(3)...") and never reaches an answer. A higher limit makes each such failure dearer without ending it, so the remedy is a bound on deliberation (a reasoning budget or effort level, a prompt line asking for a short estimate, or a re-ask with thinking off after a truncation), not a still higher limit. **Fix chosen: `reasoning_effort: low`** (2026-11-15). The three hand-79 prompts that had each run past 12,288 tokens were re-sent unchanged with `"reasoning": {"effort": "low"}` ($0.0011 in total): all three answered, in 433, 562 and 1,804 reasoning tokens (5, 6 and 19 s), served by DeepInfra. The table now asks for low effort whenever thinking is on, keeping 12,288 tokens as a backstop. The lab has the same opt-in `reasoning_effort` (`low`, `medium`, `high`; requires `reasoning_enabled: true`). Caveat: low effort may change how well the model plays, so table sessions from now on are not comparable with the two runs above.

Hands 1-79, same deals (from `analyze --decisions`):

| | No help | Cheat sheet |
| --- | --- | --- |
| Net | -7 chips (-4 bb/100) | +99 chips (+63 bb/100) |
| Postflop bet when checked to | 25% (15 of 61) | 15% (10 of 65) |
| Low-equity bets with `station` staying in | 5 | 1 |
| River winners bet when `station` was in | 1 of 2 | 1 of 2 |
| Truncations | 1 | 8 |

Seventy-nine hands cannot separate +63 from -4. The one suggestive change is fewer bluffs into the station, which is what the cheat sheet tells the model; river value against the station did not change in this sample.

## Design

- `dealer.rs` plays a hand step by step with `engine::Hand`. A test checks its records equal `arena::Session`'s on the same seed, and every hand passes `arena::replay`.
- `frames.rs` turns a hand into table pictures; a test checks each picture against the referee's state on random hands (chips conserved, stacks and bets equal).
- `session.rs` is the game thread: builds the model seat from the settings, paces play, freezes on pause, taps the audit stream, and keeps `analyze::Tally` per seat.
- `analysis.rs` calls `evaluation::grade_seat`, `allin_adjusted` and `scaffold::suggestion`; a test checks its numbers equal the lab's.
- `canvas.rs`, `tableview.rs`, `app.rs`: a 720x405 software framebuffer with bitmap fonts and pixel sprites, shown through `winit` + `softbuffer` at a whole-number scale.
- Each stretch of settings is saved as `runs/table-<time>/session-NNN` in lab format, so `poker-lab analyze` and `poker-lab report` read it.

Lab changes: `analyze::Tally`, `Tally::style`, `analyze::tally` are public; `OpenRouterAgent::take_memory` / `restore_memory` were added.

## Limits

- Six seats, one model seat, the fixed five-bot field. No mixed-model table yet.
- Reasoning arrives when the response does (requests are not streamed), then is typed out.
- A pause takes effect at the next action; a request already sent completes first.
- Raises and bets are not graded (the lab has no model of how opponents respond).
- A session of a few dozen hands says almost nothing about win rate; the SESSION tab shows the 95% range to make that visible.
