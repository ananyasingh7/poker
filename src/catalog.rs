// ABOUTME: One CSV row per run or batch under a directory tree: what was run (model, thinking, help,
// ABOUTME: seeds), how much of it completed, and what it cost. Reads manifests and summaries only.
//!
//! `poker-lab catalog runs > results/runs.csv`. A batch directory (one with `batch-manifest.json`)
//! is one row; a single run (`manifest.json` outside a batch) is one row. Superseded attempts
//! inside a batch are skipped. Every field is read loosely, so older run formats still list.

use serde_json::Value;
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const HEADER: &str = "path,date,kind,model,thinking,conditions,scaffolds,sizing,seeds,model_seats,hands_per_session,sessions,sessions_scheduled,hands,requests,cost_usd,stop_reason";

fn read(path: &Path) -> Option<Value> {
    serde_json::from_reader(File::open(path).ok()?).ok()
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The local date a file was last written, as YYYY-MM-DD (the dates the docs use).
fn date(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(String::new(), |d| {
            let secs = d.as_secs() as i64;
            civil((secs + utc_offset(secs)).div_euclid(86_400))
        })
}

/// The local time zone's offset from UTC at `secs`, in seconds.
fn utc_offset(secs: i64) -> i64 {
    let time = secs as libc::time_t;
    // SAFETY: localtime_r writes only into the zeroed `tm` it is given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return 0;
    }
    tm.tm_gmtoff as i64
}

/// "off", "low", "on" or "" for a pilot config.
fn thinking(pilot: &Value) -> String {
    match (
        pilot["reasoning_enabled"].as_bool(),
        pilot["reasoning_effort"].as_str(),
    ) {
        (_, Some(effort)) => effort.to_owned(),
        (Some(true), None) => "on".into(),
        (Some(false), None) => "off".into(),
        (None, None) => String::new(),
    }
}

fn sizing(pilot: &Value) -> String {
    let mut parts = vec![];
    for (key, name) in [
        ("sizing_hint", "hint"),
        ("sharp_sizing", "sharp"),
        ("size_menu", "menu"),
    ] {
        if pilot[key] == true {
            parts.push(name);
        }
    }
    parts.join("+")
}

fn list(value: &Value) -> String {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|v| v.as_str().map_or(v.to_string(), str::to_owned))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// "1-10" for a contiguous list, otherwise every seed.
fn seeds(value: &Value) -> String {
    let Some(seeds) = value.as_array() else {
        return String::new();
    };
    let numbers: Vec<u64> = seeds.iter().filter_map(Value::as_u64).collect();
    match (numbers.first(), numbers.last()) {
        (Some(first), Some(last))
            if numbers.windows(2).all(|w| w[1] == w[0] + 1) && numbers.len() > 1 =>
        {
            format!("{first}-{last}")
        }
        _ => list(value),
    }
}

fn model(pilot: &Value) -> String {
    let models = list(&pilot["models"]);
    if pilot["all_models"] == true && !models.is_empty() {
        models
    } else {
        pilot["model"].as_str().unwrap_or("").to_owned()
    }
}

/// Quotes a CSV field when it needs it.
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

fn batch_row(dir: &Path, root: &Path) -> Option<String> {
    let manifest = read(&dir.join("batch-manifest.json"))?;
    let pilot = &manifest["pilot"];
    let summary = read(&dir.join("batch-summary.json")).unwrap_or(Value::Null);
    let hands: u64 = summary["sessions"]
        .as_array()
        .map(|s| s.iter().filter_map(|x| x["hands"].as_u64()).sum())
        .unwrap_or(0);
    let cost = summary["reported_cost_usd"].as_f64();
    Some(row(&[
        relative(dir, root),
        date(&dir.join("batch-manifest.json")),
        "batch".into(),
        model(pilot),
        thinking(pilot),
        list(&manifest["conditions"]),
        match list(&manifest["scaffolds"]) {
            s if s.is_empty() => pilot["scaffold"].as_str().unwrap_or("").to_owned(),
            s => s,
        },
        sizing(pilot),
        seeds(&manifest["seeds"]),
        list(&manifest["model_seats"]),
        pilot["hands"].to_string(),
        summary["completed_sessions"].to_string(),
        summary["scheduled_sessions"].to_string(),
        hands.to_string(),
        summary["requests"].to_string(),
        cost.map_or(String::new(), |c| format!("{c:.4}")),
        summary["stop_reason"].as_str().unwrap_or("").to_owned(),
    ]))
}

