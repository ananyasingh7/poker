# Native subprocess participants (milestone 5, implemented)

One native Rust worker process per seat, supervised by the referee host. This document describes what is implemented and measured. The original plan and its acceptance list are in [milestone 5](../milestones/05-native-subprocesses.md); deviations from that plan are listed at the end.

This is **trusted-process supervision**: lifecycle and address-space separation for project-owned workers on macOS and Linux. It is not a sandbox against hostile code running as the same OS user, and it says nothing about poker strength.

## Run it

```sh
# scripted tables on both backends, records compared hand by hand, latency reported
cargo run --release -- bench 100 10 runs/bench.json

# model runs: add "execution": "subprocess" to any model config
cargo run --release -- openrouter examples/openrouter-subprocess-pilot.json runs/subprocess-pilot
cargo run --release -- openrouter examples/openrouter-six-subprocess.json   runs/six-subprocess
cargo run --release -- openrouter examples/openrouter-subprocess-notes.json runs/subprocess-notes
cargo run --release -- replay runs/six-subprocess/evaluator.jsonl
cargo run --release -- report runs/six-subprocess 3
```

`execution` defaults to `in_process`, so existing configs behave as before. It also works inside a `batch` config's `pilot` block (same code path; not exercised live yet). Replay and reports are offline and backend-independent.

## Architecture

| Piece | File | Role |
| --- | --- | --- |
| Protocol v2 | `src/participant.rs` | Typed request/response envelopes, policies, usage. |
| Worker | `src/worker.rs`, `poker-lab participant` | Synchronous JSONL loop on stdin/stdout around one policy. |
| Supervisor | `src/supervisor.rs` | Dedicated thread with a current-thread Tokio runtime that owns every child: spawn, bounded framing, deadlines, stderr drain, kill, reap. |
| `SubprocessAgent` | `src/supervisor.rs` | Implements the existing synchronous `Agent` trait; the engine is unchanged. |
| Ledger | `src/supervisor.rs` (`Shared`) | Host-owned request/spend accounting for the whole table. |
| Fixture | `src/bin/fault-worker.rs` | Test-only worker that misbehaves on demand. |
| Benchmark | `src/bench.rs`, `poker-lab bench` | Backend equivalence and overhead. |

The host owns the engine, deck, seeds, scheduling, legality, fallback choice, budget, and the public/evaluator logs. A worker owns its policy, its authorized observations, its notebook/statistics, and (for model seats) its provider adapter. Decisions are requested one at a time in engine order; hand-end callbacks run sequentially by seat.

Each worker is started from `current_exe` (never a shell or a configured path), in its own process group, with a **cleared environment** and its own `workers/seat-N/` directory (0700) as working directory. Model workers additionally receive `OPENROUTER_API_KEY` and any TLS/proxy variables the host has; scripted workers receive nothing. The key never appears in arguments, protocol frames, manifests, or logs.

Seeded `random` opponents stay in-process on a subprocess table: their generator is derived from the session seed, which is never sent to a worker.

## Protocol v2

One JSON object per line, at most 1 MiB per frame in either direction. Exactly one request is outstanding per worker; workers never write unsolicited lines. Every envelope carries an `identity` (`version`, strictly increasing `id`, `session`, `seat`, optional `hand`) and the response must echo it exactly.

```json
{"identity":{"version":2,"id":4,"session":"d1a5-18…","seat":0,"hand":1},
 "message":{"kind":"decide","body":{"view":{…PlayerView…},"allowance":1}}}
{"identity":{"version":2,"id":4,"session":"d1a5-18…","seat":0,"hand":1},
 "outcome":{"kind":"action","body":{"action":{"type":"call"}}},
 "usage":{"attempts":1,"prompt_tokens":512,"completion_tokens":10,"reported_cost_usd":0.00006,
          "unknown_cost_attempts":0,"normalized":0,"note":"not_attempted","halt":null}}
```

| Request | Carries | Response |
| --- | --- | --- |
| `init` | The seat's own policy (`caller`, a scripted style such as `tight` or `station`, or `open_router` with model settings and memory mode). No table config, opponents, seeds, evaluator data, or paths. | `ready` |
| `start_hand` | Hand number in the identity; must be sequential. | `ack` |
| `decide` | That seat's `PlayerView` and an `allowance` of 0 or 1 provider requests. | `action` or `error` |
| `hand_end` | That seat's completed-hand view and an allowance for one notebook request. | `ack` or `error` |
| `shutdown` | — | `ack`, then exit 0 |

`error` has a class: `provider` (API failure, timeout, unparseable model output), `illegal_action`, `budget` (no grant or a worker-local limit), `memory` (notebook update rejected; old notebook kept), or `lifecycle` (host and worker disagree about protocol state; fatal). Unknown fields are rejected at every level. The v1 development envelope does not parse; a worker that receives one prints a diagnostic and exits with status 2. Engine replay `Record.version` is still 1.

A worker exits cleanly on stdin end-of-file, so it cannot outlive its host, including a host killed with SIGKILL.

## Deadlines and failure policy

