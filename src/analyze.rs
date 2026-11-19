// ABOUTME: Offline analysis of a run or batch directory: per-seat playing style, fallback rate,
// ABOUTME: cost per hand, and win rates with confidence intervals over independent seed blocks.
use crate::{
    arena::Record,
    engine::{Action, Event, Hand, Street},
    evaluation,
    openrouter::Config,
    river::{self, Kind},
    scaffold,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

/// Two-sided 95% Student t critical values for 1..=30 degrees of freedom.
const T95: [f64; 30] = [
    12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
    2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
    2.052, 2.048, 2.045, 2.042,
];

/// Mean, standard error and 95% t-interval of independent block values. With fewer than two
/// blocks there is no interval: the mean is reported alone.
pub fn interval(values: &[f64]) -> Value {
    let n = values.len();
    if n == 0 {
        return json!({"blocks":0});
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return json!({"blocks":n,"mean":mean});
    }
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let se = (variance / n as f64).sqrt();
    let t = T95.get(n - 2).copied().unwrap_or(1.96);
    json!({"blocks":n,"mean":mean,"se":se,"ci95":[mean - t * se, mean + t * se]})
}

/// Hands per segment of a session's learning curve: hands 1-20, 21-40, 41-60, ...
pub const SEGMENT_HANDS: usize = 20;

/// One seed block of one (condition, label): the independent unit of every interval.
#[derive(Default)]
struct Block {
    net: i64,
    hands: u64,
    adjusted: f64,
    loss: f64,
    river_loss: f64,
    turn_loss: f64,
    /// The same sums by segment of the session (`SEGMENT_HANDS`), for learning curves.
    segments: BTreeMap<usize, Block>,
}
impl Block {
    fn add(&mut self, t: &Tally) {
        self.net += t.net_chips;
        self.adjusted += t.adjusted_net;
        self.loss += t.grade_loss;
        self.river_loss += t.river_loss;
        self.turn_loss += t.turn_loss;
        self.hands += t.hands;
    }
}

/// One of a block's sums, by name.
type Measure = (&'static str, fn(&Block) -> f64);
/// The learning-curve measures: luck-adjusted chips, river cost and turn cost.
const CURVE: [Measure; 3] = [
    ("adjusted", |b| b.adjusted),
    ("river_cost", |b| b.river_loss),
    ("turn_cost", |b| b.turn_loss),
];

/// "1-20" for segment 0.
fn segment_name(segment: usize) -> String {
    format!(
        "{}-{}",
        segment * SEGMENT_HANDS + 1,
        (segment + 1) * SEGMENT_HANDS
    )
}

/// Behaviour and results of one seat label, accumulated hand by hand with `tally`.
#[derive(Default, Clone)]
pub struct Tally {
    hands: u64,
    preflop_hands: u64,
    vpip: u64,
    pfr: u64,
    aggressive: u64,
    calls: u64,
    faced_bet: u64,
    folded_to_bet: u64,
    showdowns: u64,
    decisions: u64,
    fallbacks: u64,
    /// Model decisions compared with the reference rule, and how many matched its action kind.
    compared: u64,
    agreed: u64,
    net_chips: i64,
    /// Net chips with all-in luck removed (expected results over the unseen board).
    adjusted_net: f64,
    /// Call-or-fold decisions graded against pot odds, how many were mistakes, and the chips
    /// those mistakes gave up (see `evaluation::grade_seat`).
    graded: u64,
    bad_calls: u64,
    bad_folds: u64,
    grade_loss: f64,
    /// River decisions graded exactly against the bots (see `river::grade_session`): how many,
    /// the chips the choices gave up, and the count and chips of each kind of mistake.
    river_graded: u64,
    river_loss: f64,
    river_kinds: BTreeMap<Kind, (u64, f64)>,
    /// Turn decisions graded approximately against the bots (see `crate::turn`), as above.
    turn_graded: u64,
    turn_loss: f64,
    turn_kinds: BTreeMap<Kind, (u64, f64)>,
    /// Leak counts: postflop bets or raises with under 30% equity against the cards actually
    /// held, how many of those were called down to a showdown, and truncated answers.
    bluffs: u64,
    bluffs_called_down: u64,
    truncations: u64,
}
impl Tally {
    pub fn add(&mut self, t: &Tally) {
        for (sum, add) in [
            (&mut self.hands, t.hands),
            (&mut self.preflop_hands, t.preflop_hands),
            (&mut self.vpip, t.vpip),
            (&mut self.pfr, t.pfr),
            (&mut self.aggressive, t.aggressive),
            (&mut self.calls, t.calls),
            (&mut self.faced_bet, t.faced_bet),
            (&mut self.folded_to_bet, t.folded_to_bet),
            (&mut self.showdowns, t.showdowns),
            (&mut self.decisions, t.decisions),
            (&mut self.fallbacks, t.fallbacks),
            (&mut self.compared, t.compared),
            (&mut self.agreed, t.agreed),
            (&mut self.graded, t.graded),
            (&mut self.bad_calls, t.bad_calls),
            (&mut self.bad_folds, t.bad_folds),
        ] {
            *sum += add;
        }
        self.net_chips += t.net_chips;
        self.adjusted_net += t.adjusted_net;
        self.grade_loss += t.grade_loss;
        self.river_graded += t.river_graded;
        self.river_loss += t.river_loss;
        for (kind, (n, chips)) in &t.river_kinds {
            let entry = self.river_kinds.entry(*kind).or_default();
            entry.0 += n;
            entry.1 += chips;
        }
        self.turn_graded += t.turn_graded;
        self.turn_loss += t.turn_loss;
        for (kind, (n, chips)) in &t.turn_kinds {
            let entry = self.turn_kinds.entry(*kind).or_default();
            entry.0 += n;
            entry.1 += chips;
        }
        self.bluffs += t.bluffs;
        self.bluffs_called_down += t.bluffs_called_down;
        self.truncations += t.truncations;
    }
    /// Adds exact river grades, and approximate turn grades if any, for this label.
    pub fn add_river(&mut self, grades: &[river::RiverGrade]) {
        for g in grades {
            let (graded, loss, kinds) = if g.street == Street::Turn {
                (
                    &mut self.turn_graded,
                    &mut self.turn_loss,
                    &mut self.turn_kinds,
                )
            } else {
                (
                    &mut self.river_graded,
                    &mut self.river_loss,
                    &mut self.river_kinds,
                )
            };
            *graded += 1;
            *loss += g.loss;
            let entry = kinds.entry(g.kind).or_default();
            entry.0 += 1;
            entry.1 += g.loss;
        }
    }
    /// The label's style and result row, as reported by `analyze`.
    pub fn style(&self, label: &str, is_model: bool, big_blind: u64) -> Value {
        json!({"label":label,"is_model":is_model,"seat_hands":self.hands,
            "vpip":ratio(self.vpip, self.preflop_hands),"pfr":ratio(self.pfr, self.preflop_hands),
            "aggression_factor":ratio(self.aggressive, self.calls),
            "fold_to_bet":ratio(self.folded_to_bet, self.faced_bet),
            "showdown":ratio(self.showdowns, self.hands),
            "fallback_rate":ratio(self.fallbacks, self.decisions),"fallbacks":self.fallbacks,
            "reference_agreement":ratio(self.agreed, self.compared),
            "adjusted_bb_per_100":(self.hands > 0).then(|| self.adjusted_net / big_blind as f64 / self.hands as f64 * 100.0),
            "graded_decisions":self.graded,"bad_calls":self.bad_calls,"bad_folds":self.bad_folds,
            "mistake_cost_bb_per_100":(self.hands > 0).then(|| self.grade_loss / big_blind as f64 / self.hands as f64 * 100.0),
            "river_graded":self.river_graded,
            "river_cost_bb_per_100":(self.hands > 0 && self.river_graded > 0).then(|| self.river_loss / big_blind as f64 / self.hands as f64 * 100.0),
            "river_kinds":self.river_kinds.iter().map(|(k, (n, chips))| (k.name().to_owned(), json!({"count":n,"bb_per_100":(self.hands > 0).then(|| chips / big_blind as f64 / self.hands as f64 * 100.0)}))).collect::<serde_json::Map<String, Value>>(),
            "turn_graded":self.turn_graded,
            "turn_cost_bb_per_100":(self.hands > 0 && self.turn_graded > 0).then(|| self.turn_loss / big_blind as f64 / self.hands as f64 * 100.0),
            "turn_kinds":self.turn_kinds.iter().map(|(k, (n, chips))| (k.name().to_owned(), json!({"count":n,"bb_per_100":(self.hands > 0).then(|| chips / big_blind as f64 / self.hands as f64 * 100.0)}))).collect::<serde_json::Map<String, Value>>(),
            "bluffs":self.bluffs,"bluffs_called_down":self.bluffs_called_down,"truncations":self.truncations,
            "net_chips":self.net_chips,"bb_per_100":bb_per_100(self.net_chips, big_blind, self.hands)})
    }
}

struct SessionData {
    /// The session directory's name, for messages.
    name: String,
    labels: Vec<String>,
    /// Scripted style per seat (None for model seats), when the run used the mixed field.
    /// The bots each seat had, with their drawn personalities when the field varied.
    styles: Option<Vec<Option<crate::opponents::SeatBot>>>,
    models: BTreeSet<String>,
    condition: String,
    seed: u64,
    records: Vec<Record>,
    requests: u64,
    cost: f64,
    clean: bool,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_reader(File::open(path).ok()?).ok()
}

fn load(dir: &Path) -> Result<SessionData, String> {
    let file = File::open(dir.join("evaluator.jsonl"))
        .map_err(|e| format!("{}: {e}", dir.join("evaluator.jsonl").display()))?;
    let mut records = vec![];
    for text in BufReader::new(file).lines() {
        let text = text.map_err(|e| e.to_string())?;
        records.push(serde_json::from_str::<Record>(&text).map_err(|e| e.to_string())?);
    }
    let seats = records.first().map_or(0, |r| r.initial_stacks.len());
    let manifest = read_json(&dir.join("manifest.json"));
    let summary = read_json(&dir.join("summary.json"));
    let config: Option<Config> = manifest
        .as_ref()
        .and_then(|m| serde_json::from_value(m["config"].clone()).ok());
    // Scripted-only runs have no manifest: seats are labelled by number.
    let labels = match &config {
        Some(config) => config.seat_labels(),
        None => (0..seats).map(|s| format!("seat {s}")).collect(),
    };
    let models = config
        .as_ref()
        .map(|c| {
            c.llm_seats()
                .into_iter()
                .map(|s| labels[s].clone())
                .collect()
        })
        .unwrap_or_default();
    let stats = summary.as_ref().map(|s| &s["stats"]);
    let styles = config.as_ref().and_then(|c| {
        let styles = c.seat_bots();
        let llm = c.llm_seats();
        // Gradable only when every non-model seat is a known scripted style.
        (0..styles.len())
            .all(|s| styles[s].is_some() || llm.contains(&s))
            .then_some(styles)
    });
    Ok(SessionData {
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        labels,
        styles,
        models,
        condition: config
            .as_ref()
            .map(Config::condition)
            .unwrap_or_else(|| "none".into()),
        seed: config.as_ref().map_or(0, |c| c.seed),
        records,
        requests: stats.and_then(|s| s["requests"].as_u64()).unwrap_or(0),
        cost: stats
            .and_then(|s| s["reported_cost_usd"].as_f64())
            .unwrap_or(0.0),
        // Runs written before the explicit flag existed are clean when nothing fell back.
        clean: summary.as_ref().is_none_or(|s| {
            s["clean"].as_bool().unwrap_or(
                s["stats"]["fallbacks"] == 0
                    && s["stats"]["halt_reason"].is_null()
                    && s["completed_hands"] == s["requested_hands"],
            )
        }),
    })
}

/// Replays one hand and adds each seat's behaviour to its label's tally. Decisions by `models`
/// labels are also compared with the scaffold reference rule (the milestone 9 advice).
pub fn tally(
    record: &Record,
    labels: &[String],
    models: &BTreeSet<String>,
    grade: bool,
    out: &mut BTreeMap<String, Tally>,
) -> Result<(), String> {
    let seats = record.initial_stacks.len();
    let adjusted = evaluation::allin_adjusted(record)?;
    let holes = evaluation::holes(record);
    let mut hand = Hand::new(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        record.seed,
    )?;
    let mut entered = vec![(false, false, false); seats]; // (acted preflop, vpip, pfr)
    for decision in &record.decisions {
        let street = hand.view(decision.seat)?.street;
        let facing = hand.legal().call.is_some();
        let t = out.entry(labels[decision.seat].clone()).or_default();
        t.decisions += 1;
        t.fallbacks += u64::from(decision.error.is_some());
        t.truncations += u64::from(
            decision
                .error
                .as_deref()
                .is_some_and(|e| e.contains("Truncated")),
        );
        if models.contains(&labels[decision.seat]) {
            let view = hand.view(decision.seat)?;
            if let Some(reference) = scaffold::suggestion(&view) {
                t.compared += 1;
                t.agreed +=
                    u64::from(scaffold::kind(reference) == scaffold::kind(decision.applied));
            }
        }
        let raise = matches!(decision.applied, Action::RaiseTo(_));
        if street == Street::Preflop {
            let seat = &mut entered[decision.seat];
            seat.0 = true;
            seat.1 |= raise || decision.applied == Action::Call;
            seat.2 |= raise;
        } else {
            t.aggressive += u64::from(raise);
            if raise && grade && models.contains(&labels[decision.seat]) {
                let view = hand.view(decision.seat)?;
                let opponents: Vec<_> = view
                    .players
                    .iter()
                    .filter(|p| !p.folded && p.seat != decision.seat)
                    .map(|p| holes[p.seat])
                    .collect();
                if evaluation::equity_against(holes[decision.seat], &view.board, &opponents) < 0.3 {
                    t.bluffs += 1;
                    t.bluffs_called_down += u64::from(record.events.iter().any(
                        |e| matches!(e, Event::Reveal { seat, .. } if *seat == decision.seat),
                    ));
                }
            }
            t.calls += u64::from(decision.applied == Action::Call);
            t.faced_bet += u64::from(facing);
            t.folded_to_bet += u64::from(facing && decision.applied == Action::Fold);
        }
        hand.act(decision.seat, decision.applied)?;
    }
    for seat in 0..seats {
        let t = out.entry(labels[seat].clone()).or_default();
        t.hands += 1;
        t.preflop_hands += u64::from(entered[seat].0);
        t.vpip += u64::from(entered[seat].1);
        t.pfr += u64::from(entered[seat].2);
        t.showdowns += u64::from(
            record
                .events
                .iter()
                .any(|e| matches!(e, Event::Reveal { seat: s, .. } if *s == seat)),
        );
        t.net_chips += record.final_stacks[seat] as i64 - record.initial_stacks[seat] as i64;
        t.adjusted_net += adjusted
            .as_ref()
            .map_or(record.final_stacks[seat] as f64, |a| a[seat])
            - record.initial_stacks[seat] as f64;
        if grade {
            for g in evaluation::grade_seat(record, seat)? {
                t.graded += 1;
                t.grade_loss += g.loss;
                if g.loss > 0.0 {
                    t.bad_calls += u64::from(g.called);
                    t.bad_folds += u64::from(!g.called);
                }
            }
        }
    }
    Ok(())
}

fn ratio(a: u64, b: u64) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}
fn bb_per_100(net_chips: i64, big_blind: u64, hands: u64) -> Option<f64> {
    (hands > 0).then(|| net_chips as f64 / big_blind as f64 / hands as f64 * 100.0)
}

/// Analyses a single run directory or a batch directory of `session-NNN` runs.
/// The session directories of a run or batch: its children holding `evaluator.jsonl`, sorted,
/// or the directory itself when it is a single session.
fn session_dirs(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("evaluator.jsonl").exists())
        .collect();
    dirs.sort();
    if dirs.is_empty() {
        dirs.push(dir.to_path_buf());
    }
    Ok(dirs)
}

