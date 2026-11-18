// ABOUTME: The `poker-lab participant` worker: one seat's policy behind a JSONL stdin/stdout loop.
// ABOUTME: Synchronous and single-request; stdout carries only protocol frames, diagnostics go to stderr.
use crate::{
    arena::{Agent, Scripted},
    engine::PlayerView,
    memory::completed,
    openrouter::{Config, Failure, OpenRouterAgent, Stats},
    opponents::Style,
    participant::{
        ErrorClass, FRAME_LIMIT, Identity, Message, ModelSettings, NoteResult, Outcome, Policy,
        Request, Response, Usage, VERSION,
    },
    private_fs,
};
use std::io::{BufRead, Write};

pub const KEY_VARIABLE: &str = "OPENROUTER_API_KEY";
pub const MODEL_TRACE: &str = "model-private.jsonl";

/// Reads one newline-terminated frame without ever buffering more than `limit` bytes.
/// `Ok(None)` is a clean end of input before any byte of a new frame.
pub fn read_frame(input: &mut impl BufRead, limit: usize) -> Result<Option<String>, String> {
    let mut frame = Vec::new();
    loop {
        let available = input.fill_buf().map_err(|e| format!("read failed: {e}"))?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err("input ended inside a frame".into())
            };
        }
        let newline = available.iter().position(|b| *b == b'\n');
        let take = newline.map_or(available.len(), |i| i + 1);
        if frame.len() + take > limit {
            return Err("frame exceeds size limit".into());
        }
        frame.extend_from_slice(&available[..take]);
        input.consume(take);
        if newline.is_some() {
            frame.pop();
            return String::from_utf8(frame)
                .map(Some)
                .map_err(|_| "frame is not valid UTF-8".into());
        }
    }
}

enum Brain {
    Scripted(Box<dyn Agent>),
    Model(Box<OpenRouterAgent>),
}
struct Active {
    session: String,
    seat: usize,
    hand: usize,
    brain: Brain,
}

fn model(settings: ModelSettings) -> Result<OpenRouterAgent, String> {
    let key = std::env::var(KEY_VARIABLE).map_err(|_| "model worker received no credential")?;
    let trace = private_fs::create_file(MODEL_TRACE)
        .map_err(|e| format!("could not create private model trace: {e}"))?;
    // Table shape, seeds and opponents are not known here; the remaining Config fields keep
    // their defaults and are not used by the adapter.
    let config = Config {
        model: settings.model,
        timeout_ms: settings.timeout_ms,
        max_tokens: settings.max_tokens,
        max_request_bytes: settings.max_request_bytes,
        response_format: settings.response_format,
        reasoning_enabled: settings.reasoning_enabled,
        reasoning_effort: settings.reasoning_effort,
        memory: settings.memory,
        notebook_style: settings.notebook_style,
        normalize_call_amount: settings.normalize_call_amount,
        scaffold: settings.scaffold,
        max_attempts: settings.max_attempts,
        retry_backoff_ms: settings.retry_backoff_ms,
        retry_backoff_max_ms: settings.retry_backoff_max_ms,
        provider: settings.provider,
        max_consecutive_failures: settings.max_consecutive_failures,
        repair_illegal: settings.repair_illegal,
        reask_truncated: settings.reask_truncated,
        notebook_every: settings.notebook_every,
        notes_thinking_off: settings.notes_thinking_off,
        sizing_hint: settings.sizing_hint,
        sharp_sizing: settings.sharp_sizing,
        size_menu: settings.size_menu,
        label_positions: settings.label_positions,
        // The host validated the table; this only satisfies Config's oracle-needs-a-scripted-
        // field check. The worker is never told who its opponents are.
        opponents: crate::openrouter::Opponents::Mixed,
        notebook_chars: settings.notebook_chars,
        max_requests: settings.max_requests,
        spend_limit_usd: settings.spend_limit_usd,
        ..Config::default()
    };
    let mut agent =
        OpenRouterAgent::with_endpoint(config, key, Box::new(trace), settings.endpoint)?;
    if let Some(notes) = settings.oracle_notes {
        agent.set_notebook(notes);
    }
    Ok(agent)
}