| Phase | Deadline |
| --- | --- |
| `init` | 5 s |
| `start_hand`, `shutdown` acknowledgement | 2 s |
| `decide`, `hand_end` | provider `timeout_ms` + 5 s for model workers; 5 s for scripted workers |
| Exit after shutdown ack | 2 s, then the process group is killed |
| Reap after kill | 2 s; failure aborts the run after the summary is written |

Deadlines cover the whole write/read exchange. Stderr is drained continuously and capped at 1 MiB per worker per session.

| What happened | Effect |
| --- | --- |
| Model returns malformed or illegal action; provider error or timeout | Check-if-legal-else-fold fallback, logged with the reason. Worker kept. Existing halt rules (three consecutive failures, HTTP configuration/quota errors) still apply table-wide. |
| Notebook update rejected | Old notebook kept, `memory_errors` incremented, reported as a hand-end lifecycle error. Worker kept. |
| Deadline, EOF, crash, partial/oversized/non-UTF-8/malformed frame, wrong identity, unexpected outcome, usage above the grant, unsolicited output, stderr overflow | Worker's process group is killed and reaped immediately; the seat falls back for the rest of the hand; session marked **contaminated**; no further provider calls for any seat; the session ends after the current hand. |
| Budget exhausted or cost unknown | No further grants to any seat; remaining model decisions fall back; session ends after the current hand. Not a transport failure. |

Because a failed worker is killed before the exchange returns, a late response can never be read on a later turn. There is no mid-session restart: that would silently erase notes. A failed model is never replaced by a scripted policy.

On Ctrl-C the CLI kills every live worker group and exits with status 130.

## Budget authority

The host holds the only ledger. Before a model action or notebook update it reserves one request slot and grants exactly one attempt. Scripted decisions and statistics-only hand-ends reserve nothing. After the response it reconciles the reported usage before granting anything else.

- The slot is refunded only when an acknowledged response reports zero attempts.
- Failed HTTP attempts count. A lost response or a crash after dispatch keeps the slot charged and marks the cost unknown, which stops all further calls.
- A worker that reports more attempts than granted is killed.
- A model seat with no grant is not contacted at all; the fallback is recorded by the host.
- The dollar threshold is still checked after each response, so the last authorized request can exceed it. Use a provider-side key limit for a hard cap.

Workers keep the same limits locally as a defensive second check.

## Artifacts

Unchanged: `public.jsonl`, `seat-N.jsonl`, `evaluator.jsonl`, `manifest.json`, `summary.json`, `hand-history-private.txt`. Added for subprocess runs (directories 0700, files 0600):

- `participants.jsonl` — host-owned lifecycle log, flushed per record: `spawn` (pid, executable, worker version, protocol, init time), `dispatch` (written **before** any granted provider operation), `exchange` (identity, operation, allowance, elapsed, outcome, error class, reconciled usage), `failure`, `exit` (code, signal, killed, reaped, stderr size).
- `workers/seat-N/wire.jsonl` — every frame sent to and received from that worker.
- `workers/seat-N/model-private.jsonl` — the worker's exact provider requests and responses (same format as the in-process trace).
- `workers/seat-N/stderr.log` — captured stderr.
- `summary.json` gains `execution`, `clean`, `lifecycle_errors`, `cleanup_error`, and `participants` (per-seat policy, pid, usage, exit, failures, deadlines). `manifest.json` gains `execution`, `participant_protocol`, and deadlines.

`clean` is true only if every requested hand completed with no fallback, memory error, halt, lifecycle error, contamination, or cleanup failure. `batch` uses it.

## Verification

At the time of milestone 5, `cargo test` ran 49 tests (plus the ignored exhaustive census); it now runs 59. The 15 supervision tests in `tests/subprocess.rs` start real processes:

- **Equivalence:** seeded six-seat, uneven-stack, and heads-up sessions produce byte-identical replay records on both backends, including side pots, showdowns, fold-outs, and busts.
- **Faults:** hang, partial line, delayed response, oversized stdout, stderr flood, wrong id/seat/hand, EOF, nonzero exit, garbage, invalid UTF-8, extra field, over-claimed usage, wrong outcome type, unsolicited line, crash at hand end, hung initialization, ignored shutdown, acknowledged-but-no-exit. Each asserts the arena was not held up, the hand completed with conserved chips and replays, the session is contaminated, nothing was accepted from the seat after the fault, and every process was reaped.
- **Budget (mock provider, no real key):** five requests shared across six workers; post-response spend threshold; unknown cost; HTTP 401; crash after dispatch keeps the slot charged with unknown cost and blocks the other model worker.
- **Memory and privacy:** all four conditions through workers; notes visible only to their own seat; a fresh session starts with an empty notebook; an oversized note is rejected without killing the worker; no credential or seed appears in any artifact; files are 0600.
- **Lifecycle:** worker exits on stdin EOF; v1 frames are refused; Ctrl-C leaves no worker behind.

These tests are about supervision and application-level information flow. They are not hostile filesystem or network isolation tests.

## Live acceptance, 2026-11-11

DeepSeek V4.1 Flash, reasoning disabled, same key for all model seats.