/// One row per decision by a model seat (every seat when a run has no manifest), for reading
/// a run decision by decision: the spot, the action taken, and the seat's equity against the
/// cards its live opponents actually held (the board still to come unknown, as in the
/// decision grade). When facing a bet: the pot-odds share required, EV(call) and who bet.
/// Evaluator-only: rows reveal hidden cards.
pub fn decisions(dir: &Path) -> Result<Vec<Value>, String> {
    let mut rows = vec![];
    for session_dir in session_dirs(dir)? {
        let session = load(&session_dir)?;
        let name = session_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Exact river grades of the model seats, keyed by (hand, decision).
        let mut river_grades = BTreeMap::new();
        let mut river_error = None;
        if let Some(styles) = &session.styles {
            let model_seats: Vec<usize> = (0..session.labels.len())
                .filter(|s| session.models.contains(&session.labels[*s]))
                .collect();
            if !model_seats.is_empty() {
                match river::grade_session_bots(&session.records, styles, &model_seats, false) {
                    Ok(grades) => {
                        for g in grades {
                            river_grades.insert((g.hand, g.decision), g);
                        }
                    }
                    Err(error) => river_error = Some(error),
                }
            }
        }
        for (index, record) in session.records.iter().enumerate() {
            let holes = evaluation::holes(record);
            let mut hand = Hand::new(
                record.rules.clone(),
                record.initial_stacks.clone(),
                record.button,
                record.seed,
            )?;
            // Last seat to bet or raise on the current street.
            let mut bettor: Option<usize> = None;
            let mut street = Street::Preflop;
            for (number, d) in record.decisions.iter().enumerate() {
                let view = hand.view(d.seat)?;
                if view.street != street {
                    street = view.street;
                    bettor = None;
                }
                let label = &session.labels[d.seat];
                if session.models.is_empty() || session.models.contains(label) {
                    let call = view.legal.as_ref().and_then(|l| l.call).unwrap_or(0);
                    let opponents: Vec<_> = view
                        .players
                        .iter()
                        .filter(|p| !p.folded && p.seat != d.seat)
                        .map(|p| holes[p.seat])
                        .collect();
                    let equity = evaluation::equity_against(holes[d.seat], &view.board, &opponents);
                    let facing = call > 0;
                    let pot = view.pot as f64;
                    let to_text = |cards: &[crate::cards::Card], sep: &str| {
                        cards
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(sep)
                    };
                    rows.push(json!({
                        "session": name, "seed": record.seed, "condition": session.condition,
                        "hand": index + 1, "decision": number, "seat": d.seat, "label": label,
                        "street": view.street, "hole": to_text(&holes[d.seat], ""),
                        "board": to_text(&view.board, " "), "pot": view.pot, "call": call,
                        "stack": view.players[d.seat].stack, "opponents": opponents.len(),
                        "action": d.applied, "error": d.error, "equity": equity,
                        "required": facing.then_some(call as f64 / (pot + call as f64)),
                        "ev_call": facing.then_some(equity * (pot + call as f64) - call as f64),
                        "bettor": if facing { bettor.map(|s| session.labels[s].clone()) } else { None },
                        "river": river_grades.get(&(index, number)).map(|g| json!({
                            "kind": g.kind, "best": g.best, "loss": g.loss,
                            "chosen_value": g.chosen_value, "best_value": g.best_value,
                            "range_sizes": g.range_sizes,
                        })),
                    }));
                    if let Some(error) = &river_error {
                        rows.last_mut().expect("just pushed")["river_error"] = json!(error);
                    }
                }
                if matches!(d.applied, Action::RaiseTo { .. }) {
                    bettor = Some(d.seat);
                }
                hand.act(d.seat, d.applied)?;
            }
        }
    }
    Ok(rows)
}