fn run_row(dir: &Path, root: &Path) -> Option<String> {
    let manifest = read(&dir.join("manifest.json"))?;
    let config = &manifest["config"];
    let summary = read(&dir.join("summary.json")).unwrap_or(Value::Null);
    let stats = &summary["stats"];
    Some(row(&[
        relative(dir, root),
        date(&dir.join("manifest.json")),
        "run".into(),
        model(config),
        thinking(config),
        config["memory"].as_str().unwrap_or("").to_owned(),
        config["scaffold"].as_str().unwrap_or("").to_owned(),
        sizing(config),
        config["seed"].to_string(),
        list(&manifest["llm_seats"]),
        config["hands"].to_string(),
        "1".into(),
        "1".into(),
        summary["completed_hands"].to_string(),
        stats["requests"].to_string(),
        stats["reported_cost_usd"]
            .as_f64()
            .map_or(String::new(), |c| format!("{c:.4}")),
        stats["halt"]
            .as_str()
            .or(stats["stop_reason"].as_str())
            .unwrap_or("")
            .to_owned(),
    ]))
}

fn relative(dir: &Path, root: &Path) -> String {
    let parent = root.parent().unwrap_or(root);
    dir.strip_prefix(parent)
        .unwrap_or(dir)
        .display()
        .to_string()
}

fn row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| field(if f == "null" { "" } else { f }))
        .collect::<Vec<_>>()
        .join(",")
}

/// Every run and batch under `root`, sorted by path, as CSV lines (header first).
pub fn catalog(root: &Path) -> Result<Vec<String>, String> {
    let mut dirs: Vec<PathBuf> = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir.join("batch-manifest.json").exists() {
            dirs.push(dir); // its sessions are part of this row
            continue;
        }
        if dir.join("manifest.json").exists() {
            dirs.push(dir);
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() && path.file_name().is_some_and(|n| n != "superseded") {
                stack.push(path);
            }
        }
    }
    dirs.sort();
    let mut lines = vec![HEADER.to_owned()];
    for dir in dirs {
        let line = if dir.join("batch-manifest.json").exists() {
            batch_row(&dir, root)
        } else {
            run_row(&dir, root)
        };
        if let Some(line) = line {
            lines.push(line);
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dates_and_fields() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_774), "2026-11-17");
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(seeds(&json!([11, 12, 13])), "11-13");
        assert_eq!(seeds(&json!([42, 314])), "42 314");
        assert_eq!(
            thinking(&json!({"reasoning_enabled":true,"reasoning_effort":"low"})),
            "low"
        );
        assert_eq!(sizing(&json!({"sharp_sizing":true})), "sharp");
    }

    #[test]
    fn lists_a_batch_and_a_single_run_but_not_their_sessions() {
        let root = std::env::temp_dir().join(format!("catalog-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let batch = root.join("topic/batch-v1");
        std::fs::create_dir_all(batch.join("session-001")).unwrap();
        std::fs::write(
            batch.join("batch-manifest.json"),
            json!({"pilot":{"model":"m/x","hands":60,"reasoning_enabled":false},"seeds":[1,2],"model_seats":[0],"conditions":["none"]}).to_string(),
        )
        .unwrap();
        std::fs::write(batch.join("session-001/manifest.json"), "{}").unwrap();
        std::fs::write(
            batch.join("batch-summary.json"),
            json!({"completed_sessions":2,"scheduled_sessions":2,"requests":10,"reported_cost_usd":0.5,"sessions":[{"hands":60},{"hands":60}]}).to_string(),
        )
        .unwrap();
        let run = root.join("topic/run-v1");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(
            run.join("manifest.json"),
            json!({"config":{"model":"m/y","hands":10,"seed":42,"memory":"notes"},"llm_seats":[0]})
                .to_string(),
        )
        .unwrap();
        let lines = catalog(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[1].contains("topic/batch-v1"), "{}", lines[1]);
        assert!(lines[1].contains(",batch,m/x,off,none,"), "{}", lines[1]);
        assert!(
            lines[1].contains(",1-2,0,60,2,2,120,10,0.5000,"),
            "{}",
            lines[1]
        );
        assert!(lines[2].contains(",run,m/y,,notes,"), "{}", lines[2]);
    }
}