| Run | Hands | Requests | Reported cost | Fallbacks | Unknown cost | Clean | Replay |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| `runs/subprocess-pilot-v1` (1 model vs mixed) | 10 | 19 | $0.001275 | 0 | 0 | yes | 10/10 |
| `runs/six-subprocess-v1` (6 model workers) | 6 | 54 | $0.003777 | 1 | 0 | no | 6/6 |
| `runs/subprocess-notes-v1` (6 model workers, notes) | 3 | 35 (18 notebook) | $0.004486 | 0 | 0 | yes | 3/3 |

Total reported spend $0.009538. In every run each dispatch record has a matching worker-trace request and response, per-seat usage sums to the table ledger, all workers exited with status 0 without being killed, stderr was empty, and the key appears in no artifact.

- The one-model run reproduced the in-process baseline session for seed 42 / seat 0 exactly: same 19 requests and the same final net chips for all six seats.
- The single fallback in the six-seat run was a real model protocol error, not a transport fault: seat 3 answered `{"action":"call","amount":5}` (it filled in the call amount, which the strict schema requires to be `null`). It was classified `provider`/`Malformed`, the seat folded, and the run is correctly marked not clean.
- All 18 notebook updates were accepted; each seat's notes stayed in its own worker.

## Overhead benchmark, 2026-11-11

`poker-lab bench 100 10` on an Apple M4 Max (14 cores), macOS 27.2, rustc 1.99.1, release build. 100 sessions × 10 hands, six scripted seats, 18,336 decisions per backend, all 1,000 hand records identical across backends. Result file: `runs/bench-subprocess-v1.json`.

| | In-process | Subprocess |
| --- | ---: | ---: |
| Decision, median | < 1 µs | 29 µs round trip |
| Decision, p95 / p99 / max | 1 / 2 / 13 µs | 44 / 55 / 145 µs |
| 10-hand session play, median | 0.20 ms | 8.6 ms |
| Hands per second (play only) | about 50,000 | about 1,090 |
| Start six workers, median / p95 | — | 10.0 / 11.1 ms |
| Shut down six workers, median / p95 | — | 1.7 / 2.2 ms |

Session play includes the `start_hand` and `hand_end` round trips for all six seats, which is why per-hand cost is larger than decisions alone. No memory-footprint or throughput-under-load claims are made; this is one machine, one run.

## Deviations from the plan

- **Envelope shape.** Identity is a nested `identity` object and message/outcome bodies sit under `body`. Serde cannot enforce `deny_unknown_fields` through flattened structs or internally tagged unit variants, and strict rejection mattered more than a flat layout.
- **No logging location in `init`.** A worker writes its trace into its own working directory, so no path crosses the protocol.
- **Workers exit on stdin EOF** (added). This covers a host that dies abruptly, which the plan left outside the cleanup guarantee.
- **Ungranted model seats are not contacted** (added). The plan implied a round trip; skipping it keeps a halted table fast and the worker's decision counter honest.
- **Scripted workers get a 5 s operation deadline** rather than provider timeout + 5 s; they make no provider call.
- **Ctrl-C handling is opt-in per process** (`install_interrupt_cleanup`), used by the CLI, because a Tokio signal handler replaces the default disposition for the life of the process.
- **Fault fixture is a second binary** (`fault-worker`), so no fault-injection code path exists in `poker-lab`. `default-run = "poker-lab"` keeps `cargo run` unchanged.
- **`libc` is a direct dependency** for one `kill(-pgid, SIGKILL)` call; the standard library cannot signal a process group.
- **Wire traces** are complete frame logs per seat rather than only lifecycle metadata.
- **`summary.json` gained an explicit `clean` flag**, and batch runs use it instead of recomputing.

## Known limits

- Unix only (process groups and signals).
- A worker blocked in a provider call cannot answer until the HTTP timeout; the host deadline bounds this.
- Usage is self-reported by trusted workers. A host-owned inference proxy would make the ledger authoritative before the request rather than after (see [container notes](09-container-preparation.md)).
- Sessions still run one at a time. Independent sessions are now safe to run in parallel, but no scheduler exists yet.
- Batch runs with `execution: "subprocess"` share the tested code path but have not been run live.

## Claude feedback (2026-11-11)

- **What I would do next, in order:** (1) a parallel session scheduler over this supervisor with one shared ledger; (2) per-seat model configuration for heterogeneous tables; (3) a host-owned inference proxy so the budget is enforced before the request; (4) an `analyze` command. Container packaging comes after all of these.
- **The 29 µs round trip is irrelevant next to model latency** (hundreds of milliseconds), so process isolation is effectively free for model tables. It does matter for large scripted-only simulations (about 46 times slower per hand); keep those in-process.
- **Decide what to do about the `call`-with-amount response.** It is the only live failure seen and it is a parsing policy question, not a supervision one. See the [pilot feedback](07-openrouter-pilot.md).
- **Not yet tested:** Linux, a live subprocess batch, provider timeouts against the real API, and resource exhaustion (memory, file descriptors). The fault tests use a fixture, which shows the supervisor's behaviour but not every way a real worker can fail.