pub fn directory(dir: &Path) -> Result<Value, String> {
    directory_with(dir, true)
}

/// As `directory`; `grade` turns the per-decision pot-odds grading on or off (it is the slow
/// part: one equity estimate per graded decision).
pub fn directory_with(dir: &Path, grade: bool) -> Result<Value, String> {
    directory_for_seat(dir, grade, false, None)
}

/// As `directory_with`; `turn` also grades turn decisions approximately (`crate::turn`), which
/// takes several times as long as the river grade.
pub fn directory_graded(dir: &Path, grade: bool, turn: bool) -> Result<Value, String> {
    directory_for_seat(dir, grade, turn, None)
}

/// As `directory_graded`, keeping only the sessions whose model played `seat` alone (all
/// sessions when `None`), so batches that rotated the model through different seats can be
/// compared with ones that did not.
pub fn directory_for_seat(
    dir: &Path,
    grade: bool,
    turn: bool,
    seat: Option<usize>,
) -> Result<Value, String> {
    let session_dirs = session_dirs(dir)?;
    let mut sessions = session_dirs
        .iter()
        .map(|d| load(d))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(seat) = seat {
        sessions.retain(|s| {
            let model_seats: Vec<usize> = (0..s.labels.len())
                .filter(|i| s.models.contains(&s.labels[*i]))
                .collect();
            model_seats == [seat]
        });
    }
    let mut totals: BTreeMap<String, Tally> = BTreeMap::new();
    // Model behaviour split by treatment, so a scaffold's effect on style is visible.
    let mut by_condition: BTreeMap<(String, String), Tally> = BTreeMap::new();
    // (condition, label) -> seed -> (net chips, seat-hands). One seed is one independent block:
    // every seat rotation of a seed shares its deals, so rotations are averaged, not counted.
    let mut blocks: BTreeMap<(String, String), BTreeMap<u64, Block>> = BTreeMap::new();
    let mut models = BTreeSet::new();
    let mut conditions = vec![];
    let (mut hands, mut requests, mut cost, mut clean) = (0u64, 0u64, 0.0, 0usize);
    let mut big_blind = 2;
    let mut river_errors: Vec<String> = vec![];
    let street_grades = if grade {
        grade_sessions(&sessions, turn)
    } else {
        vec![None; sessions.len()]
    };
    for (index, session) in sessions.iter().enumerate() {
        let mut local: BTreeMap<String, Tally> = BTreeMap::new();
        // (label, segment) -> tally of that stretch of the session, for learning curves.
        let mut segments: BTreeMap<(String, usize), Tally> = BTreeMap::new();
        for (number, record) in session.records.iter().enumerate() {
            let mut one: BTreeMap<String, Tally> = BTreeMap::new();
            tally(record, &session.labels, &session.models, grade, &mut one)?;
            for (label, t) in one {
                local.entry(label.clone()).or_default().add(&t);
                segments
                    .entry((label, number / SEGMENT_HANDS))
                    .or_default()
                    .add(&t);
            }
            big_blind = record.rules.big_blind;
        }
        if let Some((model_seats, graded)) = &street_grades[index] {
            match graded {
                Ok(grades) => {
                    for seat in model_seats {
                        let label = &session.labels[*seat];
                        let own: Vec<_> =
                            grades.iter().filter(|g| g.seat == *seat).cloned().collect();
                        local.entry(label.clone()).or_default().add_river(&own);
                        for g in own {
                            segments
                                .entry((label.clone(), g.hand / SEGMENT_HANDS))
                                .or_default()
                                .add_river(std::slice::from_ref(&g));
                        }
                    }
                }
                // A run whose bots do not match its manifest cannot be graded; it is reported,
                // not fatal, so the rest of the analysis still stands.
                Err(error) => river_errors.push(format!("{}: {error}", session.name)),
            }
        }
        hands += session.records.len() as u64;
        requests += session.requests;
        cost += session.cost;
        clean += usize::from(session.clean);
        models.extend(session.models.iter().cloned());
        if !conditions.contains(&session.condition) {
            conditions.push(session.condition.clone());
        }
        for ((label, segment), t) in &segments {
            blocks
                .entry((session.condition.clone(), label.clone()))
                .or_default()
                .entry(session.seed)
                .or_default()
                .segments
                .entry(*segment)
                .or_default()
                .add(t);
        }
        for (label, t) in local {
            blocks
                .entry((session.condition.clone(), label.clone()))
                .or_default()
                .entry(session.seed)
                .or_default()
                .add(&t);
            if session.models.contains(&label) {
                by_condition
                    .entry((session.condition.clone(), label.clone()))
                    .or_default()
                    .add(&t);
            }
            totals.entry(label).or_default().add(&t);
        }
    }
    let style: Vec<Value> = totals
        .iter()
        .map(|(label, t)| t.style(label, models.contains(label), big_blind))
        .collect();
    let model_style: Vec<Value> = by_condition
        .iter()
        .map(|((condition, label), t)| {
            let mut row = t.style(label, true, big_blind);
            row["condition"] = json!(condition);
            row
        })
        .collect();
    // Per seed block: bb/100 of one block field (net chips, all-in adjusted, or mistake cost).
    let per_block = |key: &(String, String), field: fn(&Block) -> f64| -> BTreeMap<u64, f64> {
        blocks[key]
            .iter()
            .filter(|(_, b)| b.hands > 0)
            .map(|(seed, b)| (*seed, field(b) / big_blind as f64 / b.hands as f64 * 100.0))
            .collect()
    };
    let block_values = |key: &(String, String)| per_block(key, |b| b.net as f64);
    let adjusted_values = |key: &(String, String)| per_block(key, |b| b.adjusted);
    let loss_values = |key: &(String, String)| per_block(key, |b| b.loss);
    let river_values = |key: &(String, String)| per_block(key, |b| b.river_loss);
    let turn_values = |key: &(String, String)| per_block(key, |b| b.turn_loss);
    let paired_rows = |values: &BTreeMap<u64, f64>, base: &BTreeMap<u64, f64>| {
        let differences: Vec<f64> = values
            .iter()
            .filter_map(|(seed, v)| Some(v - base.get(seed)?))
            .collect();
        interval(&differences)
    };
    let mut win_rates = vec![];
    let mut paired = vec![];
    for key in blocks.keys() {
        let values = block_values(key);
        let seat_hands: u64 = blocks[key].values().map(|b| b.hands).sum();
        let mut row = interval(&values.values().copied().collect::<Vec<_>>());
        row["condition"] = json!(key.0);
        row["label"] = json!(key.1);
        row["seat_hands"] = json!(seat_hands);
        let adjusted = adjusted_values(key);
        row["adjusted"] = interval(&adjusted.values().copied().collect::<Vec<_>>());
        let losses = loss_values(key);
        row["mistake_cost"] = interval(&losses.values().copied().collect::<Vec<_>>());
        let rivers = river_values(key);
        row["river_cost"] = interval(&rivers.values().copied().collect::<Vec<_>>());
        let turns = turn_values(key);
        if turn {
            row["turn_cost"] = interval(&turns.values().copied().collect::<Vec<_>>());
        }
        let curve = learning_curve(&blocks[key], big_blind, turn);
        row["by_segment"] = curve.by_segment;
        row["late_minus_early"] = curve.late_minus_early;
        // Per-seed values, so runs in different directories can be paired on shared deals
        // (`poker-lab compare`).
        row["per_seed"] = values
            .keys()
            .map(|seed| {
                let mut v = json!({"bb_per_100":values[seed],"adjusted":adjusted.get(seed),
                    "hands":blocks[key][seed].hands,
                    "river_cost":rivers.get(seed),"turn_cost":turn.then(|| turns.get(seed)).flatten()});
                for (name, learned) in &curve.per_seed[seed] {
                    v[format!("learn_{name}")] = json!(learned);
                }
                (seed.to_string(), v)
            })
            .collect::<serde_json::Map<String, Value>>()
            .into();
        win_rates.push(row);
        // Paired comparison against the first condition on the seeds both conditions played.
        let reference = (conditions[0].clone(), key.1.clone());
        if key.0 != conditions[0] && models.contains(&key.1) && blocks.contains_key(&reference) {
            let mut row = paired_rows(&values, &block_values(&reference));
            row["adjusted"] = paired_rows(&adjusted, &adjusted_values(&reference));
            row["mistake_cost"] = paired_rows(&losses, &loss_values(&reference));
            row["river_cost"] = paired_rows(&rivers, &river_values(&reference));
            if turn {
                row["turn_cost"] = paired_rows(&turns, &turn_values(&reference));
            }
            row["label"] = json!(key.1);
            row["condition"] = json!(key.0);
            row["versus"] = json!(conditions[0]);
            // Learning relative to the reference: (late minus early) minus the reference's
            // (late minus early), paired by seed. The cards differ by hand number; the
            // reference arm saw the same cards, so this difference is what the treatment adds.
            let base = learning_curve(&blocks[&reference], big_blind, turn);
            let mut learning = json!({});
            for (index, (name, _)) in CURVE.iter().enumerate() {
                let diffs: Vec<f64> = curve
                    .per_seed
                    .iter()
                    .filter_map(|(seed, v)| {
                        Some(v.get(index)?.1? - base.per_seed.get(seed)?.get(index)?.1?)
                    })
                    .collect();
                if !diffs.is_empty() {
                    learning[*name] = interval(&diffs);
                }
            }
            row["learning"] = learning;
            paired.push(row);
        }
    }
    Ok(
        json!({"directory":dir.display().to_string(),"sessions":sessions.len(),"clean_sessions":clean,
        "hands":hands,"requests":requests,"reported_cost_usd":cost,
        "cost_per_hand_usd":(hands > 0).then(|| cost / hands as f64),
        "unit":"bb/100; intervals are 95% t-intervals over seed blocks (seat rotations of a seed are averaged); `adjusted` removes all-in luck",
        "style":style,"model_style_by_condition":model_style,"win_rates":win_rates,"paired_differences":paired,
        "river_grade_errors":river_errors}),
    )
}

