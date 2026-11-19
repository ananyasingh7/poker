// ABOUTME: The game thread behind the table: builds the seats from the spectator's settings, plays
// ABOUTME: paced hands, captures what the model saw and thought, keeps statistics and saves lab-format runs.
use crate::{
    analysis::{HandAnalysis, analyse},
    dealer::{Dealer, Outcome, Step},
    frames::{self, Frame, HandStart, Kind},
};
use poker_lab::{
    analyze::{self, Tally},
    arena::{Agent, Record, replay},
    cards::Card,
    engine::{Action, PlayerView, Street},
    memory::{Memory, Mode},
    openrouter::{
        Config, Effort, Format, NotebookStyle, OpenRouterAgent, Opponents, ProviderPrefs, Stats,
    },
    opponents::Style,
    private_fs,
    river::{RiverGrade, RiverGrader},
    scaffold::Scaffold,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

pub const SEATS: usize = 6;
/// The watched model always sits in seat 0; the view rotates it to the bottom of the table.
pub const HERO: usize = 0;
/// The scripted field of the lab's baseline, scaffold and memory experiments.
pub const FIELD: [Style; 5] = [
    Style::Station,
    Style::Overfolder,
    Style::Solid,
    Style::Adaptive,
    Style::Tight,
];
/// The lab's workhorse since 2026-11-18 (docs/results.md §6): as many chips as DeepSeek at about
/// a fifth of the cost.
pub const DEFAULT_MODEL: &str = "z-ai/glm-5.3-flash";
/// Mixing rate of the table's VARIED bots.
pub const VARIED_MIXING: f64 = 0.1;
const STAND_IN_MODEL: &str = "stand-in/solid";
/// Completion limit with thinking on; reasoning tokens count against it. The first live run
/// (2026-11-15, 4,096 tokens) truncated 11 of 79 river decisions mid-reasoning, and each became
/// the fallback check or fold, including a folded winner. Billing is per token used, so a high
/// limit costs only on the long spots that need it.
pub const THINKING_MAX_TOKENS: u32 = 12_288;
/// With an opponent cheat sheet, unbounded reasoning still truncated 5.4% of decisions at
/// 12,288 tokens (counting hand combinations one by one). Effort "low" finished the same
/// prompts in 433-1,804 tokens; the token limit stays as a backstop.
pub const THINKING_EFFORT: Effort = Effort::Low;
/// Long enough for a full-length answer at the slowest decode speed seen (~69 tokens/s).
pub const THINKING_TIMEOUT_MS: u64 = 240_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brain {
    /// The model, through OpenRouter. Costs money.
    Model,
    /// The lab's `solid` rule in the model's seat. Free and offline; it has no thoughts to show.
    StandIn,
}

/// Cross-hand help. The lab measures these as separate conditions, so only one is on at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notes {
    Off,
    /// The model keeps its own per-opponent notes (`memory: notes`, structured profiles).
    Own,
    /// Accurate descriptions of the scripted opponents (`memory: oracle`).
    CheatSheet,
    /// Tracker statistics of every seat, like a poker HUD (`memory: stats`).
    Tracker,
}

