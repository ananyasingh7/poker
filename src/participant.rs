// ABOUTME: Participant protocol v2: the typed JSONL contract between the referee host and one seat's worker.
// ABOUTME: Carries only that seat's authorized observations; never seeds, opponent cards, or sibling data.
use crate::{
    engine::{Action, PlayerView},
    memory::Mode,
    openrouter::{Format, NotebookStyle},
    scaffold::Scaffold,
};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 2;
/// Complete JSONL frame limit, newline included, in both directions.
pub const FRAME_LIMIT: usize = 1_048_576;

/// Correlation identity. A response must echo the request's identity exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub version: u32,
    /// Strictly increasing per worker; exactly one request may be outstanding.
    pub id: u64,
    pub session: String,
    pub seat: usize,
    pub hand: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub identity: Identity,
    pub message: Message,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Message {
    Init {
        policy: Policy,
    },
    StartHand,
    /// `allowance` is the host's grant for provider requests during this operation (0 or 1).
    Decide {
        view: PlayerView,
        allowance: u32,
    },
    HandEnd {
        view: PlayerView,
        allowance: u32,
    },
    Shutdown,
}
impl Message {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::StartHand => "start_hand",
            Self::Decide { .. } => "decide",
            Self::HandEnd { .. } => "hand_end",
            Self::Shutdown => "shutdown",
        }
    }
}

/// What one worker is told about itself. Deliberately excludes the table configuration,
/// opponent policies, seeds, evaluator data, and any other seat's paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
// One value per seat, created once: boxing the model settings would only complicate every match.
#[allow(clippy::large_enum_variant)]
pub enum Policy {
    Caller,
    Tight,
    Loose,
    Aggressive,
    Station,
    Overfolder,
    Solid,
    Adaptive,
    OpenRouter(ModelSettings),
}
impl Policy {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::Tight => "tight",
            Self::Loose => "loose",
            Self::Aggressive => "aggressive",
            Self::Station => "station",
            Self::Overfolder => "overfolder",
            Self::Solid => "solid",
            Self::Adaptive => "adaptive",
            Self::OpenRouter(_) => "openrouter",
        }
    }
    pub fn is_model(&self) -> bool {
        matches!(self, Self::OpenRouter(_))
    }
}

fn default_attempts() -> u32 {
    crate::openrouter::Config::default().max_attempts
}
fn default_notebook_every() -> u32 {
    1
}
fn default_backoff() -> u64 {
    crate::openrouter::Config::default().retry_backoff_ms
}
fn default_failure_limit() -> u32 {
    crate::openrouter::Config::default().max_consecutive_failures
}
fn default_backoff_max() -> u64 {
    crate::openrouter::Config::default().retry_backoff_max_ms
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub model: String,
    pub timeout_ms: u64,
    pub max_tokens: u32,
    pub max_request_bytes: usize,
    pub response_format: Format,
    pub reasoning_enabled: Option<bool>,
    /// See `Config::reasoning_effort`.
    #[serde(default)]
    pub reasoning_effort: Option<crate::openrouter::Effort>,
    pub memory: Mode,
    pub notebook_style: NotebookStyle,
    pub normalize_call_amount: bool,
    /// Decision aids for action prompts; computed by the worker from its own view.
    #[serde(default)]
    pub scaffold: Scaffold,
    /// Provider attempts per decision and backoff between them (see `Config::max_attempts`).
    #[serde(default = "default_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff")]
    pub retry_backoff_ms: u64,
    #[serde(default = "default_backoff_max")]
    pub retry_backoff_max_ms: u64,
    #[serde(default)]
    pub provider: crate::openrouter::ProviderPrefs,
    #[serde(default = "default_failure_limit")]
    pub max_consecutive_failures: u32,
    #[serde(default)]
    pub repair_illegal: bool,
    #[serde(default)]
    pub reask_truncated: bool,
    #[serde(default = "default_notebook_every")]
    pub notebook_every: u32,
    #[serde(default)]
    pub notes_thinking_off: bool,
    #[serde(default)]
    pub sizing_hint: bool,
    #[serde(default)]
    pub sharp_sizing: bool,
    #[serde(default)]
    pub size_menu: bool,
    #[serde(default)]
    pub label_positions: bool,
    /// Experimenter-supplied notebook for the oracle condition.
    pub oracle_notes: Option<String>,
    pub notebook_chars: usize,
    /// Worker-local defensive limits. The host ledger remains the authority.
    pub max_requests: u64,
    pub spend_limit_usd: f64,
    /// Test-only provider override; the CLI never sets it.
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub identity: Identity,
    pub outcome: Outcome,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Outcome {
    Ready { crate_version: String },
    Ack,
    Action { action: Action },
    Error { class: ErrorClass, detail: String },
}
impl Outcome {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready { .. } => "ready",
            Self::Ack => "ack",
            Self::Action { .. } => "action",
            Self::Error { .. } => "error",
        }
    }
}