/// A label's learning curve over its seed blocks.
struct Curve {
    /// Per segment: hands, and intervals over seeds of each `CURVE` measure (bb/100).
    by_segment: Value,
    /// Last segment minus first, paired by seed: does the label do better late in a session?
    late_minus_early: Value,
    /// Per seed, the late-minus-early value of each measure (None without two segments).
    per_seed: BTreeMap<u64, Vec<(&'static str, Option<f64>)>>,
}

fn learning_curve(seeds: &BTreeMap<u64, Block>, big_blind: u64, turn: bool) -> Curve {
    let rate = |b: &Block, field: fn(&Block) -> f64| {
        (b.hands > 0).then(|| field(b) / big_blind as f64 / b.hands as f64 * 100.0)
    };
    let measures: Vec<_> = CURVE
        .iter()
        .filter(|(name, _)| turn || *name != "turn_cost")
        .collect();
    let last = seeds
        .values()
        .filter_map(|b| b.segments.keys().max())
        .max()
        .copied()
        .unwrap_or(0);
    let by_segment: Vec<Value> = (0..=last)
        .map(|segment| {
            let parts: Vec<&Block> = seeds
                .values()
                .filter_map(|b| b.segments.get(&segment))
                .collect();
            let mut row = json!({"hands":segment_name(segment),
                "seat_hands":parts.iter().map(|b| b.hands).sum::<u64>()});
            for (name, field) in &measures {
                let values: Vec<f64> = parts.iter().filter_map(|b| rate(b, *field)).collect();
                row[*name] = interval(&values);
            }
            row
        })
        .collect();
    let per_seed: BTreeMap<u64, Vec<(&'static str, Option<f64>)>> = seeds
        .iter()
        .map(|(seed, b)| {
            let first = b.segments.get(&0);
            let late = b.segments.get(&last).filter(|_| last > 0);
            let learned = measures
                .iter()
                .map(|(name, field)| {
                    let diff = first
                        .zip(late)
                        .and_then(|(f, l)| Some(rate(l, *field)? - rate(f, *field)?));
                    (*name, diff)
                })
                .collect();
            (*seed, learned)
        })
        .collect();
    let mut late_minus_early = json!({"late":segment_name(last),"early":segment_name(0)});
    for (index, (name, _)) in measures.iter().enumerate() {
        let diffs: Vec<f64> = per_seed.values().filter_map(|v| v[index].1).collect();
        late_minus_early[*name] = interval(&diffs);
    }
    Curve {
        by_segment: json!(by_segment),
        late_minus_early,
        per_seed,
    }
}

/// The model seats' learning curves: each segment's mean, then late minus early with intervals.
fn learning_text(out: &mut String, a: &Value) {
    let models: BTreeSet<&str> = a["style"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["is_model"] == true)
        .filter_map(|s| s["label"].as_str())
        .collect();
    let rows: Vec<&Value> = a["win_rates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["label"].as_str().is_some_and(|l| models.contains(l)))
        .filter(|r| r["by_segment"].as_array().is_some_and(|s| s.len() > 1))
        .collect();
    if rows.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "\nLearning curve: model seats by hand number within a session (bb/100, mean over seeds; costs lower is better)"
    );
    for row in rows {
        let _ = writeln!(
            out,
            "  {} {}",
            row["condition"].as_str().unwrap_or(""),
            row["label"].as_str().unwrap_or("")
        );
        for segment in row["by_segment"].as_array().into_iter().flatten() {
            let _ = writeln!(
                out,
                "    hands {:<8} adjusted {:>7}   river cost {:>6}   turn cost {:>6}",
                segment["hands"].as_str().unwrap_or(""),
                number(&segment["adjusted"]["mean"]),
                number(&segment["river_cost"]["mean"]),
                number(&segment["turn_cost"]["mean"])
            );
        }
        let learned = &row["late_minus_early"];
        for (name, title) in [
            ("adjusted", "adjusted"),
            ("river_cost", "river cost"),
            ("turn_cost", "turn cost"),
        ] {
            if learned[name]["blocks"].as_u64().unwrap_or(0) > 0 {
                let _ = writeln!(
                    out,
                    "    {} minus {}, {title}: {}",
                    learned["late"].as_str().unwrap_or(""),
                    learned["early"].as_str().unwrap_or(""),
                    interval_text(&learned[name])
                );
            }
        }
    }
}

fn percent(v: &Value) -> String {
    v.as_f64()
        .map_or("    -".into(), |x| format!("{:4.0}%", x * 100.0))
}
fn number(v: &Value) -> String {
    v.as_f64().map_or("-".into(), |x| format!("{x:+.1}"))
}
fn interval_text(row: &Value) -> String {
    match row["ci95"].as_array() {
        Some(ci) => format!(
            "{} ± {:.1} SE, 95% CI [{}, {}], {} blocks",
            number(&row["mean"]),
            row["se"].as_f64().unwrap_or(0.0),
            number(&ci[0]),
            number(&ci[1]),
            row["blocks"]
        ),
        None => format!("{} (one block: no interval)", number(&row["mean"])),
    }
}

/// The model seats of each session and their river (and, with `turn`, turn) grades, or `None`
/// for a session without scripted styles or model seats. Sessions are graded in parallel: the
/// turn grade is the slow part of `analyze`.
type SessionGrades = Option<(Vec<usize>, Result<Vec<river::RiverGrade>, String>)>;
fn grade_sessions(sessions: &[SessionData], turn: bool) -> Vec<SessionGrades> {
    let one = |session: &SessionData| -> SessionGrades {
        let styles = session.styles.as_ref()?;
        let model_seats: Vec<usize> = (0..session.labels.len())
            .filter(|s| session.models.contains(&session.labels[*s]))
            .collect();
        if model_seats.is_empty() {
            return None;
        }
        let graded = river::grade_session_bots(&session.records, styles, &model_seats, turn);
        Some((model_seats, graded))
    };
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = sessions.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = sessions
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(one).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("grading thread panicked"))
            .collect()
    })
}