/// What the spectator chooses before the game and during a pause.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub brain: Brain,
    pub model: String,
    /// Ask the model to reason before answering and show that reasoning. The lab's experiments
    /// ran with reasoning off, so this departs from the measured setup and costs more.
    pub thinking: bool,
    /// Host-computed facts with every decision (`scaffold: features`).
    pub facts_sheet: bool,
    pub notes: Notes,
    /// The sharp bet-sizing rule with every decision (`sharp_sizing`; docs/model-input.md 2f). It replaced
    /// the plain rule plus bet menu on 2026-11-17: on fresh deals it cut river mistakes where the
    /// plain rule's cut was unresolved, and the menu never showed an effect (docs/results.md §6).
    pub bet_sizing: bool,
    /// With own notes and thinking on, write the notes with thinking off (`notes_thinking_off`):
    /// much cheaper.
    pub quick_notes: bool,
    pub seed: u64,
    pub reset_stacks: bool,
    /// Less predictable bots: each bot's thresholds drawn per game from the seed, and a
    /// `VARIED_MIXING` chance per decision of deviating (`opponent_variation`). Fixed for a game.
    pub varied_bots: bool,
    pub spend_cap_usd: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            brain: Brain::Model,
            model: DEFAULT_MODEL.into(),
            thinking: true,
            facts_sheet: true,
            notes: Notes::Off,
            bet_sizing: false,
            quick_notes: false,
            seed: 42,
            reset_stacks: false,
            varied_bots: true,
            spend_cap_usd: 0.25,
        }
    }
}
impl Settings {
    pub fn is_baseline(&self) -> bool {
        !self.facts_sheet && self.notes == Notes::Off && !self.bet_sizing
    }
    /// The help in force, in the spectator's words.
    pub fn help_label(&self) -> String {
        let mut parts = vec![];
        if self.facts_sheet {
            parts.push("FACTS SHEET");
        }
        match self.notes {
            Notes::Own if self.quick_notes && self.thinking => parts.push("QUICK NOTES"),
            Notes::Own => parts.push("OWN NOTES"),
            Notes::CheatSheet => parts.push("CHEAT SHEET"),
            Notes::Tracker => parts.push("TRACKER"),
            Notes::Off => {}
        }
        if self.bet_sizing {
            parts.push("BET SIZING");
        }
        if parts.is_empty() {
            "BASELINE".into()
        } else {
            parts.join(" + ")
        }
    }
    /// The lab configuration these settings stand for: the scripted field and request settings
    /// of the lab's latest DeepSeek experiments, with the chosen help.
    pub fn lab_config(&self) -> Config {
        Config {
            model: match self.brain {
                Brain::Model => self.model.trim().to_owned(),
                Brain::StandIn => STAND_IN_MODEL.into(),
            },
            seats: SEATS,
            model_seat: HERO,
            hands: 1_000_000,
            seed: self.seed,
            timeout_ms: if self.thinking {
                THINKING_TIMEOUT_MS
            } else {
                30_000
            },
            max_requests: 1_000_000,
            // Reasoning tokens count against the completion limit.
            max_tokens: if self.thinking {
                THINKING_MAX_TOKENS
            } else {
                256
            },
            spend_limit_usd: self.spend_cap_usd,
            response_format: Format::JsonSchema,
            reasoning_enabled: Some(self.thinking),
            reasoning_effort: self.thinking.then_some(THINKING_EFFORT),
            // A rare runaway answer is re-asked quickly with thinking off instead of folding
            // or checking by default (see docs/results.md §4).
            reask_truncated: self.thinking,
            sharp_sizing: self.bet_sizing,
            notes_thinking_off: self.quick_notes && self.thinking && self.notes == Notes::Own,
            memory: match self.notes {
                Notes::Off => Mode::None,
                Notes::Own => Mode::Notes,
                Notes::CheatSheet => Mode::Oracle,
                Notes::Tracker => Mode::Stats,
            },
            notebook_chars: 2000,
            opponents: Opponents::Mixed,
            opponent_field: FIELD.to_vec(),
            opponent_variation: poker_lab::opponents::Variation {
                personalities: self.varied_bots,
                mixing: if self.varied_bots { VARIED_MIXING } else { 0.0 },
                random_field: false,
            },
            reset_stacks: self.reset_stacks,
            notebook_style: NotebookStyle::StructuredProfiles,
            normalize_call_amount: true,
            scaffold: if self.facts_sheet {
                Scaffold::Features
            } else {
                Scaffold::None
            },
            provider: provider_for(&self.model),
            ..Config::default()
        }
    }
    /// Whether the model's seat must be rebuilt to go from `self` to `next`.
    fn seat_differs(&self, next: &Settings) -> bool {
        self.brain != next.brain
            || self.model != next.model
            || self.thinking != next.thinking
            || self.facts_sheet != next.facts_sheet
            || self.notes != next.notes
            || self.spend_cap_usd != next.spend_cap_usd
    }
}

/// Provider preferences the lab measured for a model: preferences, not restrictions, so any
/// other model ID still routes.
fn provider_for(model: &str) -> ProviderPrefs {
    let lower = model.to_lowercase();
    let (order, quantizations): (&[&str], &[&str]) = if lower.contains("deepseek") {
        (&["DeepInfra", "InferenceNet", "Baidu"], &["fp8"])
    } else if lower.contains("glm") {
        (&["Wafer", "Darkbloom"], &[])
    } else {
        (&[], &[])
    };
    ProviderPrefs {
        order: order.iter().map(|s| (*s).to_owned()).collect(),
        quantizations: quantizations.iter().map(|s| (*s).to_owned()).collect(),
        allow_fallbacks: Some(true),
        ..ProviderPrefs::default()
    }
}

/// Short table names from the lab's seat labels: a model ID becomes its family name, a scripted
/// style its own name. Names are made unique because statistics are kept per name.
pub fn display_names(labels: &[String]) -> Vec<String> {
    let mut names: Vec<String> = labels
        .iter()
        .map(|label| {
            let lower = label.to_lowercase();
            if lower == STAND_IN_MODEL {
                "STAND-IN".to_owned()
            } else if lower.starts_with("mock/") {
                "MOCK".to_owned()
            } else if lower.contains("deepseek") {
                "DEEPSEEK".to_owned()
            } else {
                let short = label.rsplit('/').next().unwrap_or(label);
                let family = short.split(['-', ':']).next().unwrap_or(short);
                let name = if family.len() >= 3 { family } else { short };
                name.chars().take(10).collect::<String>().to_uppercase()
            }
        })
        .collect();
    for seat in 0..names.len() {
        if names.iter().filter(|n| **n == names[seat]).count() > 1 {
            names[seat] = format!("{} {seat}", names[seat]);
        }
    }
    names
}