/// The in-process agent for a scripted policy; `None` for model policies. Shared with the
/// in-process backend so both backends run exactly the same policy code.
pub fn scripted(policy: &Policy) -> Option<Box<dyn Agent>> {
    Some(match policy {
        Policy::Caller => Box::new(Scripted::Caller),
        Policy::Tight => Style::Tight.agent(),
        Policy::Loose => Style::Loose.agent(),
        Policy::Aggressive => Style::Aggressive.agent(),
        Policy::Station => Style::Station.agent(),
        Policy::Overfolder => Style::Overfolder.agent(),
        Policy::Solid => Style::Solid.agent(),
        Policy::Adaptive => Style::Adaptive.agent(),
        Policy::OpenRouter(_) => return None,
    })
}

fn brain(policy: Policy) -> Result<Brain, String> {
    Ok(match scripted(&policy) {
        Some(agent) => Brain::Scripted(agent),
        None => match policy {
            Policy::OpenRouter(settings) => Brain::Model(Box::new(model(settings)?)),
            _ => unreachable!("every non-model policy is scripted"),
        },
    })
}

fn lifecycle(detail: &str) -> (Outcome, Usage) {
    (
        Outcome::Error {
            class: ErrorClass::Lifecycle,
            detail: detail.into(),
        },
        Usage::default(),
    )
}

/// Usage for one operation: the difference in the adapter's counters around it.
fn usage(before: &Stats, after: &Stats, note: NoteResult) -> Usage {
    Usage {
        attempts: (after.requests - before.requests) as u32,
        prompt_tokens: after.prompt_tokens - before.prompt_tokens,
        completion_tokens: after.completion_tokens - before.completion_tokens,
        reported_cost_usd: after.reported_cost_usd - before.reported_cost_usd,
        unknown_cost_attempts: (after.unknown_cost_requests - before.unknown_cost_requests) as u32,
        infra_failures: (after.infra_failures - before.infra_failures) as u32,
        assumed_cost_usd: after.assumed_cost_usd - before.assumed_cost_usd,
        normalized: (after.normalized_actions - before.normalized_actions) as u32,
        repaired: (after.repaired_actions - before.repaired_actions) as u32,
        note,
        halt: after.halt_reason.clone(),
    }
}

fn decide(brain: &mut Brain, view: &PlayerView, allowance: u32) -> (Outcome, Usage) {
    match brain {
        Brain::Scripted(agent) => match agent.decide(view) {
            Ok(action) => (Outcome::Action { action }, Usage::default()),
            Err(detail) => (
                Outcome::Error {
                    class: ErrorClass::Provider,
                    detail,
                },
                Usage::default(),
            ),
        },
        Brain::Model(agent) => {
            agent.allowance = Some(allowance);
            let before = agent.stats.borrow().clone();
            let result = agent.act(view);
            let used = usage(&before, &agent.stats.borrow(), NoteResult::NotAttempted);
            let outcome = match result {
                Ok(action) => Outcome::Action { action },
                Err(failure) => Outcome::Error {
                    class: match failure {
                        Failure::IllegalAction => ErrorClass::IllegalAction,
                        Failure::Budget | Failure::Audit => ErrorClass::Budget,
                        _ => ErrorClass::Provider,
                    },
                    // Same text the in-process backend records, so replays read alike.
                    detail: format!("openrouter:{failure:?}"),
                },
            };
            (outcome, used)
        }
    }
}

