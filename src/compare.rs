// ABOUTME: Side-by-side comparison of several run directories (different models, help or thinking
// ABOUTME: settings): one row per arm, and paired differences on the deals the arms share.
//!
//! `poker-lab compare DIR... [--seat N] [--turn] [--json]`. Each directory is analysed as `analyze` does;
//! every model seat label becomes an arm named `model · thinking · condition`, so runs of
//! different models read apart at a glance. Only complete seeds count (every seat-hand the batch
//! scheduled), so a session cut short by its spend cap never enters a comparison. Arms with the
//! same name from different directories (the same treatment run on different seeds) are merged.
//!
//! Two kinds of paired difference, each over the seeds both arms completed:
//! - help: every condition minus the same model's `none` arm (same thinking setting);
//! - model: every arm minus the first directory's model in the same condition.

use crate::analyze::{self, interval};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write as _, fs::File, path::Path};

/// Per-seed values of one arm, bb/100: chips, all-in adjusted, river cost, turn cost.
type Seeds = BTreeMap<u64, [Option<f64>; 4]>;
const MEASURES: [&str; 4] = ["bb_per_100", "adjusted", "river_cost", "turn_cost"];

struct Arm {
    name: String,
    model: String,
    thinking: String,
    condition: String,
    /// "" for the fixed field, else the variation (see `bots_label`).
    bots: String,
    directories: Vec<String>,
    seeds: Seeds,
    /// Model seat-hands behind the descriptive columns below (all sessions, complete or not).
    hands: u64,
    /// Sums over those hands, turned into rates when reported.
    cost_usd: f64,
    fallbacks: f64,
    vpip_hands: f64,
    river_graded: u64,
    turn_graded: u64,
    truncations: u64,
    providers: Vec<String>,
}
impl Arm {
    /// Folds in the same treatment run in another directory: seeds already present are kept.
    fn merge(&mut self, other: Arm) {
        for (seed, values) in other.seeds {
            self.seeds.entry(seed).or_insert(values);
        }
        self.directories.extend(other.directories);
        self.hands += other.hands;
        self.cost_usd += other.cost_usd;
        self.fallbacks += other.fallbacks;
        self.vpip_hands += other.vpip_hands;
        self.river_graded += other.river_graded;
        self.turn_graded += other.turn_graded;
        self.truncations += other.truncations;
        for p in other.providers {
            if !self.providers.contains(&p) {
                self.providers.push(p);
            }
        }
    }
    fn rate(&self, sum: f64) -> Option<f64> {
        (self.hands > 0).then(|| sum / self.hands as f64)
    }
}

/// "think off", "think low", "think on", or "think ?" for a pilot config.
fn thinking(pilot: &Value) -> String {
    match (
        pilot["reasoning_enabled"].as_bool(),
        pilot["reasoning_effort"].as_str(),
    ) {
        (_, Some(effort)) => format!("think {effort}"),
        (Some(true), None) => "think on".into(),
        (Some(false), None) => "think off".into(),
        (None, None) => "think ?".into(),
    }
}