/// One request to the model and what came back, taken from the agent's audit stream.
#[derive(Clone, Debug, Default)]
pub struct Thought {
    pub hand: usize,
    /// The agent's own request counter; pairs a response with its request.
    pub key: u64,
    /// "action", "action_repair", "action_reask", "notebook" or "notebook_compress".
    pub purpose: String,
    /// For action requests: which of the seat's decisions in this hand it answers (0-based).
    pub decision: Option<usize>,
    pub attempts: u32,
    /// The observation the model was sent, as JSON.
    pub observation: Value,
    pub waiting: bool,
    /// The model's answer text (the JSON move, or the notebook edit).
    pub reply: Option<String>,
    pub reasoning: Option<String>,
    /// "ok", or the failure name (`Truncated`, `IllegalAction`, ...).
    pub outcome: String,
    pub latency_ms: u64,
    pub cost_usd: Option<f64>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub provider: Option<String>,
    /// For notebook requests: the notes after the update was applied.
    pub notes_after: Option<String>,
    pub received: Option<Instant>,
}
impl Thought {
    pub fn is_action(&self) -> bool {
        self.purpose.starts_with("action")
    }
    pub fn aids(&self) -> Option<&Value> {
        self.observation.get("decision_aids")
    }
    pub fn notes_shown(&self) -> Option<&str> {
        self.observation.get("private_notebook")?.as_str()
    }
}

/// The hand in progress.
pub struct Live {
    pub number: usize,
    pub start: HandStart,
    pub holes: Vec<[Card; 2]>,
    pub frames: Vec<Frame>,
    /// Set while the model's seat is being asked for a decision.
    pub asking: Option<Instant>,
    /// Set while seats are told the result (a model with own notes is rewriting them).
    pub closing: bool,
    pub help: String,
}

/// A finished hand, kept for replay and analysis.
pub struct HandLog {
    pub number: usize,
    pub record: Record,
    pub frames: Vec<Frame>,
    pub analysis: HandAnalysis,
    /// The watched seat's river decisions graded against the bots (see `poker_lab::river`), and
    /// its turn decisions graded approximately (`poker_lab::turn`); `street` tells them apart.
    pub river: Vec<RiverGrade>,
    /// The help in force when it was played.
    pub help: String,
}

#[derive(Clone, Debug, Default)]
pub struct HelpTotals {
    pub help: String,
    pub hands: usize,
    pub net_bb: f64,
    pub adjusted_bb: f64,
    pub mistake_bb: f64,
    /// Expected big blinds given up by river choices.
    pub river_bb: f64,
    /// The same for turn choices (approximate grade).
    pub turn_bb: f64,
}

/// Session-wide numbers. `rows` are the lab's `analyze` style rows, one per seat.
#[derive(Clone, Debug, Default)]
pub struct SessionStats {
    pub rows: Vec<Value>,
    pub by_help: Vec<HelpTotals>,
    pub lab: Stats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for the spectator to start the game.
    Setup,
    Playing,
    /// Looking at a saved run; nothing is being played.
    Review,
}

pub struct State {
    // Spectator controls.
    pub paused: bool,
    pub quit: bool,
    /// Playback speed multiplier; 0 disables pacing (tests).
    pub speed: f32,
    /// Wanted settings; taken up at the next hand.
    pub settings: Settings,
    // Written by the game thread.
    pub phase: Phase,
    /// The settings the current hand is played under.
    pub applied: Option<Settings>,
    pub notice: Option<String>,
    pub names: Vec<String>,
    pub hero: usize,
    pub live: Option<Live>,
    pub hands: Vec<HandLog>,
    pub thoughts: Vec<Thought>,
    pub notebook: String,
    pub stats: SessionStats,
    pub run_dir: Option<PathBuf>,
    action_requests: (usize, usize),
}
impl State {
    pub fn new(settings: Settings) -> Self {
        Self {
            paused: false,
            quit: false,
            speed: 1.0,
            names: display_names(&settings.lab_config().seat_labels()),
            settings,
            phase: Phase::Setup,
            applied: None,
            notice: None,
            hero: HERO,
            live: None,
            hands: vec![],
            thoughts: vec![],
            notebook: String::new(),
            stats: SessionStats::default(),
            run_dir: None,
            action_requests: (0, 0),
        }
    }
    /// The thoughts behind the seat's `decision`-th decision of `hand` (an action request and,
    /// when the lab's repair re-ask is on, its follow-up).
    pub fn thoughts_for(&self, hand: usize, decision: usize) -> Vec<&Thought> {
        self.thoughts
            .iter()
            .filter(|t| t.hand == hand && t.is_action() && t.decision == Some(decision))
            .collect()
    }
    pub fn notebook_thought(&self, hand: usize) -> Option<&Thought> {
        self.thoughts
            .iter()
            .rev()
            .find(|t| t.hand == hand && t.purpose.starts_with("notebook"))
    }
    /// Reads one line of the model agent's audit log (see `OpenRouterAgent::log`).
    pub fn ingest_audit(&mut self, line: &Value) {
        let hand = line["id"]["hand"].as_u64().unwrap_or(0) as usize;
        let key = line["id"]["decision"].as_u64().unwrap_or(0);
        let purpose = line["purpose"].as_str().unwrap_or_default();
        match line["kind"].as_str() {
            Some("request") => {
                let attempt = line["attempt"].as_u64().unwrap_or(1) as u32;
                if let Some(thought) = self.thought_mut(hand, key) {
                    thought.attempts = attempt;
                    thought.waiting = true;
                    return;
                }
                if self.action_requests.0 != hand {
                    self.action_requests = (hand, 0);
                }
                let decision = match purpose {
                    "action" => {
                        self.action_requests.1 += 1;
                        Some(self.action_requests.1 - 1)
                    }
                    // Follow-ups answer the same decision as the request before them.
                    "action_repair" | "action_reask" => self.action_requests.1.checked_sub(1),
                    _ => None,
                };
                let observation = line["body"]["messages"][1]["content"]
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(Value::Null);
                self.thoughts.push(Thought {
                    hand,
                    key,
                    purpose: purpose.to_owned(),
                    decision,
                    attempts: attempt,
                    observation,
                    waiting: true,
                    ..Thought::default()
                });
            }
            Some("response") => {
                let raw: Value = line["raw_response"]
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(Value::Null);
                let message = &raw["choices"][0]["message"];
                let reasoning = match message["reasoning"].as_str() {
                    Some(text) => Some(text.to_owned()),
                    None => message["reasoning_details"].as_array().map(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p["text"].as_str().or(p["summary"].as_str()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }),
                }
                .filter(|text| !text.trim().is_empty());
                let outcome = match &line["result"] {
                    Value::Object(result) if result.contains_key("Ok") => "ok".to_owned(),
                    result => result["Err"]
                        .as_str()
                        .map_or_else(|| "unknown".to_owned(), str::to_owned),
                };
                let usage = &line["usage"];
                let Some(thought) = self.thought_mut(hand, key) else {
                    return;
                };
                thought.waiting = false;
                thought.reply = message["content"].as_str().map(str::to_owned);
                thought.reasoning = reasoning;
                thought.outcome = outcome;
                thought.latency_ms = line["latency_ms"].as_u64().unwrap_or(0);
                thought.cost_usd = usage["cost"].as_f64();
                thought.prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                thought.completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
                thought.reasoning_tokens = usage["completion_tokens_details"]["reasoning_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                thought.provider = line["provider"].as_str().map(str::to_owned);
                thought.received = Some(Instant::now());
            }
            Some("notebook_update") => {
                let hand = line["hand"].as_u64().unwrap_or(0) as usize;
                let after = line["after"].as_str().unwrap_or_default().to_owned();
                if let Some(thought) = self
                    .thoughts
                    .iter_mut()
                    .rev()
                    .find(|t| t.hand == hand && t.purpose.starts_with("notebook"))
                {
                    thought.notes_after = Some(after.clone());
                }
                self.notebook = after;
            }
            _ => {}
        }
    }
    fn thought_mut(&mut self, hand: usize, key: u64) -> Option<&mut Thought> {
        self.thoughts
            .iter_mut()
            .rev()
            .find(|t| t.hand == hand && t.key == key)
    }
}

pub struct Shared {
    pub state: Mutex<State>,
    pub wake: Condvar,
}
impl Shared {
    pub fn new(settings: Settings) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::new(settings)),
            wake: Condvar::new(),
        })
    }
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Changes spectator controls and wakes the game thread.
    pub fn control(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.lock());
        self.wake.notify_all();
    }
}

