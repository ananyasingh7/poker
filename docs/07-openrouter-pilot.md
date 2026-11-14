# OpenRouter baseline pilot

The first milestone 4 slice is implemented entirely in Rust. One configurable seat calls OpenRouter; the original pilot config uses seeded random scripted opponents and no memory. This pilot tests integration reliability, not strategic strength. Optional controlled opponents and memory conditions are described in the [experiment guide](08-experiments-and-observability.md); six-model experiments remain future work.

## Run

1. Credentials are read from `OPENROUTER_API_KEY`, or from `.openrouter-api-key` in the working directory when the environment variable is absent. The local key file is Git-ignored and should have mode 0600. Do not put credentials in the config, source, or logs.
2. `examples/openrouter-pilot.json` is configured for `deepseek/deepseek-v4.1-flash-1017`, with reasoning disabled for the initial low-cost baseline. It uses six seats, model in seat zero, ten hands, seed 42, a 30-second request deadline, 100 requests maximum, 256 completion tokens per request, and a $1 reported-spend threshold. Change `reasoning_enabled` to true for a separately labeled reasoning experiment, or omit it to use the model/provider default.
3. Run with a new, nonexistent output directory:

```sh
cargo run -- openrouter examples/openrouter-pilot.json /tmp/poker-openrouter-run
cargo run -- replay /tmp/poker-openrouter-run/evaluator.jsonl
```