/// Worker-reported errors. `Lifecycle` means the host and worker disagree about protocol
/// state and is fatal; the others are valid transport carrying a failed operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// Provider/API failure, timeout, or unparseable model output.
    Provider,
    /// The model answered with a well-formed but illegal action.
    IllegalAction,
    /// No host grant, or a worker-local limit refused the provider call.
    Budget,
    /// Notebook update rejected or failed; the previous notebook is retained.
    Memory,
    Lifecycle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteResult {
    #[default]
    NotAttempted,
    Kept,
    Replaced,
    Failed,
}

/// Operation-scoped usage, reconciled by the host before it grants another call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub attempts: u32,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reported_cost_usd: f64,
    /// Attempts whose cost the provider did not report (including timeouts).
    pub unknown_cost_attempts: u32,
    /// Attempts lost to infrastructure and retried or abandoned.
    #[serde(default)]
    pub infra_failures: u32,
    /// Conservative charge for attempts that returned nothing but may have been billed.
    #[serde(default)]
    pub assumed_cost_usd: f64,
    /// Calls accepted with a redundant amount (opt-in normalization).
    pub normalized: u32,
    /// Illegal actions corrected by the opt-in re-ask.
    #[serde(default)]
    pub repaired: u32,
    pub note: NoteResult,
    /// Set when the worker's own adapter decided no further provider calls are safe.
    pub halt: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Hand, Rules};
    use serde_json::{Value, json};

    fn identity() -> Identity {
        Identity {
            version: VERSION,
            id: 7,
            session: "s".into(),
            seat: 0,
            hand: Some(3),
        }
    }
    fn view() -> PlayerView {
        Hand::new(Rules::default(), vec![200; 2], 0, 1)
            .unwrap()
            .view(0)
            .unwrap()
    }
    #[test]
    fn requests_and_responses_round_trip() {
        let messages = [
            Message::Init {
                policy: Policy::Tight,
            },
            Message::StartHand,
            Message::Decide {
                view: view(),
                allowance: 1,
            },
            Message::HandEnd {
                view: view(),
                allowance: 0,
            },
            Message::Shutdown,
        ];
        for message in messages {
            let wire = serde_json::to_string(&Request {
                identity: identity(),
                message,
            })
            .unwrap();
            assert!(!wire.contains("seed") && !wire.contains('\n'));
            let back: Request = serde_json::from_str(&wire).unwrap();
            assert_eq!(back.identity, identity());
            assert_eq!(serde_json::to_string(&back).unwrap(), wire);
        }
        let outcomes = [
            Outcome::Ready {
                crate_version: "0".into(),
            },
            Outcome::Ack,
            Outcome::Action {
                action: Action::RaiseTo(6),
            },
            Outcome::Error {
                class: ErrorClass::Provider,
                detail: "openrouter:Timeout".into(),
            },
        ];
        for outcome in outcomes {
            let response = Response {
                identity: identity(),
                outcome,
                usage: Usage::default(),
            };
            let wire = serde_json::to_string(&response).unwrap();
            assert_eq!(serde_json::from_str::<Response>(&wire).unwrap(), response);
        }
    }
    #[test]
    fn unknown_fields_and_kinds_are_rejected() {
        let good = serde_json::to_value(Request {
            identity: identity(),
            message: Message::StartHand,
        })
        .unwrap();
        assert!(serde_json::from_value::<Request>(good.clone()).is_ok());
        let mut extra = good.clone();
        extra["seed"] = json!(42);
        assert!(serde_json::from_value::<Request>(extra).is_err());
        let mut identity = good.clone();
        identity["identity"]["seed"] = json!(42);
        assert!(serde_json::from_value::<Request>(identity).is_err());
        let mut kind = good.clone();
        kind["message"] = json!({"kind":"reset"});
        assert!(serde_json::from_value::<Request>(kind).is_err());
        let mut nested = good.clone();
        nested["message"]["view"] = Value::Null;
        assert!(serde_json::from_value::<Request>(nested).is_err());
        let decide = serde_json::to_value(Request {
            identity: self::identity(),
            message: Message::Decide {
                view: view(),
                allowance: 1,
            },
        })
        .unwrap();
        let mut body = decide.clone();
        body["message"]["body"]["deck"] = json!([1, 2]);
        assert!(serde_json::from_value::<Request>(body).is_err());
        // The v1 development envelope had no session/seat identity and must not parse.
        let v1 = json!({"version":1,"id":1,"message":{"kind":"reset"}});
        assert!(serde_json::from_value::<Request>(v1).is_err());
        let response = json!({"identity":self::identity(),
            "outcome":{"kind":"ack"},"usage":Usage::default()});
        assert!(serde_json::from_value::<Response>(response.clone()).is_ok());
        let mut ambiguous = response.clone();
        ambiguous["outcome"]["action"] = json!({"type":"call"});
        assert!(serde_json::from_value::<Response>(ambiguous).is_err());
        let mut usage = response;
        usage["usage"]["bonus"] = json!(1);
        assert!(serde_json::from_value::<Response>(usage).is_err());
    }
}