/// What the game thread needs besides settings.
#[derive(Clone)]
pub struct Env {
    pub key: Option<String>,
    /// Provider override for tests and the offline mock; `None` is OpenRouter.
    pub endpoint: Option<String>,
    /// Where runs are saved (`runs` in the lab's directory); `None` saves nothing.
    pub runs_root: Option<PathBuf>,
}

/// Feeds the agent's audit lines to the screen while writing them to the run's audit file.
struct AuditTap {
    file: Option<File>,
    shared: Arc<Shared>,
    pending: Vec<u8>,
}
impl Write for AuditTap {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(file) = &mut self.file {
            file.write_all(buf)?;
        }
        self.pending.extend_from_slice(buf);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            if let Ok(value) = serde_json::from_slice::<Value>(&line) {
                self.shared.lock().ingest_audit(&value);
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.file {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// The model seat, shared so its memory can be carried into a rebuilt agent.
struct ModelSeat(Rc<RefCell<OpenRouterAgent>>);
impl Agent for ModelSeat {
    fn decide(&mut self, view: &PlayerView) -> Result<Action, String> {
        self.0.borrow_mut().decide(view)
    }
    fn start_hand(&mut self, number: usize) -> Result<(), String> {
        self.0.borrow_mut().start_hand(number)
    }
    fn finish_hand(&mut self, view: &PlayerView) -> Result<(), String> {
        self.0.borrow_mut().finish_hand(view)
    }
    fn stop_reason(&self) -> Option<&str> {
        // A halted model plays the rest of its hand as fallbacks, as in the lab; the game
        // thread reads the halt from the shared ledger after the hand and stops the table.
        None
    }
}

/// One stretch of hands under the same settings, saved as a lab session directory.
struct Segment {
    evaluator: File,
    public: File,
    summary: File,
    hands: usize,
    net: Vec<i64>,
}
impl Segment {
    fn create(dir: &Path, config: &Config) -> Result<Self, String> {
        let at = |e: std::io::Error| format!("{}: {e}", dir.display());
        private_fs::create_dir(dir).map_err(at)?;
        let mut manifest = private_fs::create_file(dir.join("manifest.json")).map_err(at)?;
        serde_json::to_writer_pretty(
            &mut manifest,
            &json!({"version":1,"source":"poker-table","crate_version":env!("CARGO_PKG_VERSION"),
                "config":config,"memory":config.memory,"scaffold":config.scaffold,
                "condition":config.condition(),"llm_seats":config.llm_seats(),
                "seat_labels":config.seat_labels(),"opponents":config.opponents,"opponent_version":2,
                "rules":{"small_blind":1,"big_blind":2},
                "rebuy":if config.reset_stacks {"reset_every_hand_100bb"} else {"busted_only_100bb"}}),
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            evaluator: private_fs::create_file(dir.join("evaluator.jsonl")).map_err(at)?,
            public: private_fs::create_file(dir.join("public.jsonl")).map_err(at)?,
            summary: private_fs::create_file(dir.join("summary.json")).map_err(at)?,
            hands: 0,
            net: vec![0; config.seats],
        })
    }
    fn record(&mut self, record: &Record, stats: &Stats, labels: &[String]) -> Result<(), String> {
        let io = |e: std::io::Error| e.to_string();
        let line = |file: &mut File, value: &Value| -> Result<(), String> {
            writeln!(file, "{value}")
                .and_then(|_| file.flush())
                .map_err(io)
        };
        line(
            &mut self.evaluator,
            &serde_json::to_value(record).map_err(|e| e.to_string())?,
        )?;
        line(
            &mut self.public,
            &serde_json::to_value(&record.events).map_err(|e| e.to_string())?,
        )?;
        self.hands += 1;
        for (seat, total) in self.net.iter_mut().enumerate() {
            *total += record.final_stacks[seat] as i64 - record.initial_stacks[seat] as i64;
        }
        let clean = stats.fallbacks == 0 && stats.memory_errors == 0 && stats.halt_reason.is_none();
        let summary = json!({"completed_hands":self.hands,"requested_hands":self.hands,
            "llm_seats":[HERO],"seat_labels":labels,"stats":stats,"final_stacks":record.final_stacks,
            "net_chips":self.net,"clean":clean,"source":"poker-table",
            "note":"stats are cumulative for the whole table session, not this segment alone"});
        self.summary.set_len(0).map_err(io)?;
        self.summary.seek(SeekFrom::Start(0)).map_err(io)?;
        serde_json::to_writer_pretty(&mut self.summary, &summary).map_err(|e| e.to_string())?;
        self.summary.flush().map_err(io)
    }
}

/// Viewing time for one picture at normal speed. A model's own action is held longer when its
/// reasoning is on screen to be read.
fn hold(kind: Kind, model_reasoned: bool) -> Duration {
    Duration::from_millis(match kind {
        Kind::Deal => 1100,
        Kind::Action if model_reasoned => 2600,
        Kind::Action => 750,
        Kind::Board => 1000,
        Kind::Showdown => 2200,
        Kind::Payout => 2600,
    })
}

/// Waits `duration` of table time: scaled by the speed setting and frozen while paused.
/// Returns false when the spectator has quit.
fn pace(shared: &Shared, duration: Duration) -> bool {
    let mut remaining = duration.as_secs_f32();
    let mut state = shared.lock();
    loop {
        if state.quit {
            return false;
        }
        if state.paused {
            state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            continue;
        }
        if state.speed <= 0.0 || remaining <= 0.0 {
            return true;
        }
        let slice = Duration::from_millis(25);
        let started = Instant::now();
        let (guard, _) = shared
            .wake
            .wait_timeout(state, slice)
            .unwrap_or_else(|e| e.into_inner());
        state = guard;
        if !state.paused {
            remaining -= started.elapsed().as_secs_f32() * state.speed;
        }
    }
}

struct Game {
    shared: Arc<Shared>,
    env: Env,
    dealer: Dealer,
    agents: Vec<Box<dyn Agent>>,
    model: Option<Rc<RefCell<OpenRouterAgent>>>,
    stats: Rc<RefCell<Stats>>,
    own_notes: Memory,
    applied: Option<Settings>,
    labels: Vec<String>,
    names: Vec<String>,
    tallies: BTreeMap<String, Tally>,
    by_help: Vec<HelpTotals>,
    /// Grades the watched seat's river decisions as hands finish; dropped after an error.
    river: Option<RiverGrader>,
    run_dir: Option<PathBuf>,
    segment: Option<Segment>,
    segments: usize,
    needs_rebuild: bool,
}
impl Game {
    fn new(shared: Arc<Shared>, env: Env, settings: &Settings) -> Result<Self, String> {
        // The field as the lab builds it (with this game's personalities when bots are varied);
        // seat 0 is a placeholder for the watched seat, replaced below.
        let mut agents: Vec<Box<dyn Agent>> = vec![Style::Solid.agent()];
        agents.extend(
            settings.lab_config().seat_bots()[1..]
                .iter()
                .map(|bot| bot.expect("scripted seat").agent()),
        );
        let mut dealer = Dealer::new(SEATS, settings.seed)?;
        dealer.reset_stacks = settings.reset_stacks;
        let run_dir = match &env.runs_root {
            Some(root) => {
                std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let dir = (0..)
                    .map(|n| {
                        root.join(if n == 0 {
                            format!("table-{stamp}")
                        } else {
                            format!("table-{stamp}-{n}")
                        })
                    })
                    .find(|dir| !dir.exists())
                    .expect("unbounded range");
                private_fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                Some(dir)
            }
            None => None,
        };
        shared.lock().run_dir = run_dir.clone();
        Ok(Self {
            shared,
            env,
            dealer,
            agents,
            model: None,
            stats: Rc::new(RefCell::new(Stats::default())),
            own_notes: Memory::default(),
            applied: None,
            labels: vec![],
            names: vec![],
            tallies: BTreeMap::new(),
            by_help: vec![],
            river: RiverGrader::for_bots(&settings.lab_config().seat_bots(), &[HERO], true).ok(),
            run_dir,
            segment: None,
            segments: 0,
            needs_rebuild: true,
        })
    }

    /// Takes up new settings between hands: rebuilds the model's seat when its treatment changed
    /// (carrying its own notes over) and starts a new saved segment.
    fn apply(&mut self, settings: &Settings) -> Result<(), String> {
        let config = settings.lab_config();
        config.validate()?;
        if let Some(model) = self.model.take() {
            // Only notes the model wrote itself are carried; a cheat sheet is not its memory.
            if self.applied.as_ref().is_some_and(|s| s.notes == Notes::Own) {
                self.own_notes = model.borrow_mut().take_memory();
            }
        }
        let segment_dir = self.run_dir.as_ref().map(|dir| {
            self.segments += 1;
            dir.join(format!("session-{:03}", self.segments))
        });
        self.segment = match &segment_dir {
            Some(dir) => Some(Segment::create(dir, &config)?),
            None => None,
        };
        self.stats.borrow_mut().halt_reason = None;
        self.agents[HERO] = match settings.brain {
            Brain::StandIn => Style::Solid.agent(),
            Brain::Model => {
                let key = self.env.key.clone().ok_or(
                    "no OpenRouter key: set OPENROUTER_API_KEY or create .openrouter-api-key",
                )?;
                let file = match &segment_dir {
                    Some(dir) => Some(
                        private_fs::create_file(dir.join("model-private.jsonl"))
                            .map_err(|e| format!("{}: {e}", dir.display()))?,
                    ),
                    None => None,
                };
                let tap = AuditTap {
                    file,
                    shared: self.shared.clone(),
                    pending: vec![],
                };
                let mut agent = OpenRouterAgent::with_endpoint(
                    config.clone(),
                    key,
                    Box::new(tap),
                    self.env.endpoint.clone(),
                )?;
                agent.stats = self.stats.clone();
                match settings.notes {
                    Notes::CheatSheet => agent.set_notebook(config.oracle_notes()),
                    Notes::Own => agent.restore_memory(std::mem::take(&mut self.own_notes)),
                    Notes::Off | Notes::Tracker => {}
                }
                let agent = Rc::new(RefCell::new(agent));
                self.model = Some(agent.clone());
                Box::new(ModelSeat(agent))
            }
        };
        self.dealer.reset_stacks = settings.reset_stacks;
        self.labels = config.seat_labels();
        self.names = display_names(&self.labels);
        self.applied = Some(settings.clone());
        self.needs_rebuild = false;
        let mut state = self.shared.lock();
        state.names = self.names.clone();
        state.applied = Some(settings.clone());
        state.notebook = match (&self.model, settings.notes) {
            (Some(_), Notes::CheatSheet) => config.oracle_notes(),
            _ => String::new(),
        };
        Ok(())
    }

    fn spent(&self) -> f64 {
        let stats = self.stats.borrow();
        stats.reported_cost_usd + stats.assumed_cost_usd
    }

    /// Stops the table with a message; the spectator resumes after reading it.
    fn stop(&mut self, message: String) {
        self.needs_rebuild = true;
        let mut state = self.shared.lock();
        state.paused = true;
        state.notice = Some(message);
        if let Some(live) = &mut state.live {
            live.asking = None;
            live.closing = false;
        }
    }

    fn play_hand(&mut self) -> Result<(), String> {
        let settings = self.applied.clone().expect("settings applied before play");
        let number = self.dealer.hands_played() + 1;
        let shared = self.shared.clone();
        let names = self.names.clone();
        let is_model = settings.brain == Brain::Model;
        let reasons = settings.thinking;
        let help = settings.help_label();
        let mut start: Option<HandStart> = None;
        let mut shown = 0;
        let outcome = self
            .dealer
            .next_hand(&mut self.agents, &mut |step| match step {
                Step::Dealt(dealt) => {
                    let frames = frames::build(dealt, &[], &names).unwrap_or_default();
                    let first = frames.first().cloned();
                    shown = frames.len();
                    shared.lock().live = Some(Live {
                        number,
                        start: dealt.clone(),
                        holes: dealt.holes().unwrap_or_default(),
                        frames,
                        asking: None,
                        closing: false,
                        help: help.clone(),
                    });
                    start = Some(dealt.clone());
                    if let Some(frame) = first {
                        pace(&shared, hold(frame.kind, false));
                    }
                }
                Step::Turn { seat } => {
                    // A pause holds the table before the next decision is asked for.
                    pace(&shared, Duration::ZERO);
                    if let Some(live) = &mut shared.lock().live {
                        live.asking = (seat == HERO && is_model).then(Instant::now);
                    }
                }
                Step::Acted(decisions) => {
                    let Some(start) = &start else { return };
                    let all = frames::build(start, decisions, &names).unwrap_or_default();
                    let by_model = is_model && decisions.last().is_some_and(|d| d.seat == HERO);
                    for frame in all.into_iter().skip(shown) {
                        shown += 1;
                        let wait = hold(
                            frame.kind,
                            by_model && reasons && frame.kind == Kind::Action,
                        );
                        if let Some(live) = &mut shared.lock().live {
                            live.asking = None;
                            live.frames.push(frame);
                        }
                        pace(&shared, wait);
                    }
                }
                Step::Closing => {
                    if let Some(live) = &mut shared.lock().live {
                        live.closing = true;
                    }
                }
            })?;
        match outcome {
            Outcome::Played {
                record,
                lifecycle_errors,
            } => {
                replay(&record)
                    .map_err(|e| format!("hand {number} failed its replay check: {e}"))?;
                let analysis = analyse(&record, HERO)?;
                let models: BTreeSet<String> = [self.names[HERO].clone()].into();
                analyze::tally(&record, &self.names, &models, true, &mut self.tallies)?;
                let river = match self.river.as_mut().map(|g| g.grade_hand(&record)) {
                    Some(Ok(grades)) => grades,
                    Some(Err(error)) => {
                        // Grading is a lens, not the game: report it and stop grading.
                        self.river = None;
                        self.shared.lock().notice = Some(format!(
                            "River and turn grading stopped at hand {number}: {error}"
                        ));
                        vec![]
                    }
                    None => vec![],
                };
                self.tallies
                    .entry(self.names[HERO].clone())
                    .or_default()
                    .add_river(&river);
                match self.by_help.last_mut() {
                    Some(last) if last.help == help => {}
                    _ => self.by_help.push(HelpTotals {
                        help: help.clone(),
                        ..HelpTotals::default()
                    }),
                }
                let totals = self.by_help.last_mut().expect("just ensured");
                totals.hands += 1;
                totals.net_bb += analysis.net_bb();
                totals.adjusted_bb += analysis.adjusted_bb();
                totals.mistake_bb += analysis.mistake_cost / analysis.big_blind as f64;
                totals.river_bb += street_bb(&river, Street::River, analysis.big_blind);
                totals.turn_bb += street_bb(&river, Street::Turn, analysis.big_blind);
                let stats = self.stats.borrow().clone();
                if let Some(segment) = &mut self.segment {
                    segment.record(&record, &stats, &self.labels)?;
                }
                let rows = self
                    .names
                    .iter()
                    .enumerate()
                    .map(|(seat, name)| {
                        self.tallies.get(name).cloned().unwrap_or_default().style(
                            name,
                            seat == HERO,
                            record.rules.big_blind,
                        )
                    })
                    .collect();
                let frames = frames::of_record(&record, &self.names)?;
                let halted = stats.halt_reason.clone();
                {
                    let mut state = self.shared.lock();
                    state.stats = SessionStats {
                        rows,
                        by_help: self.by_help.clone(),
                        lab: stats,
                    };
                    if let Some(live) = &mut state.live {
                        live.closing = false;
                    }
                    state.hands.push(HandLog {
                        number,
                        record,
                        frames,
                        analysis,
                        river,
                        help,
                    });
                    if let Some(error) = lifecycle_errors.first() {
                        state.notice = Some(format!(
                            "Hand {number}: {} failed ({})",
                            error.phase, error.error
                        ));
                    }
                }
                if let Some(reason) = halted {
                    self.stop(self.halt_message(&reason));
                }
            }
            Outcome::Abandoned => {
                self.stats.borrow_mut().halt_reason = None;
                self.stop(format!(
                    "Provider outage: hand {number} was abandoned, not played with a fallback. Resume to deal again."
                ));
            }
            Outcome::Stopped(reason) => self.stop(self.halt_message(&reason)),
        }
        Ok(())
    }

    fn halt_message(&self, reason: &str) -> String {
        match reason {
            "reported_spend_limit" | "global_spend_limit" => format!(
                "Spend cap reached: ${:.4} of ${:.2}. Raise the cap in CONFIG to keep playing.",
                self.spent(),
                self.applied.as_ref().map_or(0.0, |s| s.spend_cap_usd)
            ),
            "three_consecutive_failures" => {
                "The model failed three decisions in a row (played as fallbacks). Check the MIND tab, then resume.".into()
            }
            "http_configuration_or_quota_error" => {
                "OpenRouter refused the request (key, credit, model ID or provider settings). Check CONFIG.".into()
            }
            other => format!("Table stopped: {other}. Resume to try again."),
        }
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let wanted = {
                let mut state = self.shared.lock();
                while (state.paused || state.phase != Phase::Playing) && !state.quit {
                    state = self
                        .shared
                        .wake
                        .wait(state)
                        .unwrap_or_else(|e| e.into_inner());
                }
                if state.quit {
                    return Ok(());
                }
                state.settings.clone()
            };
            let changed = self.applied.as_ref().is_none_or(|applied| {
                applied.seat_differs(&wanted) || applied.reset_stacks != wanted.reset_stacks
            });
            if changed || self.needs_rebuild {
                if wanted.brain == Brain::Model && self.spent() >= wanted.spend_cap_usd {
                    self.stop(self.halt_message("reported_spend_limit"));
                    continue;
                }
                if let Err(error) = self.apply(&wanted) {
                    self.stop(format!("Cannot seat the player: {error}"));
                    continue;
                }
                self.shared.lock().notice = None;
            }
            self.play_hand()?;
        }
    }
}

/// Starts the game thread. It waits in `Phase::Setup` until the spectator starts the game.
pub fn spawn(shared: Arc<Shared>, env: Env) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        // The seed is fixed when play starts, so the table is built on the first start.
        let settings = {
            let mut state = shared.lock();
            while state.phase != Phase::Playing && !state.quit {
                state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            if state.quit {
                return;
            }
            state.settings.clone()
        };
        let result = Game::new(shared.clone(), env, &settings).and_then(|mut game| game.run());
        if let Err(error) = result {
            let mut state = shared.lock();
            state.paused = true;
            state.notice = Some(format!("The table stopped on an error: {error}"));
        }
    })
}