fn hand_end(brain: &mut Brain, view: &PlayerView, allowance: u32) -> (Outcome, Usage) {
    match brain {
        Brain::Scripted(agent) => match agent.finish_hand(view) {
            Ok(()) => (Outcome::Ack, Usage::default()),
            Err(detail) => lifecycle(&detail),
        },
        Brain::Model(agent) => {
            agent.allowance = Some(allowance);
            let before = agent.stats.borrow().clone();
            let revision = agent.notebook_revision();
            let result = agent.finish_hand(view);
            let after = agent.stats.borrow().clone();
            let note = match &result {
                Err(_) => NoteResult::Failed,
                Ok(()) if after.requests == before.requests => NoteResult::NotAttempted,
                Ok(()) if agent.notebook_revision() != revision => NoteResult::Replaced,
                Ok(()) => NoteResult::Kept,
            };
            let outcome = match result {
                Ok(()) => Outcome::Ack,
                Err(detail) => Outcome::Error {
                    class: ErrorClass::Memory,
                    detail,
                },
            };
            (outcome, usage(&before, &after, note))
        }
    }
}

fn handle(
    active: &mut Option<Active>,
    last_id: &mut u64,
    identity: &Identity,
    message: Message,
) -> (Outcome, Usage) {
    if identity.version != VERSION {
        return lifecycle("unsupported participant protocol version; this worker speaks v2");
    }
    if identity.id <= *last_id {
        return lifecycle("request id is not strictly increasing");
    }
    *last_id = identity.id;
    let Some(state) = active else {
        return match message {
            Message::Init { policy } => match brain(policy) {
                Ok(brain) => {
                    *active = Some(Active {
                        session: identity.session.clone(),
                        seat: identity.seat,
                        hand: 0,
                        brain,
                    });
                    (
                        Outcome::Ready {
                            crate_version: env!("CARGO_PKG_VERSION").into(),
                        },
                        Usage::default(),
                    )
                }
                Err(detail) => lifecycle(&detail),
            },
            _ => lifecycle("worker is not initialized"),
        };
    };
    if identity.session != state.session || identity.seat != state.seat {
        return lifecycle("request addressed to another session or seat");
    }
    match message {
        Message::Init { .. } => lifecycle("duplicate initialization"),
        Message::StartHand => {
            if identity.hand != Some(state.hand + 1) {
                return lifecycle("hands must start in sequence");
            }
            state.hand += 1;
            let started = match &mut state.brain {
                Brain::Scripted(agent) => agent.start_hand(state.hand),
                Brain::Model(agent) => agent.start_hand(state.hand),
            };
            match started {
                Ok(()) => (Outcome::Ack, Usage::default()),
                Err(detail) => lifecycle(&detail),
            }
        }
        Message::Decide { view, allowance } => {
            if identity.hand != Some(state.hand) || state.hand == 0 {
                lifecycle("decision outside the current hand")
            } else if view.seat != state.seat || view.legal.is_none() {
                lifecycle("view is not this seat's pending decision")
            } else {
                decide(&mut state.brain, &view, allowance)
            }
        }
        Message::HandEnd { view, allowance } => {
            if identity.hand != Some(state.hand) || state.hand == 0 {
                lifecycle("hand end outside the current hand")
            } else if view.seat != state.seat || !completed(&view) {
                lifecycle("view is not this seat's completed hand")
            } else {
                hand_end(&mut state.brain, &view, allowance)
            }
        }
        Message::Shutdown => (Outcome::Ack, Usage::default()),
    }
}