fn arms(dir: &Path, turn: bool, seat: Option<usize>) -> Result<Vec<Arm>, String> {
    let manifest: Value = File::open(dir.join("batch-manifest.json"))
        .map_err(|e| format!("{}: {e} (compare needs batch directories)", dir.display()))
        .and_then(|f| serde_json::from_reader(f).map_err(|e| e.to_string()))?;
    let pilot = &manifest["pilot"];
    let rotations = match seat {
        Some(_) => 1,
        None => manifest["model_seats"]
            .as_array()
            .map_or(1, |s| s.len() as u64),
    };
    let full_hands = pilot["hands"].as_u64().unwrap_or(0) * rotations;
    let analysis = analyze::directory_for_seat(dir, true, turn, seat)?;
    let providers = pilot["provider"]["order"]
        .as_array()
        .map(|o| {
            o.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("/")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "router".into());
    let style = |label: &str, condition: &str| -> Option<&Value> {
        analysis["model_style_by_condition"]
            .as_array()?
            .iter()
            .find(|s| s["label"] == label && s["condition"] == condition)
    };
    let mut out = vec![];
    for row in analysis["win_rates"].as_array().into_iter().flatten() {
        let (Some(label), Some(condition)) = (row["label"].as_str(), row["condition"].as_str())
        else {
            continue;
        };
        let Some(style) = style(label, condition) else {
            continue; // a scripted seat
        };
        let mut seeds = Seeds::new();
        for (seed, v) in row["per_seed"].as_object().into_iter().flatten() {
            if v["hands"].as_u64().unwrap_or(0) < full_hands {
                continue;
            }
            let seed: u64 = seed.parse().map_err(|_| "bad seed key")?;
            seeds.insert(seed, MEASURES.map(|m| v[m].as_f64()));
        }
        let model = label.rsplit('/').next().unwrap_or(label).to_owned();
        let thinking = thinking(pilot);
        // Runs against less predictable bots are a different environment: named apart and never
        // merged or paired with fixed-bot runs of the same treatment.
        let bots = bots_label(&pilot["opponent_variation"]);
        let hands = style["seat_hands"].as_u64().unwrap_or(0);
        // VPIP is a rate in the style row: turn it back into a sum so merged arms weight each
        // run by its size.
        let per_hand = |key: &str| style[key].as_f64().unwrap_or(0.0) * hands as f64;
        out.push(Arm {
            name: match bots.as_str() {
                "" => format!("{model} · {thinking} · {condition}"),
                b => format!("{model} · {thinking} · {condition} · {b} bots"),
            },
            model,
            thinking,
            condition: condition.to_owned(),
            bots,
            directories: vec![dir.display().to_string()],
            seeds,
            hands,
            cost_usd: analysis["cost_per_hand_usd"].as_f64().unwrap_or(0.0) * hands as f64,
            fallbacks: style["fallbacks"].as_f64().unwrap_or(0.0),
            vpip_hands: per_hand("vpip"),
            river_graded: style["river_graded"].as_u64().unwrap_or(0),
            turn_graded: style["turn_graded"].as_u64().unwrap_or(0),
            truncations: style["truncations"].as_u64().unwrap_or(0),
            providers: vec![providers.clone()],
        });
    }
    Ok(out)
}

/// "varied" style label of a pilot's `opponent_variation`, or "" for the fixed field.
pub fn bots_label(variation: &Value) -> String {
    let mut parts = vec![];
    if variation["personalities"] == true {
        parts.push("personalities".to_owned());
    }
    if let Some(m) = variation["mixing"].as_f64().filter(|m| *m > 0.0) {
        parts.push(format!("mix {m}"));
    }
    if variation["random_field"] == true {
        parts.push("random field".to_owned());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("varied ({})", parts.join(", "))
    }
}

fn measure(arm: &Arm, m: usize) -> Value {
    let values: Vec<f64> = arm.seeds.values().filter_map(|v| v[m]).collect();
    interval(&values)
}

fn paired(a: &Arm, b: &Arm) -> Value {
    let mut row = json!({"arm":a.name,"versus":b.name});
    let shared: Vec<u64> = a
        .seeds
        .keys()
        .filter(|s| b.seeds.contains_key(s))
        .copied()
        .collect();
    row["seeds"] = json!(shared);
    for (m, name) in MEASURES.iter().enumerate() {
        let diffs: Vec<f64> = shared
            .iter()
            .filter_map(|s| Some(a.seeds[s][m]? - b.seeds[s][m]?))
            .collect();
        row[*name] = interval(&diffs);
    }
    row
}

/// Compares the batch directories `dirs`; the first directory's model is the reference. With
/// `seat`, only sessions in which the model played that seat count.
pub fn compare(dirs: &[&Path], turn: bool, seat: Option<usize>) -> Result<Value, String> {
    let mut all: Vec<Arm> = vec![];
    for dir in dirs {
        for arm in arms(dir, turn, seat)? {
            match all.iter_mut().find(|a| a.name == arm.name) {
                Some(same) => same.merge(arm),
                None => all.push(arm),
            }
        }
    }
    let rows: Vec<Value> = all
        .iter()
        .map(|a| {
            // Rates per hand, comparable across models that play different numbers of hands.
            let mut row = json!({"arm":a.name,"model":a.model,"thinking":a.thinking,"bots":a.bots,
                "condition":a.condition,"directories":a.directories,"providers":a.providers.join(", "),
                "complete_seeds":a.seeds.keys().collect::<Vec<_>>(),"seat_hands":a.hands,
                "cost_per_hand_usd":a.rate(a.cost_usd),"vpip":a.rate(a.vpip_hands),
                "fallbacks_per_100_hands":a.rate(a.fallbacks * 100.0),
                "river_decisions_per_100_hands":a.rate(a.river_graded as f64 * 100.0),
                "turn_decisions_per_100_hands":a.rate(a.turn_graded as f64 * 100.0),
                "truncations":a.truncations,
                "per_seed":a.seeds.iter().map(|(s, v)| (s.to_string(), json!(MEASURES.iter().zip(v).map(|(m, x)| (m.to_string(), json!(x))).collect::<serde_json::Map<_, _>>()))).collect::<serde_json::Map<_, _>>()});
            for (m, name) in MEASURES.iter().enumerate() {
                row[*name] = measure(a, m);
            }
            row
        })
        .collect();
    let mut help = vec![];
    for a in all.iter().filter(|a| a.condition != "none") {
        if let Some(base) = all.iter().find(|b| {
            b.condition == "none"
                && b.model == a.model
                && b.thinking == a.thinking
                && b.bots == a.bots
        }) {
            help.push(paired(a, base));
        }
    }
    let mut model = vec![];
    if let Some(first) = all.first() {
        let reference = (first.model.clone(), first.thinking.clone());
        for a in all
            .iter()
            .filter(|a| (a.model.clone(), a.thinking.clone()) != reference)
        {
            if let Some(base) = all.iter().find(|b| {
                b.model == reference.0
                    && b.thinking == reference.1
                    && b.condition == a.condition
                    && b.bots == a.bots
            }) {
                model.push(paired(a, base));
            }
        }
    }
    Ok(
        json!({"unit":"bb/100; 95% t-intervals over complete seeds; cost columns: chips given up on graded decisions (lower is better)",
        "turn_graded":turn,"seat":seat,"arms":rows,"help_effects":help,"model_differences":model}),
    )
}

fn cell(v: &Value) -> String {
    match (v["mean"].as_f64(), v["ci95"].as_array()) {
        (Some(mean), Some(ci)) => format!(
            "{mean:+.0} [{:+.0}, {:+.0}]",
            ci[0].as_f64().unwrap_or(0.0),
            ci[1].as_f64().unwrap_or(0.0)
        ),
        (Some(mean), None) => format!("{mean:+.0}"),
        _ => "-".into(),
    }
}
fn plain(v: &Value) -> String {
    v["mean"].as_f64().map_or("-".into(), |m| format!("{m:.0}"))
}

/// Markdown tables of a `compare` result.
pub fn render(c: &Value) -> String {
    let turn = c["turn_graded"] == true;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Arms (bb/100; chips and luck-adjusted with 95% intervals over complete seeds; costs are chips given up, lower is better)\n"
    );
    let _ = writeln!(
        out,
        "| Arm | Seeds | Chips | Luck-adjusted | River cost | River decisions /100 hands | {}VPIP | $/hand | Fallbacks /100 hands | Truncated | Providers |",
        if turn { "Turn cost (approx) | " } else { "" }
    );
    let _ = writeln!(
        out,
        "| --- | --- | --- | --- | --- | --- | {}--- | --- | --- | --- | --- |",
        if turn { "--- | " } else { "" }
    );
    let number =
        |v: &Value, places: usize| v.as_f64().map_or("-".into(), |x| format!("{x:.places$}"));
    for a in c["arms"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {}{} | {} | {} | {} | {} |",
            a["arm"].as_str().unwrap_or(""),
            a["complete_seeds"].as_array().map_or(0, Vec::len),
            cell(&a["bb_per_100"]),
            cell(&a["adjusted"]),
            plain(&a["river_cost"]),
            number(&a["river_decisions_per_100_hands"], 0),
            if turn {
                format!("{} | ", plain(&a["turn_cost"]))
            } else {
                String::new()
            },
            a["vpip"]
                .as_f64()
                .map_or("-".into(), |x| format!("{:.0}%", x * 100.0)),
            number(&a["cost_per_hand_usd"], 5),
            number(&a["fallbacks_per_100_hands"], 1),
            a["truncations"],
            a["providers"].as_str().unwrap_or("")
        );
    }
    let _ = writeln!(
        out,
        "\nCosts are per 100 hands played, so a model that reaches the river less often shows a lower river cost: read them beside the river decisions and VPIP columns."
    );
    for (key, title) in [
        (
            "help_effects",
            "Help effects: each arm minus the same model's no-help arm, paired on shared seeds",
        ),
        (
            "model_differences",
            "Model differences: each arm minus the first model in the same condition, paired on shared seeds",
        ),
    ] {
        let rows = c[key].as_array().cloned().unwrap_or_default();
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n{title}\n");
        let _ = writeln!(
            out,
            "| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | {}",
            if turn { "Turn cost |" } else { "" }
        );
        let _ = writeln!(
            out,
            "| --- | --- | --- | --- | --- | --- | {}",
            if turn { "--- |" } else { "" }
        );
        for r in rows {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {}",
                r["arm"].as_str().unwrap_or(""),
                r["versus"].as_str().unwrap_or(""),
                r["seeds"].as_array().map_or(0, Vec::len),
                cell(&r["bb_per_100"]),
                cell(&r["adjusted"]),
                cell(&r["river_cost"]),
                if turn {
                    format!("{} |", cell(&r["turn_cost"]))
                } else {
                    String::new()
                }
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm(model: &str, condition: &str, seeds: &[(u64, f64)]) -> Arm {
        Arm {
            name: format!("{model} · think low · {condition}"),
            model: model.into(),
            thinking: "think low".into(),
            condition: condition.into(),
            bots: String::new(),
            directories: vec![],
            seeds: seeds
                .iter()
                .map(|(s, v)| (*s, [Some(*v), Some(*v), Some(*v), None]))
                .collect(),
            hands: 60,
            cost_usd: 0.0,
            fallbacks: 0.0,
            vpip_hands: 0.0,
            river_graded: 0,
            turn_graded: 0,
            truncations: 0,
            providers: vec![],
        }
    }

    #[test]
    fn same_treatment_on_other_seeds_merges() {
        let mut a = arm("m", "none", &[(1, 10.0), (2, 20.0)]);
        a.fallbacks = 3.0;
        let mut b = arm("m", "none", &[(2, 99.0), (11, 30.0)]);
        b.fallbacks = 1.0;
        a.merge(b);
        assert_eq!(a.seeds.keys().copied().collect::<Vec<_>>(), [1, 2, 11]);
        assert_eq!(a.seeds[&2][0], Some(20.0), "the first run's seed is kept");
        assert_eq!(a.hands, 120);
        assert_eq!(a.rate(a.fallbacks * 100.0), Some(4.0 / 120.0 * 100.0));
    }

    #[test]
    fn pairs_only_on_shared_seeds() {
        let a = arm("m", "stats", &[(1, 10.0), (2, 20.0), (3, 99.0)]);
        let b = arm("m", "none", &[(1, 4.0), (2, 8.0), (4, 0.0)]);
        let row = paired(&a, &b);
        assert_eq!(row["seeds"], json!([1, 2]));
        assert_eq!(row["bb_per_100"]["mean"], json!(9.0));
        assert_eq!(row["turn_cost"]["blocks"], json!(0));
    }

    #[test]
    fn thinking_labels() {
        assert_eq!(
            thinking(&json!({"reasoning_enabled":true,"reasoning_effort":"low"})),
            "think low"
        );
        assert_eq!(thinking(&json!({"reasoning_enabled":false})), "think off");
    }
}
