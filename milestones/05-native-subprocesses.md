# Milestone 5: Native Rust subprocess participants

**Status:** Implemented and live-validated on 2026-11-11. The text below is the original plan, kept as written; what was built, measured, and changed is in [native subprocess supervision](../docs/10-native-subprocess-supervision.md) and the feedback section at the end.

**Decision:** Run one native Rust worker process per seat. Keep Docker/OCI as a later deployment backend using the same protocol. No Docker daemon, container image, local model weights, or virtual machine is required for this milestone. The first model table uses the existing DeepSeek model and API key for all six seats.

## Goal and current foundation

Complete six-player sessions with independently supervised participants while preserving engine authority, player-specific observations, private memory, deterministic action replay, and one table-wide request/spending policy. A participant failure must not hang the arena or cause a delayed response to be accepted on a later turn.

The project already has a deterministic Rust referee, synchronous Agent lifecycle, scripted policies, an OpenRouter adapter, separate player/evaluator logs, optional notes/statistics, and a live-validated six-model in-process table. Prepared version-1 participant types exist in `src/participant.rs`, but no worker executable, process transport, or supervisor exists. The current shared usage ledger uses process-local Rc/RefCell and cannot simply be copied into six workers.

This milestone covers trusted project-owned workers on macOS and Linux. Separate processes provide lifecycle and address-space separation; they do not sandbox hostile code running as the same OS user. Arbitrary third-party executables, Windows support, hard memory/CPU quotas, containers, and local inference deployment are deferred.

## 1. Worker and host architecture

- Add `poker-lab participant` as an internal worker entry point in the existing Rust executable. It reads bounded JSONL requests on stdin, writes only JSONL responses on stdout, and flushes each response. Diagnostics go to stderr. The executable is selected through `current_exe`, not a shell command or user-controlled executable string.
- Initially support the existing caller and fixed tight/loose/aggressive policies; then support OpenRouter with none/notes/stats/both conditions. Keep the original in-process backend available for regression comparison. Random scripted policies are not required for this milestone; no deck or session seed is sent to workers.
- The parent owns the engine, deck, hand/session IDs, scheduling, legal validation, fallback selection, shared budgets, public/evaluator logs, and completed-session status. Workers own only their assigned policy, authorized observations, private notebook/statistics, and provider adapter.
- Add a supervisor module and a SubprocessAgent implementation. Preserve the synchronous engine; use a dedicated supervisor thread with a Tokio runtime for child process I/O, deadlines, and shutdown. The synchronous Agent methods communicate with that supervisor through bounded channels. Enable Tokio process, time, I/O and synchronization features directly rather than relying on transitive dependencies.
- Start all workers and complete initialization before dealing the first hand. Within a session, preserve one worker per seat and stable seat aliases. Deal and request actions in normal engine order; do not parallelize betting decisions. Send hand-end callbacks sequentially by seat so shared budget accounting remains deterministic.
- Each worker gets a separate 0700 working directory and 0600 private trace files. Clear its inherited environment, then explicitly supply only necessary runtime/TLS settings and, for OpenRouter workers, OPENROUTER_API_KEY. Do not place credentials in command-line arguments, protocol records, manifests, or diagnostics. Scripted workers receive no credential. Sharing the key across model workers is intentional; conversations and memory remain separate.

## 2. Protocol and lifecycle

Introduce **participant protocol v2**, because initialization, hand context, usage reporting, and error acknowledgements exceed the prepared v1 contract. Leave engine replay Record.version at 1. V1 is a development prototype, not a deployed compatibility requirement; reject old transport versions with a clear error and update its fixtures.

Every envelope includes protocol version, request ID, session ID, assigned seat, and optional hand number. IDs increase monotonically per worker; exactly one request may be outstanding. Responses must echo the entire correlation identity. No unsolicited stdout messages are permitted.

Messages:

- **Init:** assigned seat, policy kind, relevant model/settings/memory configuration, logging location, and protocol capabilities. Exclude the full table configuration, opponent policies, evaluator data, session/deck seeds, and sibling paths. Response is Ready; duplicate initialization is an error.
- **StartHand:** hand number only. Acknowledges the beginning of a new hand without clearing session memory.
- **Decide:** the assigned seat's authorized PlayerView and a host-issued allowance for at most one provider request. Returns a typed Action or structured error plus usage metadata. Host validates turn, seat, and action independently.
- **HandEnd:** that seat's completed-hand PlayerView and an allowance for at most one notebook request when notes are enabled. Returns an acknowledgement or explicit failure plus usage metadata. Statistics-only updates require no provider call.
- **Shutdown:** acknowledgement followed by clean exit. A new session always starts a fresh process; do not support in-process reset or mid-session restart in this version.