/// Serves requests until Shutdown or end of input. Returns the process exit code.
/// End of input is a clean exit: a worker must never outlive its host.
pub fn run(mut input: impl BufRead, mut output: impl Write) -> i32 {
    let mut active = None;
    let mut last_id = 0;
    loop {
        let frame = match read_frame(&mut input, FRAME_LIMIT) {
            Ok(Some(frame)) => frame,
            Ok(None) => return 0,
            Err(e) => {
                eprintln!("participant: {e}");
                return 2;
            }
        };
        let request: Request = match serde_json::from_str(&frame) {
            Ok(request) => request,
            Err(e) => {
                eprintln!("participant: malformed or unsupported request frame: {e}");
                return 2;
            }
        };
        let shutdown = matches!(request.message, Message::Shutdown);
        let (outcome, usage) = handle(
            &mut active,
            &mut last_id,
            &request.identity,
            request.message,
        );
        let acknowledged = outcome == Outcome::Ack;
        let response = Response {
            identity: request.identity,
            outcome,
            usage,
        };
        let line = serde_json::to_string(&response).expect("serializable response");
        if writeln!(output, "{line}")
            .and_then(|_| output.flush())
            .is_err()
        {
            return 3;
        }
        if shutdown && acknowledged {
            return 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arena::play,
        engine::{Action, Hand, Rules},
    };

    fn request(id: u64, seat: usize, hand: Option<usize>, message: Message) -> String {
        serde_json::to_string(&Request {
            identity: Identity {
                version: VERSION,
                id,
                session: "session-a".into(),
                seat,
                hand,
            },
            message,
        })
        .unwrap()
    }
    fn serve(frames: &[String]) -> (i32, Vec<Response>) {
        let input = frames.join("\n") + "\n";
        let mut output = Vec::new();
        let code = run(input.as_bytes(), &mut output);
        let responses = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        (code, responses)
    }
    fn view(seat: usize) -> PlayerView {
        Hand::new(Rules::default(), vec![200; 2], 0, 1)
            .unwrap()
            .view(seat)
            .unwrap()
    }
    fn is_lifecycle_error(response: &Response) -> bool {
        matches!(
            response.outcome,
            Outcome::Error {
                class: ErrorClass::Lifecycle,
                ..
            }
        )
    }
    fn init(id: u64) -> String {
        request(
            id,
            0,
            None,
            Message::Init {
                policy: Policy::Caller,
            },
        )
    }
    fn decide_message() -> Message {
        Message::Decide {
            view: view(0),
            allowance: 0,
        }
    }

    #[test]
    fn scripted_session_follows_the_lifecycle() {
        let done = play(
            Rules::default(),
            vec![200; 2],
            0,
            1,
            &mut [Box::new(Scripted::Caller), Box::new(Scripted::Caller)],
        )
        .unwrap();
        let summary = done.private_views[0].last().unwrap().clone();
        let (code, responses) = serve(&[
            init(1),
            request(2, 0, Some(1), Message::StartHand),
            request(3, 0, Some(1), decide_message()),
            request(
                4,
                0,
                Some(1),
                Message::HandEnd {
                    view: summary,
                    allowance: 0,
                },
            ),
            request(5, 0, Some(2), Message::StartHand),
            request(9, 0, None, Message::Shutdown),
        ]);
        assert_eq!(code, 0);
        assert!(matches!(responses[0].outcome, Outcome::Ready { .. }));
        assert_eq!(responses[1].outcome, Outcome::Ack);
        assert_eq!(
            responses[2].outcome,
            Outcome::Action {
                action: Action::Call
            }
        );
        assert_eq!(responses[2].identity.id, 3);
        assert_eq!(responses[2].identity.hand, Some(1));
        assert_eq!(responses[3].outcome, Outcome::Ack);
        assert_eq!(responses[4].outcome, Outcome::Ack);
        assert_eq!(responses[5].outcome, Outcome::Ack);
        assert!(responses.iter().all(|r| r.usage == Usage::default()));
    }
    #[test]
    fn initialization_order_and_duplicates_are_errors() {
        let (_, responses) = serve(&[
            request(1, 0, Some(1), Message::StartHand),
            init(2),
            init(3),
            request(4, 0, Some(1), decide_message()),
        ]);
        assert!(is_lifecycle_error(&responses[0]));
        assert!(matches!(responses[1].outcome, Outcome::Ready { .. }));
        assert!(is_lifecycle_error(&responses[2]));
        // No hand has started, so a decision is out of sequence.
        assert!(is_lifecycle_error(&responses[3]));
    }
    #[test]
    fn wrong_seat_hand_sequence_ids_and_versions_are_rejected() {
        let (_, responses) = serve(&[
            init(1),
            request(2, 0, Some(2), Message::StartHand),
            request(3, 0, Some(1), Message::StartHand),
            request(
                4,
                0,
                Some(1),
                Message::Decide {
                    view: view(1),
                    allowance: 0,
                },
            ),
            request(5, 1, Some(1), decide_message()),
            request(5, 0, Some(1), decide_message()),
            request(6, 0, Some(2), decide_message()),
            request(
                7,
                0,
                Some(1),
                Message::HandEnd {
                    view: view(0),
                    allowance: 0,
                },
            ),
            request(8, 0, Some(1), decide_message()).replace("\"version\":2", "\"version\":1"),
            request(9, 0, Some(1), decide_message()),
        ]);
        assert!(is_lifecycle_error(&responses[1]), "skipped hand number");
        assert_eq!(responses[2].outcome, Outcome::Ack);
        assert!(is_lifecycle_error(&responses[3]), "another seat's view");
        assert!(is_lifecycle_error(&responses[4]), "another seat's request");
        assert!(is_lifecycle_error(&responses[5]), "repeated id");
        assert!(is_lifecycle_error(&responses[6]), "wrong hand");
        assert!(is_lifecycle_error(&responses[7]), "incomplete summary");
        assert!(is_lifecycle_error(&responses[8]), "old version");
        assert!(matches!(responses[9].outcome, Outcome::Action { .. }));
    }
    #[test]
    fn malformed_oversized_and_truncated_input_stops_the_worker() {
        let (code, responses) = serve(&["{\"version\":1,\"id\":1}".into()]);
        assert_eq!((code, responses.len()), (2, 0));
        let mut extra: serde_json::Value = serde_json::from_str(&init(1)).unwrap();
        extra["identity"]["seed"] = 42.into();
        assert_eq!(serve(&[extra.to_string()]).0, 2);
        let huge = "x".repeat(64);
        assert!(read_frame(&mut huge.as_bytes(), 16).is_err());
        assert!(read_frame(&mut "partial".as_bytes(), 64).is_err());
        assert!(read_frame(&mut &[0xff, b'\n'][..], 64).is_err());
        assert_eq!(read_frame(&mut "".as_bytes(), 64), Ok(None));
        assert_eq!(
            read_frame(&mut "ab\ncd\n".as_bytes(), 3),
            Ok(Some("ab".into()))
        );
        // End of input without Shutdown is a clean exit: a worker never outlives its host.
        assert_eq!(serve(&[init(1)]).0, 0);
    }
    #[test]
    fn model_worker_without_credential_fails_initialization() {
        let settings = ModelSettings {
            model: "test/model".into(),
            timeout_ms: 1000,
            max_tokens: 16,
            max_request_bytes: 65_536,
            response_format: crate::openrouter::Format::JsonSchema,
            reasoning_enabled: None,
            reasoning_effort: None,
            memory: crate::memory::Mode::None,
            notebook_style: crate::openrouter::NotebookStyle::Journal,
            normalize_call_amount: false,
            scaffold: crate::scaffold::Scaffold::None,
            max_attempts: 1,
            retry_backoff_ms: 1,
            retry_backoff_max_ms: 1,
            provider: Default::default(),
            max_consecutive_failures: 3,
            repair_illegal: false,
            reask_truncated: false,
            notebook_every: 1,
            notes_thinking_off: false,
            sizing_hint: false,
            sharp_sizing: false,
            size_menu: false,
            label_positions: false,
            oracle_notes: None,
            notebook_chars: 100,
            max_requests: 1,
            spend_limit_usd: 1.0,
            endpoint: None,
        };
        // Unit tests never hold a real key; the variable is absent in the test environment.
        if std::env::var(KEY_VARIABLE).is_ok() {
            return;
        }
        let (_, responses) = serve(&[request(
            1,
            0,
            None,
            Message::Init {
                policy: Policy::OpenRouter(settings),
            },
        )]);
        assert!(is_lifecycle_error(&responses[0]));
    }
}