Choose a model that supports the requested parameters. `response_format` defaults to `json_schema`; if your chosen model supports only JSON object mode, explicitly change it to `json_object`. Both modes use the same strict local response validator. The adapter sets `provider.require_parameters=true` to avoid silently ignoring the format or other requested parameters. It does not silently switch models or output modes. See [OpenRouter structured outputs](https://openrouter.ai/docs/guides/features/structured-outputs).

## Decision and failure behavior

The adapter uses nonstreaming `POST https://openrouter.ai/api/v1/chat/completions`. Every decision is a fresh system prompt plus the current authorized `PlayerView`, with card integers translated into readable rank/suit strings in hole cards, board, and public history. Complete current-hand public history is retained. No seed, other private hand, previous conversation, or cross-hand notes are sent. Temperature is zero; this does not guarantee deterministic model responses.

The version-1 model response is `{"action":"call","amount":null}` or `{"action":"raise_to","amount":6}` (also fold/check). Amount is required and must be null for non-raises, an unsigned integer for raises. Extra fields, markdown wrappers, truncated completions, malformed JSON, and illegal actions are rejected. `raise_to` remains total street commitment. The arena applies check if legal, otherwise fold, and records the error and applied fallback in replay. The model trace retains the original response for analysis.

Calls are synchronous with a bounded HTTP deadline, no automatic application retries, and redirects disabled. Transport errors, provider/HTTP errors, malformed or truncated responses, illegal actions, response-size limits, audit failures, and budget stops are distinguished. Authentication/configuration/quota HTTP failures stop future calls. Three consecutive response failures also stop future calls. Missing cost information stops future calls immediately, including after a timeout: an unknown cost is not assumed to be zero.

When a stop condition occurs, the current hand finishes using the check/fold fallback wherever the model still owes an action. The runner then stops before another hand and writes its summary. These fallback-affected hands must not be treated as clean model-performance observations. The request cap is enforced before sending; full serialized request bodies are limited to 65,536 bytes by default, and response bodies to 1 MiB. No silent history truncation is used. Oversized responses are not retained in full.

The reported-spend threshold is checked **after** each response and can be exceeded by the final request. It is not a hard dollar ceiling. Use an OpenRouter key/account spending limit for a provider-enforced cap. Request count, request bytes, and completion-token limits bound requests independently. Model-dependent pricing and input tokenization are not estimated locally. Reported token usage and cost are read from the response; missing cost is counted separately. See [OpenRouter usage accounting](https://openrouter.ai/docs/cookbook/administration/usage-accounting).

## Audit artifacts

Existing public, per-seat private, and evaluator replay files remain compatible. Additional files are:

- `manifest.json`: resolved configuration, rules, seed, opponent policy, crate version, and disabled memory/statistics conditions. Privileged because it includes the seed.
- `model-private.jsonl`: flushed request and response records, joined by hand/decision/seat; exact request bodies, prompt/protocol versions, bounded raw response, HTTP status, elapsed time, generation ID, resolved model/provider when returned, usage, parsed action/error, and stop reason. Secrets and authorization headers are excluded; echoed API key values are redacted. Requests are logged before network calls; an unmatched request indicates interruption or an audit failure. Actions skipped after a halt are recorded in engine replay rather than as network attempts.
- `summary.json`: actual/requested hand counts, network request count, fallback count, reported tokens/cost, unknown-cost count, stop reason, final stacks, rebuys, and net chips after subtracting initial chips and rebuys. A successful CLI exit means the summary was written; inspect stop reason and completed counts to detect early termination.

The directory uses Unix mode 0700 and files 0600. These are private experiment artifacts, not agent-accessible shared logs. Every completed hand and network request/result is flushed. Process-level crash recovery of a partially played hand is not implemented. Provider-hidden reasoning is neither requested nor assumed available; provider response fields that are returned are retained privately.

## Verification and remaining work

`cargo test` uses local Rust TCP mock servers, never paid API calls, to exercise legal responses, wire format and information isolation, malformed/illegal/truncated responses, HTTP errors, deadlines, missing cost, request and spending limits, audit failures, and complete-hand replay. Existing engine reliability tests remain in place. `cargo clippy --all-targets -- -D warnings` checks all targets.

The integration has passed live DeepSeek pilots. Fixed heuristic opponents, seating schedules, notebooks, and statistics are now available. Confidence intervals with sufficient independent sessions and multiple model participants remain subsequent milestone 4 work. The original random opponents are mechanics fixtures, not meaningful poker-strength benchmarks.

## Follow-up status

The original no-memory pilot has now been validated live. Rust hand reports, controlled heuristic opponents, multi-session runs, and four memory conditions are implemented; see [experiments and observability](08-experiments-and-observability.md). The sections above describe the original baseline; the new guide defines the optional treatments and their extra calls. Six-model play and powered evaluation are still pending.

## Claude feedback (2026-11-11)

- **Reproducibility is better than expected.** The subprocess one-model run on 2026-11-11 reproduced the earlier in-process session for seed 42 / seat 0 exactly: 19 requests and identical final chips for all six seats. Temperature 0 with reasoning disabled is close to deterministic for this model. Do not rely on that across providers or dates, and keep recording the resolved model and provider.
- **Strict parsing turned a clear call into a fold.** In `runs/six-subprocess-v1` the model answered `{"action":"call","amount":5}`: it filled in the call amount, which the schema requires to be null. The intent was unambiguous. Options: (a) tighten the prompt ("amount must be null unless raise_to"), (b) accept an amount equal to the exact call amount and log it as normalised, or (c) allow one re-ask. Each is a different condition; whichever you pick, report strict and normalised fallback rates separately. Poker Arena uses a three-step fallback ladder and reports under 2%.
- **The prompt gives the model very little to play with.** No position names, no blind sizes, no pot odds, and an instruction to return only JSON. That is fine as a minimal baseline, but label it as such; a "default prompt" in the PokerSkill paper lost 130–220 mbb/hand to a strong opponent largely through state misreads.
- **Temperature 0 prevents mixing.** See the strategy feedback: supply a private host-generated random number if you want the model to be able to randomise.
- **Prompt caching.** The system prompt is static and the observation grows through the hand. Put stable content first and check whether the provider reports cached tokens; at about 520 prompt tokens per request it is a small saving now but will matter with notes and skills.
- **Three consecutive failures halts the whole table.** Reasonable for a pilot. At scale a single flaky seat will end long sessions; consider a per-session failure budget instead.
