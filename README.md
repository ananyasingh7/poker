# LLM Poker Lab

An exploration of how language models play poker against one another: how they reason under uncertainty, learn opponent tendencies, manage limited memory, bluff, and adapt. The proposed system combines a controlled poker arena with a long-run evaluation framework.

The working idea is to run multiple models at a table governed by a deterministic poker engine. Each model receives its own view of the hand, a small set of opponent statistics, and a limited notebook it can choose to update. Future experiments could add poker guidance or solver-based baselines, then compare performance across opponents and experimental conditions.

Poker is a useful test of decision-making with incomplete information, but performance here alone cannot establish general intelligence. The documentation describes research questions and design proposals, not established results or implementation commitments.

## Documentation

- [Milestone plan](milestones/README.md)

- [Project vision](docs/01-vision.md)
- [Poker strategy and opponent modeling](docs/02-strategy.md)
- [Proposed system architecture](docs/03-architecture.md)
- [Memory and information design](docs/04-memory.md)
- [Evaluation and research roadmap](docs/05-evaluation.md)
- [Implemented rules and protocol](docs/06-rules-and-protocol.md)
- [OpenRouter pilot](docs/07-openrouter-pilot.md)
- [Experiments and observability](docs/08-experiments-and-observability.md)
- [Deferred container notes](docs/09-container-preparation.md)
- [Native subprocess supervision (milestone 5)](docs/10-native-subprocess-supervision.md)
- [Analysis, parallel experiments, mixed models and opponents (milestones 6–8)](docs/11-analysis-experiments-and-opponents.md)
- [Removing luck: all-in adjustment and decision grades](docs/12-luck-removal-and-decision-grades.md)
- [Spectator table](docs/13-spectator-table.md)
- [Paper: arXiv 2606.13815](docs/paper-2606.13815.md)
- [Paper: arXiv 2605.30094](docs/paper-2605.30094.md)
- [Paper: arXiv 2604.04157](docs/paper-2604.04157.md)


## Rust implementation

Milestones 1–3 now have a custom Rust engine, two-to-six-player scripted arena, deterministic replay, and reliability tests. See the [implemented rules and protocol](docs/06-rules-and-protocol.md) for exact behavior and limitations.

```sh
cargo test
cargo run -- 2 10 42
cargo run -- 6 100 42 /tmp/poker-run
cargo run -- replay /tmp/poker-run/evaluator.jsonl
cargo test --release -- --ignored
cargo clippy --all-targets -- -D warnings
```

