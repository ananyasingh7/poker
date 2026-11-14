// ABOUTME: Offline analysis of a run or batch directory: per-seat playing style, fallback rate,
// ABOUTME: cost per hand, and win rates with confidence intervals over independent seed blocks.
use crate::{
    arena::Record,
    engine::{Action, Event, Hand, Street},
    openrouter::Config,
    scaffold,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
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

#[derive(Default, Clone)]
struct Tally {
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
}
impl Tally {
    fn add(&mut self, t: &Tally) {
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
        ] {
            *sum += add;
        }
        self.net_chips += t.net_chips;
    }
    fn style(&self, label: &str, is_model: bool, big_blind: u64) -> Value {
        json!({"label":label,"is_model":is_model,"seat_hands":self.hands,
            "vpip":ratio(self.vpip, self.preflop_hands),"pfr":ratio(self.pfr, self.preflop_hands),
            "aggression_factor":ratio(self.aggressive, self.calls),
            "fold_to_bet":ratio(self.folded_to_bet, self.faced_bet),
            "showdown":ratio(self.showdowns, self.hands),
            "fallback_rate":ratio(self.fallbacks, self.decisions),"fallbacks":self.fallbacks,
            "reference_agreement":ratio(self.agreed, self.compared),
            "net_chips":self.net_chips,"bb_per_100":bb_per_100(self.net_chips, big_blind, self.hands)})
    }
}

struct SessionData {
    labels: Vec<String>,
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
    Ok(SessionData {
        labels,
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
fn tally(
    record: &Record,
    labels: &[String],
    models: &BTreeSet<String>,
    out: &mut BTreeMap<String, Tally>,
) -> Result<(), String> {
    let seats = record.initial_stacks.len();
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
pub fn directory(dir: &Path) -> Result<Value, String> {
    let mut session_dirs: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("evaluator.jsonl").exists())
        .collect();
    session_dirs.sort();
    if session_dirs.is_empty() {
        session_dirs.push(dir.to_path_buf());
    }
    let sessions = session_dirs
        .iter()
        .map(|d| load(d))
        .collect::<Result<Vec<_>, _>>()?;
    let mut totals: BTreeMap<String, Tally> = BTreeMap::new();
    // Model behaviour split by treatment, so a scaffold's effect on style is visible.
    let mut by_condition: BTreeMap<(String, String), Tally> = BTreeMap::new();
    // (condition, label) -> seed -> (net chips, seat-hands). One seed is one independent block:
    // every seat rotation of a seed shares its deals, so rotations are averaged, not counted.
    let mut blocks: BTreeMap<(String, String), BTreeMap<u64, (i64, u64)>> = BTreeMap::new();
    let mut models = BTreeSet::new();
    let mut conditions = vec![];
    let (mut hands, mut requests, mut cost, mut clean) = (0u64, 0u64, 0.0, 0usize);
    let mut big_blind = 2;
    for session in &sessions {
        let mut local: BTreeMap<String, Tally> = BTreeMap::new();
        for record in &session.records {
            tally(record, &session.labels, &session.models, &mut local)?;
            big_blind = record.rules.big_blind;
        }
        hands += session.records.len() as u64;
        requests += session.requests;
        cost += session.cost;
        clean += usize::from(session.clean);
        models.extend(session.models.iter().cloned());
        if !conditions.contains(&session.condition) {
            conditions.push(session.condition.clone());
        }
        for (label, t) in local {
            let block = blocks
                .entry((session.condition.clone(), label.clone()))
                .or_default()
                .entry(session.seed)
                .or_default();
            block.0 += t.net_chips;
            block.1 += t.hands;
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
    let block_values = |key: &(String, String)| -> BTreeMap<u64, f64> {
        blocks[key]
            .iter()
            .filter_map(|(seed, (net, hands))| Some((*seed, bb_per_100(*net, big_blind, *hands)?)))
            .collect()
    };
    let mut win_rates = vec![];
    let mut paired = vec![];
    for key in blocks.keys() {
        let values = block_values(key);
        let seat_hands: u64 = blocks[key].values().map(|b| b.1).sum();
        let mut row = interval(&values.values().copied().collect::<Vec<_>>());
        row["condition"] = json!(key.0);
        row["label"] = json!(key.1);
        row["seat_hands"] = json!(seat_hands);
        win_rates.push(row);
        // Paired comparison against the first condition on the seeds both conditions played.
        let reference = (conditions[0].clone(), key.1.clone());
        if key.0 != conditions[0] && models.contains(&key.1) && blocks.contains_key(&reference) {
            let base = block_values(&reference);
            let differences: Vec<f64> = values
                .iter()
                .filter_map(|(seed, v)| Some(v - base.get(seed)?))
                .collect();
            let mut row = interval(&differences);
            row["label"] = json!(key.1);
            row["condition"] = json!(key.0);
            row["versus"] = json!(conditions[0]);
            paired.push(row);
        }
    }
    Ok(
        json!({"directory":dir.display().to_string(),"sessions":sessions.len(),"clean_sessions":clean,
        "hands":hands,"requests":requests,"reported_cost_usd":cost,
        "cost_per_hand_usd":(hands > 0).then(|| cost / hands as f64),
        "unit":"bb/100; intervals are 95% t-intervals over seed blocks (seat rotations of a seed are averaged)",
        "style":style,"model_style_by_condition":model_style,"win_rates":win_rates,"paired_differences":paired}),
    )
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
        }
    }
    out
}