/// Expected big blinds the choices on one graded street gave up.
fn street_bb(grades: &[RiverGrade], street: Street, big_blind: u64) -> f64 {
    grades
        .iter()
        .filter(|g| g.street == street)
        .map(|g| g.loss)
        .sum::<f64>()
        / big_blind as f64
}

/// Loads a saved lab run (a session directory with `evaluator.jsonl`) for replay and analysis.
pub fn load_run(dir: &Path) -> Result<State, String> {
    let at = |e: std::io::Error| format!("{}: {e}", dir.display());
    let manifest: Option<Value> = File::open(dir.join("manifest.json"))
        .ok()
        .and_then(|file| serde_json::from_reader(file).ok());
    let config: Option<Config> = manifest
        .as_ref()
        .and_then(|m| serde_json::from_value(m["config"].clone()).ok());
    let mut state = State::new(Settings::default());
    state.phase = Phase::Review;
    state.paused = true;
    state.run_dir = Some(dir.to_path_buf());
    let mut help = "LAB RUN".to_owned();
    if let Some(config) = &config {
        state.hero = config.model_seat;
        state.names = display_names(&config.seat_labels());
        help = config.condition().to_uppercase();
        if config.memory == Mode::Oracle {
            state.notebook = config.oracle_notes();
        }
    }
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut grader = config
        .as_ref()
        .and_then(|c| RiverGrader::for_bots(&c.seat_bots(), &[c.model_seat], true).ok());
    let mut totals = HelpTotals {
        help: help.clone(),
        ..HelpTotals::default()
    };
    for (index, line) in BufReader::new(File::open(dir.join("evaluator.jsonl")).map_err(at)?)
        .lines()
        .enumerate()
    {
        let record: Record = serde_json::from_str(&line.map_err(at)?).map_err(|e| e.to_string())?;
        if config.is_none() && index == 0 {
            state.names = (0..record.initial_stacks.len())
                .map(|s| format!("SEAT {s}"))
                .collect();
        }
        let models: BTreeSet<String> = [state.names[state.hero].clone()].into();
        analyze::tally(&record, &state.names, &models, true, &mut tallies)?;
        let analysis = analyse(&record, state.hero)?;
        let river = match grader.as_mut().map(|g| g.grade_hand(&record)) {
            Some(Ok(grades)) => grades,
            Some(Err(error)) => {
                grader = None;
                state.notice = Some(format!(
                    "River and turn grading stopped at hand {}: {error}",
                    index + 1
                ));
                vec![]
            }
            None => vec![],
        };
        tallies
            .entry(state.names[state.hero].clone())
            .or_default()
            .add_river(&river);
        totals.hands += 1;
        totals.net_bb += analysis.net_bb();
        totals.adjusted_bb += analysis.adjusted_bb();
        totals.mistake_bb += analysis.mistake_cost / analysis.big_blind as f64;
        totals.river_bb += street_bb(&river, Street::River, analysis.big_blind);
        totals.turn_bb += street_bb(&river, Street::Turn, analysis.big_blind);
        state.hands.push(HandLog {
            number: index + 1,
            frames: frames::of_record(&record, &state.names)?,
            analysis,
            river,
            record,
            help: help.clone(),
        });
    }
    if state.hands.is_empty() {
        return Err(format!("{}: no hands in evaluator.jsonl", dir.display()));
    }
    let big_blind = state.hands[0].record.rules.big_blind;
    state.stats.rows = state
        .names
        .iter()
        .enumerate()
        .map(|(seat, name)| {
            tallies.get(name).cloned().unwrap_or_default().style(
                name,
                seat == state.hero,
                big_blind,
            )
        })
        .collect();
    state.stats.by_help = vec![totals];
    if let Some(summary) = File::open(dir.join("summary.json"))
        .ok()
        .and_then(|file| serde_json::from_reader::<_, Value>(file).ok())
    {
        let stats = &summary["stats"];
        let count = |key: &str| stats[key].as_u64().unwrap_or(0);
        state.stats.lab = Stats {
            requests: count("requests"),
            memory_requests: count("memory_requests"),
            memory_errors: count("memory_errors"),
            fallbacks: count("fallbacks"),
            prompt_tokens: count("prompt_tokens"),
            completion_tokens: count("completion_tokens"),
            reported_cost_usd: stats["reported_cost_usd"].as_f64().unwrap_or(0.0),
            repaired_actions: count("repaired_actions"),
            normalized_actions: count("normalized_actions"),
            infra_failures: count("infra_failures"),
            ..Stats::default()
        };
    }
    if let Ok(file) = File::open(dir.join("model-private.jsonl")) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                state.ingest_audit(&value);
            }
        }
        for thought in &mut state.thoughts {
            thought.received = None;
            thought.waiting = false;
        }
    }
    Ok(state)
}
