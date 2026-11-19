# LLM Poker Lab

A controlled lab for measuring how language models play six-handed no-limit Texas hold'em, and above all **how they learn**: whether a model gets better at a table as it sees more hands, what kind of memory or training makes that happen, and what it costs.

## Why

Language models can describe poker strategy fluently; whether that turns into winning play is an open question (see [related papers](docs/papers.md)). Poker is a good test because it needs probabilistic judgment, reading opponents, remembering what matters, and adapting, all under hidden information. The lab's goals:

1. **Understand how models learn** (the central question): within a session (notes, statistics, adapting to an opponent who adjusts), across sessions (persistent profiles), and through training (fine-tuning). Which of these turns experience into better play?
2. **Find what explains a model's results**, not just who wins: computation (facts, odds), opponent information (notes, statistics, accurate profiles), bet sizing, thinking, and the harness around the model.
3. **Measure decisions, not luck**: paired deals, all-in luck removal, and grades of each decision against the best available choice.
4. **Use opponents with known answers**: scripted bots whose exploits are written down, plus a tuned-bot ceiling, so a model's "read" can be checked.
5. **Report cost beside strength**, and keep everything replayable.

Details: [design](docs/design.md).

## What we know so far

85,755 hands, $16.64 ([full results](docs/results.md)):

- DeepSeek V4.1 Flash beats the bot field (+56 bb/100 thinking off) but wins only a fraction of what a simple tuned bot wins (+288).
- The biggest helps, in order of certainty: **thinking** (low effort) on; **host-computed facts** (position, pot odds, hand strength); **accurate opponent profiles**; an explicit **bet-sizing rule** (the sharp rule held up on fresh cards). The model's own notes are accurate but their chip gain is unresolved.
- Helper gains measured on the seeds used for tuning shrank on fresh seeds: claims now need fresh cards.
- GLM-5.3 Flash wins as many chips as DeepSeek (+180 vs +175 bb/100; the difference, +4 [−85, +94], cannot be told from zero) at a fifth of the cost, but makes more graded mistakes on the turn and river. gpt-oss-20b is unusable (empty answers).
- Weaknesses: the opponents are fixed and simple, samples are 10-20 seeds per arm, and only two thinking models have been compared. See the [roadmap](ROADMAP.md).

## Quick start

Rust only. Credentials from `OPENROUTER_API_KEY` or a git-ignored `.openrouter-api-key`.

```sh
cargo test                                                    # engine, protocol, analysis (no API calls)
cargo run -- 6 100 42 /tmp/poker-run                          # scripted six-seat session, no model
cargo run --release -- openrouter examples/1-pilots/pilot.json runs/my-pilot          # one model session
cargo run --release -- batch examples/6-lineup/deepseek-none.json runs/my-batch       # a parallel batch
cargo run --release -- analyze runs/my-batch --turn           # style, win rates, luck removal, grades
cargo run --release -- compare runs/a runs/b --seat 0 --turn  # arms of several runs side by side
cargo run --release -- catalog runs > results/runs.csv        # one row per run
cargo run --release -p poker-table                            # watch a model play (add --mock for free)
```

Full command reference: [methods](docs/methods.md).

**What is a seed?** A number that decides how the deck is shuffled. The same seed deals exactly the same cards in the same order, so two setups (say, with and without notes) can face identical hands and the difference between them is the setup, not the cards. One seed here means one 60-hand session's worth of deals; "seeds 11-20" is ten different decks. A "fresh" seed is one never used while tuning a helper, so a result on it cannot have been fitted to those cards. Seeds are also the unit of the error bars: ten seeds is ten independent tries.

## Repository map

| Path | What |
| --- | --- |
| `src/` | The lab: engine, bots, model agent, worker supervision, batches, analysis, graders |
| `table/` | The spectator table (separate workspace crate) |
| `examples/<topic>/` | Every config that was run, grouped by topic |
| `runs/<topic>/` | Raw run data (git-ignored, about 3.5 GB; private: contains seeds and hidden cards) |
| `results/` | Small, readable outputs kept in git: run catalog, analyses, comparisons |
| `docs/` | [design](docs/design.md), [methods](docs/methods.md), [results](docs/results.md), [model input](docs/model-input.md), [table](docs/table.md), [papers](docs/papers.md) |
| `ROADMAP.md` | Done, open gaps, next experiments |

Topics, in order: `1-pilots`, `2-thinking-off`, `3-qwen`, `4-table`, `5-thinking-low`, `6-lineup`. The same names are used in `examples/`, `runs/`, `results/` and the sections of [results](docs/results.md).