Responses distinguish Action, Ack, Ready, and Error outcomes; do not encode all operations through an ambiguous optional action. Every response carries operation-scoped usage: provider attempt count, prompt/completion tokens when known, reported cost or unknown-cost status, note-update result, and any halt reason. API errors and invalid model actions remain distinct from broken transport/protocol errors.

Refactor the Agent lifecycle to report hand-end and startup failures explicitly. The current finish_hand method returns nothing, so it cannot convey a failed remote notebook update. Preserve existing scripted behavior with default successful callbacks. Engine events and recorded applied actions remain replay-compatible; lifecycle failures belong in session/participant audit records and summaries.

## 3. Supervision, budgets, and failure policy

**Deadlines and framing**

- Initialization: 5 seconds. Provider HTTP deadline: configured timeout_ms, 30 seconds by default. Host Decide/HandEnd deadline: that timeout plus 5 seconds, measured across the entire write/read exchange. StartHand and Shutdown acknowledgement: 2 seconds.
- Cap each complete JSONL frame at 1 MiB, including newline. Reject invalid UTF-8, malformed JSON, extra fields, unmatched IDs, wrong seat/session/hand, unexpected message types, unsolicited lines, and EOF before a complete response. Read incrementally with a length bound; an unbounded read_until is not acceptable.
- Continuously drain stderr while requests run. Retain at most 1 MiB per worker per session; exceeding that cap is a logged protocol/resource failure that terminates the worker. This prevents a full stderr pipe from deadlocking a model call.
- Launch workers in separate Unix process groups. On shutdown, allow 2 seconds for normal exit, then kill the process group and reap the direct child. Bound the kill/reap phase to 2 seconds; if cleanup fails, record that fact and abort the run. Clean up all workers on initialization failure, normal completion, host error, and handled Ctrl-C. Abrupt host SIGKILL is outside the cleanup guarantee.

**Global budget authority**

- Maintain the ledger in the parent. Do not create independent full-run budgets in each child. Before any model action or notebook operation, reserve one request slot and grant permission for exactly one attempt; current adapters make no automatic retries. Scripted decisions and stats-only callbacks reserve nothing.
- Refund a reserved slot only when an acknowledged response proves no provider request was attempted. Successful and failed HTTP attempts both count. After a lost response or worker crash, keep the slot charged and mark cost unknown; never assume that termination cancelled provider billing.
- Reconcile usage and cost before granting another call. Reject reported attempt counts above the grant. Any unknown cost, exhausted request allowance, or reached reported-spend threshold stops all further provider calls across all seats.
- Keep the existing post-response dollar threshold semantics: the last authorized request can exceed the threshold, so it is not a guaranteed hard dollar cap. Host limits and provider-enforced account limits have different roles.
- Workers may retain local defensive limits, but parent authorization is mandatory. Refactor the OpenRouter call path to accept a per-operation grant and return telemetry, replacing reliance on a process-shared ledger for this backend.

**Failure behavior**

- A validly transported malformed/illegal model action gets the existing check-if-legal/otherwise-fold fallback. Log the proposal and reason; retain the worker unless existing consecutive-failure/budget policies halt it.
- A rejected notebook update keeps the prior notebook and is reported as a memory error. A malformed transport response, wrong correlation, crash, EOF, or deadline breach kills and reaps that worker and marks the session contaminated.
- After any fatal participant failure or global budget halt, stop all new provider calls. Finish the current hand with logged fallback decisions where model actions are needed, persist its result, then end the session and shut down all workers. Never silently replace a failed model with a scripted policy or continue to later sessions with unknown costs.
- No mid-session restart: that could silently erase learned notes. A new independent run/session starts fresh workers and empty memory.

## 4. Configuration and observability

Add `execution: "in_process" | "subprocess"` to model-run configuration; default to in_process so existing examples remain unchanged. Add a subprocess example based on the existing six-model config: same model/key, six hands, no memory, 300 shared requests, and $0.50 reported-spend threshold. Backend selection must also work for one-model and four-condition memory runs.

Retain public.jsonl, evaluator.jsonl, per-seat views, readable reports, and summary.json. Record execution backend, participant protocol version, seat-to-worker identity, executable/package version, PID, startup/exit status, deadlines, and worker restarts (zero in this design) in the manifest or lifecycle log. Do not publish credentials or inferred hidden cards.

Add parent-owned participant lifecycle/transport JSONL records with request identity, operation, elapsed time, outcome, exit code/signal, fallback reason, and reconciled usage. Player-bearing wire traces and stderr are private per-seat artifacts. Retain exact bounded provider requests/responses in worker-private model traces; link them through the same seat/session/hand/request IDs. Summaries expose per-seat and aggregate usage, fallbacks, memory failures, unknown costs, and whether the session completed cleanly. Parent logs must survive a worker crash; a missing worker trace must not erase the parent's record of a dispatched operation.