The optional log directory must not already exist. It contains separate public, per-seat private, and evaluator-only JSONL files. The evaluator record contains seeds and must never be shared with agents. Implementation, CLI, and tests are Rust only. OpenRouter experiments, supervised worker processes and a [spectator table](#spectator-table) are available below; container deployment remains later work.

## OpenRouter pilot

A Rust adapter now supports one model against scripted players. The [pilot config](examples/openrouter-pilot.json) selects DeepSeek V4.1 Flash 1017. Credentials come from `OPENROUTER_API_KEY` or the Git-ignored local `.openrouter-api-key` file. Run from the project directory:

```sh
cargo run -- openrouter examples/openrouter-pilot.json /tmp/poker-openrouter-run
```

See [pilot setup, limits, logs, and verification](docs/07-openrouter-pilot.md). The integration has passed a live DeepSeek pilot; automated tests use local mock servers. Memory conditions and batch runs are now available below; multi-model evaluation remains later work.

## Hand reports and experiments

```sh
cargo run -- report /tmp/poker-openrouter-run 0
cargo run -- batch examples/openrouter-baseline.json runs/baseline-v1
cargo run -- batch examples/openrouter-memory.json runs/memory-v1
```

Reports default to public information; the optional seat includes only that seat's private cards. Create the `runs` parent directory and use a fresh run name. See [opponent policies, batch budgets, and memory conditions](docs/08-experiments-and-observability.md), the [milestone 5 native subprocess plan](milestones/05-native-subprocesses.md), and [deferred Docker/OCI notes](docs/09-container-preparation.md).

For all six seats using the same DeepSeek model and API key, with independent player contexts and one shared budget:

```sh
cargo run -- openrouter examples/openrouter-six-models.json runs/six-models-v1
```

Use a fresh output directory for each run. This mode needs no Docker and loads no model weights locally.

## Experiments at scale (milestones 6–8)

```sh
cargo run --release -- batch examples/experiment-smoke.json runs/smoke   # parallel sessions, one shared budget
cargo run --release -- analyze runs/smoke                               # style, cost, win rates with intervals
cargo run --release -- batch examples/experiment-scaffold.json runs/scaffold   # milestone 9: decision-aid treatments
cargo run --release -- batch CONFIG.json EXISTING_DIR --resume          # continue an interrupted batch
cargo run --release -- ceiling station,overfolder,solid,adaptive,tight  # what a tuned bot wins from a field
```

Batches can run sessions in parallel, seat a different model per seat, use profile-style or oracle notes, and play against four new opponent styles. See [docs/11](docs/11-analysis-experiments-and-opponents.md) for configs, results so far, and the cost of the full-size experiments, which have not been run.

## Supervised worker processes (milestone 5)

Add `"execution": "subprocess"` to a model config to run one supervised native worker process per seat, with deadlines, kill/reap on any transport fault, private per-seat directories, and one host-owned budget for the table:

```sh
cargo run --release -- openrouter examples/openrouter-six-subprocess.json runs/six-subprocess
cargo run --release -- bench 100 10 runs/bench.json
```

`bench` plays identical scripted tables on both backends, checks every hand record matches, and reports startup and round-trip latency. See [native subprocess supervision](docs/10-native-subprocess-supervision.md) for the protocol, failure policy, artifacts, live acceptance results, and measured overhead.

## Spectator table

![The spectator table: DeepSeek playing six-handed with the session statistics panel open](docs/images/spectator-table.png)

`poker-table` is a retro pixel-art table for watching the model play six-handed against the lab's scripted field. It shows what the model was sent, its reasoning and its move for every decision, replays earlier hands, and reports the lab's statistics per hand and for the session. The player and the help it may use (facts sheet, cheat sheet, own notes, or baseline) can be changed before the game and during a pause.

```sh
cargo run --release -p poker-table            # live model; the spend cap is in the settings sheet
cargo run --release -p poker-table -- --mock  # free offline demo with a local mock model
cargo run --release -p poker-table -- --open runs/memory-structured-v1/session-001   # replay a saved lab session
```

Sessions are saved under `runs/table-*` in the lab's format, so `analyze` and `report` read them. See [docs/13](docs/13-spectator-table.md) for the screens, settings, keys, design and the first live run.

## Claude feedback (2026-11-11)

- **The framing is behind the code.** The opening still says the documentation describes proposals, "not established results or implementation commitments". Milestones 1–5 are now implemented and live-validated. Consider splitting this page into "what exists" and "what is proposed" so a reader does not have to work it out from the milestone files.
- **The project's real differentiator is auditability, and the README should say so.** Compared with the three papers summarised in `docs/`, this arena has a deterministic replayable referee, matched seeds, per-seat information boundaries, a host-owned budget ledger, and now process-per-seat supervision with fault tests. None of the papers report that level of control. That is a stronger pitch than "LLMs play poker".
- **The next bottleneck is wall-clock time, not money or infrastructure.** From `runs/baseline-v1` the focal model's per-session results have a standard deviation of roughly 190 bb/100 over 10-hand sessions (about 6 bb per hand; 12 sessions, so a rough figure). Detecting a 5 bb/100 edge at two standard errors needs on the order of 58,000 hands. At the measured $0.000145 per hand that is under $10 with this model, but at roughly one second per hand sequentially it is about 16 hours. A parallel session scheduler plus an analysis command would unlock more than any further isolation work. See [evaluation feedback](docs/05-evaluation.md).
- **Housekeeping.** The documentation list above now includes docs 06–10. `Cargo.lock` and `.gitignore` are untracked; commit both (this is a binary crate, so the lock file belongs in Git).