/// A plain-text rendering of `directory`'s result.
pub fn render(a: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} sessions ({} clean), {} hands, {} requests, ${:.4} reported (${:.6}/hand)",
        a["sessions"],
        a["clean_sessions"],
        a["hands"],
        a["requests"],
        a["reported_cost_usd"].as_f64().unwrap_or(0.0),
        a["cost_per_hand_usd"].as_f64().unwrap_or(0.0)
    );
    let _ = writeln!(
        out,
        "\nPlaying style by seat label\n{:<36} {:>7} {:>5} {:>5} {:>5} {:>7} {:>5} {:>6} {:>9}",
        "label", "hands", "VPIP", "PFR", "AF", "fold/bet", "SD", "fallbk", "bb/100"
    );
    for s in a["style"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "{:<36} {:>7} {} {} {:>5} {:>8} {} {:>6} {:>9}",
            format!(
                "{}{}",
                s["label"].as_str().unwrap_or(""),
                if s["is_model"] == true { " *" } else { "" }
            ),
            s["seat_hands"].as_u64().unwrap_or(0),
            percent(&s["vpip"]),
            percent(&s["pfr"]),
            s["aggression_factor"]
                .as_f64()
                .map_or("-".into(), |x| format!("{x:.2}")),
            percent(&s["fold_to_bet"]),
            percent(&s["showdown"]),
            s["fallbacks"].as_u64().unwrap_or(0),
            number(&s["bb_per_100"])
        );
    }
    let _ = writeln!(
        out,
        "(* model seat. AF = postflop bets and raises per call.)"
    );
    let by_condition = a["model_style_by_condition"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !by_condition.is_empty() {
        let _ = writeln!(
            out,
            "\nModel style by condition\n{:<16} {:>7} {:>5} {:>5} {:>5} {:>8} {:>6} {:>7}",
            "condition", "hands", "VPIP", "PFR", "AF", "fold/bet", "fallbk", "agrees"
        );
        for s in &by_condition {
            let _ = writeln!(
                out,
                "{:<16} {:>7} {} {} {:>5} {:>8} {:>6} {:>7}",
                s["condition"].as_str().unwrap_or(""),
                s["seat_hands"].as_u64().unwrap_or(0),
                percent(&s["vpip"]),
                percent(&s["pfr"]),
                s["aggression_factor"]
                    .as_f64()
                    .map_or("-".into(), |x| format!("{x:.2}")),
                percent(&s["fold_to_bet"]),
                s["fallbacks"].as_u64().unwrap_or(0),
                percent(&s["reference_agreement"])
            );
        }
        let _ = writeln!(
            out,
            "(agrees = share of model decisions matching the reference rule's fold/passive/raise choice.)"
        );
    }
    let _ = writeln!(out, "\nWin rate in bb/100 by condition");
    for row in a["win_rates"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "  {:<16} {:<36} {}",
            row["condition"].as_str().unwrap_or(""),
            row["label"].as_str().unwrap_or(""),
            interval_text(row)
        );
        if row["adjusted"]["blocks"].as_u64().unwrap_or(0) > 0 {
            let _ = writeln!(
                out,
                "  {:<16} {:<36} {}",
                "",
                "  all-in adjusted:",
                interval_text(&row["adjusted"])
            );
        }
    }
    let paired = a["paired_differences"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !paired.is_empty() {
        let _ = writeln!(
            out,
            "\nPaired difference on shared seeds (condition minus reference)"
        );
        for row in &paired {
            let _ = writeln!(
                out,
                "  {} vs {:<8} {:<30} {}",
                row["condition"].as_str().unwrap_or(""),
                row["versus"].as_str().unwrap_or(""),
                row["label"].as_str().unwrap_or(""),
                interval_text(row)
            );
            if row["adjusted"]["blocks"].as_u64().unwrap_or(0) > 0 {
                let _ = writeln!(
                    out,
                    "    all-in adjusted: {}",
                    interval_text(&row["adjusted"])
                );
            }
            if row["mistake_cost"]["blocks"].as_u64().unwrap_or(0) > 0
                && row["mistake_cost"]["mean"].as_f64().unwrap_or(0.0) != 0.0
            {
                let _ = writeln!(
                    out,
                    "    call/fold mistake cost: {}",
                    interval_text(&row["mistake_cost"])
                );
            }
            if row["river_cost"]["blocks"].as_u64().unwrap_or(0) > 0
                && row["river_cost"]["mean"].as_f64().unwrap_or(0.0) != 0.0
            {
                let _ = writeln!(
                    out,
                    "    river cost (exact):     {}",
                    interval_text(&row["river_cost"])
                );
            }
            if row["turn_cost"]["blocks"].as_u64().unwrap_or(0) > 0
                && row["turn_cost"]["mean"].as_f64().unwrap_or(0.0) != 0.0
            {
                let _ = writeln!(
                    out,
                    "    turn cost (approx):     {}",
                    interval_text(&row["turn_cost"])
                );
            }
            for (name, title) in [
                ("adjusted", "adjusted"),
                ("river_cost", "river cost"),
                ("turn_cost", "turn cost"),
            ] {
                if row["learning"][name]["blocks"].as_u64().unwrap_or(0) > 0 {
                    let _ = writeln!(
                        out,
                        "    learning (late minus early, minus the reference's), {title}: {}",
                        interval_text(&row["learning"][name])
                    );
                }
            }
        }
    }
    learning_text(&mut out, a);
    let graded: Vec<&Value> = a["style"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(
            a["model_style_by_condition"]
                .as_array()
                .into_iter()
                .flatten(),
        )
        .filter(|s| s["graded_decisions"].as_u64().unwrap_or(0) > 0)
        .collect();
    if !graded.is_empty() {
        let _ = writeln!(
            out,
            "\nCall-or-fold decisions graded against pot odds\n{:<52} {:>7} {:>9} {:>9} {:>11}",
            "label (condition)", "graded", "bad call", "bad fold", "cost bb/100"
        );
        for s in graded {
            let n = s["graded_decisions"].as_u64().unwrap_or(0);
            let share = |k: &str| {
                format!(
                    "{:.0}%",
                    s[k].as_u64().unwrap_or(0) as f64 / n.max(1) as f64 * 100.0
                )
            };
            let name = match s["condition"].as_str() {
                Some(condition) => format!("{} ({condition})", s["label"].as_str().unwrap_or("")),
                None => s["label"].as_str().unwrap_or("").to_owned(),
            };
            let _ = writeln!(
                out,
                "{:<52} {:>7} {:>9} {:>9} {:>11}",
                name,
                n,
                share("bad_calls"),
                share("bad_folds"),
                s["mistake_cost_bb_per_100"]
                    .as_f64()
                    .map_or("-".into(), |x| format!("{x:.1}"))
            );
        }
        let _ = writeln!(
            out,
            "(Equity against the opponents' actual cards, board unknown; immediate pot odds only, later betting ignored.)"
        );
    }
    let models: Vec<&Value> = a["model_style_by_condition"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    grade_table(
        &mut out,
        &models,
        "river",
        "River decisions graded exactly against the bots",
        "(Every river alternative replayed against the bots' real policies; the model's own later reply is the better of fold and call.)",
    );
    grade_table(
        &mut out,
        &models,
        "turn",
        "Turn decisions graded approximately against the bots",
        "(Every turn alternative replayed over the bots' ranges and sampled river cards; the model's later play is the best of check, pot bet, all-in, fold or call for what it saw. An estimate: compare conditions, not absolute values.)",
    );
    let leaky: Vec<&&Value> = models
        .iter()
        .filter(|s| s["decisions"].as_u64().unwrap_or(1) > 0)
        .collect();
    if !leaky.is_empty() && leaky.iter().any(|s| s["bluffs"].as_u64().is_some()) {
        let _ = writeln!(
            out,
            "\nLeaks\n{:<44} {:>14} {:>13} {:>12} {:>11}",
            "model (condition)", "missed bets", "bluffs", "called down", "truncated"
        );
        for s in leaky {
            let missed = &s["river_kinds"]["missed bet"];
            let bluffs = s["bluffs"].as_u64().unwrap_or(0);
            let down = s["bluffs_called_down"].as_u64().unwrap_or(0);
            let _ = writeln!(
                out,
                "{:<44} {:>14} {:>13} {:>12} {:>11}",
                model_name(s),
                match missed["count"].as_u64() {
                    Some(n) => format!(
                        "{n} ({:.1} bb)",
                        missed["bb_per_100"].as_f64().unwrap_or(0.0)
                    ),
                    None => "-".into(),
                },
                bluffs,
                if bluffs > 0 {
                    format!("{:.0}%", down as f64 / bluffs as f64 * 100.0)
                } else {
                    "-".into()
                },
                s["truncations"]
            );
        }
        let _ = writeln!(
            out,
            "(Missed bets: river checks where a bet won more, with their cost per 100 hands. Bluffs: postflop bets or raises with under 30% equity against the cards held; called down: the bluffer reached showdown. Truncated: answers cut off by the token limit, played as fallbacks.)"
        );
    }
    out.push_str(&river_errors_text(a));
    out
}

/// The per-kind cost table of one graded street (`river` or `turn`), if any decision was graded.
fn grade_table(out: &mut String, models: &[&Value], street: &str, title: &str, footnote: &str) {
    let rows: Vec<&&Value> = models
        .iter()
        .filter(|s| s[format!("{street}_graded")].as_u64().unwrap_or(0) > 0)
        .collect();
    if rows.is_empty() {
        return;
    }
    let kinds = [
        "missed bet",
        "bad bet",
        "sizing",
        "bad call",
        "bad fold",
        "missed raise",
        "bad raise",
    ];
    let _ = writeln!(out, "\n{title} (cost in bb/100; count in brackets)");
    let _ = write!(
        out,
        "{:<44} {:>6} {:>7}",
        "model (condition)", "graded", "total"
    );
    for k in kinds {
        let _ = write!(out, " {k:>13}");
    }
    let _ = writeln!(out);
    for s in rows {
        let _ = write!(
            out,
            "{:<44} {:>6} {:>7}",
            model_name(s),
            s[format!("{street}_graded")],
            s[format!("{street}_cost_bb_per_100")]
                .as_f64()
                .map_or("-".into(), |x| format!("{x:.1}"))
        );
        for k in kinds {
            let entry = &s[format!("{street}_kinds")][k];
            let cell = match entry["count"].as_u64() {
                Some(n) => format!("{:.1} ({n})", entry["bb_per_100"].as_f64().unwrap_or(0.0)),
                None => "-".into(),
            };
            let _ = write!(out, " {cell:>13}");
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{footnote}");
}

fn river_errors_text(a: &Value) -> String {
    let errors: Vec<&str> = a["river_grade_errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if errors.is_empty() {
        String::new()
    } else {
        format!(
            "\nRiver grading skipped for {} session(s) whose bots do not match the manifest:\n  {}\n",
            errors.len(),
            errors.join("\n  ")
        )
    }
}

fn model_name(s: &Value) -> String {
    match s["condition"].as_str() {
        Some(condition) => format!("{} ({condition})", s["label"].as_str().unwrap_or("")),
        None => s["label"].as_str().unwrap_or("").to_owned(),
    }
}

#[cfg(test)]
mod learning_tests {
    use super::*;

    fn block(segments: &[(usize, f64, f64)]) -> Block {
        let mut b = Block::default();
        for (segment, adjusted, river) in segments {
            let part = Block {
                hands: 20,
                adjusted: *adjusted,
                river_loss: *river,
                ..Block::default()
            };
            b.hands += 20;
            b.adjusted += adjusted;
            b.segments.insert(*segment, part);
        }
        b
    }

    #[test]
    fn late_minus_early_is_paired_by_seed() {
        // Big blind 2, 20 hands: 40 chips is +100 bb/100.
        let seeds = BTreeMap::from([
            (1, block(&[(0, 0.0, 40.0), (1, 0.0, 0.0), (2, 40.0, 0.0)])),
            (2, block(&[(0, 40.0, 40.0), (1, 0.0, 0.0), (2, 120.0, 0.0)])),
            // A session cut short after one segment has no late value.
            (3, block(&[(0, 999.0, 0.0)])),
        ]);
        let curve = learning_curve(&seeds, 2, false);
        assert_eq!(curve.by_segment.as_array().unwrap().len(), 3);
        assert_eq!(curve.by_segment[0]["hands"], "1-20");
        assert_eq!(curve.by_segment[2]["hands"], "41-60");
        assert_eq!(curve.by_segment[2]["seat_hands"], 40);
        assert_eq!(curve.late_minus_early["adjusted"]["blocks"], 2);
        assert_eq!(curve.late_minus_early["adjusted"]["mean"], 150.0);
        assert_eq!(curve.late_minus_early["river_cost"]["mean"], -100.0);
        assert!(curve.late_minus_early.get("turn_cost").is_none());
        assert_eq!(curve.per_seed[&3][0], ("adjusted", None));
    }
}