Keep replay fully offline. Add execution/protocol coverage to documentation and CLI help. A report must distinguish the transport error, provider error, and applied fallback rather than collapsing them into a generic player action.

## 5. Implementation order and acceptance tests

1. **Protocol and lifecycle:** implement v2 typed envelopes, explicit lifecycle results, and a synchronous scripted worker. Test round trips, field/version rejection, wrong-seat views, initialization order, StartHand sequencing, and fresh-session reset.
2. **Supervisor:** add bounded async pipes, process groups, deadlines, cleanup, and SubprocessAgent. Use Rust fixture workers to simulate hangs, partial lines, oversized stdout, stderr floods, delayed responses, wrong IDs, EOF, nonzero exit, and ignored shutdown. Verify that no later action accepts a stale response and every reachable child is reaped.
3. **Scripted equivalence:** run fixed seeded tables with identical policies/stacks/button in both backends. Compare applied actions, public events, awards, final stacks, and replay, excluding timing/PIDs. Include heads-up, six seats, folds, all-ins, and side pots. Failure tests assert chip conservation and explicit session contamination.
4. **OpenRouter and budgets:** move one existing adapter into a worker with per-operation grants. Mock the provider in Rust; test table-wide request limits across six workers, unknown costs, notebook costs, a worker crash after dispatch, and usage reconciliation. No test may consume a real API key by default.
5. **Memory and privacy:** test all four memory conditions, authorized summaries, oversized note rejection, no cross-seat notebook/context transfer, and reset through fresh processes. Inspect emitted messages for deck/session seeds, unrevealed cards, sibling notes, and credentials. Do not describe these tests as hostile filesystem isolation.
6. **Live acceptance:** run one-model subprocess play, then the six-seat same-model configuration for six hands within the shared $0.50 threshold. Also run a bounded notes-enabled smoke session. Inspect trace completeness, per-seat usage, known costs, fallbacks, lifecycle errors, and offline replay. Record live spend. Do not automatically rerun failed paid experiments.
7. **Measure and document:** compare in-process versus subprocess scripted runs over 100 ten-hand sessions, recording startup time and median/p95 action round-trip latency without API latency. Record actual results on the current machine; do not invent throughput or memory claims. Finish with cargo test, cargo fmt --check, cargo clippy --all-targets -- -D warnings, and documented reproduction commands.

**Done when:** native scripted and OpenRouter workers complete auditable hands; six independent model workers share one authoritative budget; memory remains seat-private; failure/cleanup tests pass; action replay matches the reference engine; live acceptance and overhead measurements are recorded. This milestone establishes trusted native-process supervision, not container isolation or poker strength.

## Deferred deployment notes

Docker/OCI remains a future backend, not a prerequisite or acceptance criterion for milestone 5. Preserve the same protocol so the launcher can eventually replace the native executable with a restricted container. Later work includes Linux image builds, non-root execution, read-only filesystems, resource limits, credential injection, network restrictions, and genuine isolation tests. Keep VM/microVM and local-inference comparisons separate from hosted-model process supervision. See [deferred container notes](../docs/09-container-preparation.md).

## Claude feedback (2026-11-11)

- **Implemented and accepted on 2026-11-11.** Every item in the "done when" line is met: scripted and model workers complete auditable hands; six model workers share one host ledger; memory stays seat-private; failure and cleanup tests pass; replay matches the in-process engine; live acceptance and overhead are recorded. Results, artifacts and the full list of deviations are in [native subprocess supervision](../docs/10-native-subprocess-supervision.md).
- **The plan was good.** The parts that mattered most in practice were the ones it was most insistent about: kill on any transport fault so a late response cannot be misread, keep the slot charged after a lost response, and no mid-session restart.
- **What I changed and why** (details in docs/10): nested identity and `body` fields so unknown fields are really rejected; workers exit on stdin end-of-file; model seats without a grant are not contacted; no path in `init`; fault fixture as a separate binary; Ctrl-C cleanup as an opt-in process-wide handler.
- **What I added beyond the plan:** a `dispatch` record written before each granted provider call; an explicit `clean` flag in summaries; a `bench` command that doubles as a 1,000-hand equivalence check.
- **What the plan asked for that I did not do:** nothing was skipped, but step 6 said to inspect "known costs" and all costs were known, so the unknown-cost path is covered only by mock tests. A live subprocess `batch` was not run.
- **One thing I would revisit.** The plan keeps per-request dollar accounting post-hoc. That is right for this milestone, but it means the ledger trusts worker reports. The inference proxy suggested in docs/09 closes that.
