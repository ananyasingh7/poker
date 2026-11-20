// ABOUTME: Runs model sessions and batches: builds the table on either backend, writes the audit
// ABOUTME: artifacts, and schedules independent sessions concurrently under one shared budget.
use crate::{
    arena::{Agent, Scripted, Session},
    cards::Rng,
    experiment::Batch,
    memory::Mode,
    openrouter::{Config, Execution, GlobalBudget, OpenRouterAgent, Opponents, Stats},
    participant::{self, Policy},
    private_fs, report,
    supervisor::{self, Deadlines, Launch, Options, Supervisor},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// Everything a run needs that is not experiment configuration.
#[derive(Clone)]
pub struct Env {
    pub key: String,
    /// Provider override for tests; the CLI always uses the real endpoint.
    pub endpoint: Option<String>,
    /// Worker executable; `None` means this executable's `participant` entry point.
    pub launch: Option<Launch>,
    /// Suppress per-hand console lines (used when sessions run concurrently).
    pub quiet: bool,
    /// Batch-wide budget shared by concurrent sessions.
    pub global: GlobalBudget,
}
impl Env {
    pub fn for_config(config: &Config) -> Result<Self, String> {
        Ok(Self::new(if config.local_port.is_some() {
            String::new()
        } else {
            load_key()?
        }))
    }

    pub fn new(key: String) -> Self {
        Self {
            key,
            endpoint: None,
            launch: None,
            quiet: false,
            global: GlobalBudget::default(),
        }
    }
}

/// Reads the credential from `OPENROUTER_API_KEY`, else the Git-ignored local key file.
pub fn load_key() -> Result<String, String> {
    let key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) => key,
        Err(std::env::VarError::NotPresent) => std::fs::read_to_string(".openrouter-api-key")
            .map_err(|_| {
                "set OPENROUTER_API_KEY or provide .openrouter-api-key in the working directory"
            })?
            .trim()
            .to_owned(),
        Err(_) => return Err("OPENROUTER_API_KEY is not valid Unicode".into()),
    };
    if key.trim().is_empty() {
        return Err("OPENROUTER_API_KEY is empty".into());
    }
    Ok(key)
}

fn create(path: impl AsRef<Path>) -> Result<BufWriter<File>, String> {
    let path = path.as_ref();
    private_fs::create_file(path)
        .map(BufWriter::new)
        .map_err(|e| format!("{}: {e}", path.display()))
}
fn line(file: &mut impl Write, value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *file, value).map_err(|e| e.to_string())?;
    writeln!(file)
        .and_then(|_| file.flush())
        .map_err(|e| e.to_string())
}

/// Plays one session into a new private directory and returns its summary.
pub fn session(config: Config, dir: &Path, env: &Env) -> Result<Value, String> {
    config.validate()?;
    let subprocess = config.execution == Execution::Subprocess;
    private_fs::create_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let styles = config.seat_styles();
    let bots = config.seat_bots();
    let labels = config.seat_labels();
    let mut manifest = create(dir.join("manifest.json"))?;
    serde_json::to_writer_pretty(
        &mut manifest,
        &json!({"version":1,"crate_version":env!("CARGO_PKG_VERSION"),"config":config,
        "execution":config.execution,"participant_protocol":subprocess.then_some(participant::VERSION),
        "participant_deadlines":subprocess.then(Deadlines::default),
        "memory":config.memory,"scaffold":config.scaffold,"condition":config.condition(),"llm_seats":config.llm_seats(),"seat_labels":labels,
        "opponents":if config.all_models{None}else{Some(config.opponents)},"opponent_version":2,
        "seat_styles":matches!(config.opponents,Opponents::Mixed).then_some(&styles),
        "seat_bots":config.opponent_variation.is_active().then_some(&bots),
        "rules":{"small_blind":1,"big_blind":2},
        "rebuy":if config.reset_stacks {"reset_every_hand_100bb"} else {"busted_only_100bb"}}),
    )
    .map_err(|e| e.to_string())?;
    manifest.flush().map_err(|e| e.to_string())?;
    let stats = Rc::new(RefCell::new(Stats {
        global: env.global.clone(),
        ..Stats::default()
    }));
    let mut agents: Vec<Box<dyn Agent>> = (0..config.seats)
        .map(|seat| match bots[seat] {
            Some(bot) => bot.agent(),
            None => Box::new(Scripted::Random(Rng(config.seed.wrapping_add(seat as u64)))),
        })
        .collect();
    let llm_seats = config.llm_seats();
    // Subprocess backend: every model and fixed-policy seat becomes a supervised worker, all
    // initialized before the first hand. Seeded random fixtures stay in-process because their
    // generator is derived from the session seed, which must never be sent to a worker.
    let mut supervisor = None;
    if subprocess {
        let mut started = Supervisor::start(Options {
            deadlines: Deadlines::default(),
            run_dir: Some(dir.to_path_buf()),
            max_requests: config.max_requests,
            spend_limit_usd: config.spend_limit_usd,
            key: Some(env.key.clone()),
            stats: stats.clone(),
        })?;
        for (seat, agent) in agents.iter_mut().enumerate() {
            let policy = if llm_seats.contains(&seat) {
                let mut settings = config.model_settings(seat);
                settings.endpoint = env.endpoint.clone();
                Policy::OpenRouter(settings)
            } else if let Some(style) = styles[seat] {
                style.policy()
            } else {
                continue;
            };
            let launch = match &env.launch {
                Some(launch) => launch.clone(),
                None => Launch::current_exe()?,
            };
            *agent = Box::new(started.spawn(seat, policy, launch)?);
        }
        supervisor = Some(started);
    } else {
        for &seat in &llm_seats {
            let name = if config.all_models {
                format!("model-seat-{seat}-private.jsonl")
            } else {
                "model-private.jsonl".into()
            };
            let mut seat_config = config.clone();
            seat_config.model = config.model_for(seat).to_owned();
            let mut model = OpenRouterAgent::with_endpoint(
                seat_config,
                env.key.clone(),
                Box::new(create(dir.join(name))?),
                env.endpoint.clone(),
            )?;
            model.stats = stats.clone();
            if config.memory == Mode::Oracle {
                model.set_notebook(config.oracle_notes());
            }
            agents[seat] = Box::new(model);
        }
    }
    let mut public = create(dir.join("public.jsonl"))?;
    let mut evaluator = create(dir.join("evaluator.jsonl"))?;
    let mut private = (0..config.seats)
        .map(|i| create(dir.join(format!("seat-{i}.jsonl"))))
        .collect::<Result<Vec<_>, _>>()?;
    let mut summary = create(dir.join("summary.json"))?;
    let mut history = create(dir.join("hand-history-private.txt"))?;
    let mut table = Session::new(config.seats, config.seed)?;
    table.reset_stacks = config.reset_stacks;
    let mut completed = 0;
    let mut lifecycle_errors = vec![];
    let mut net = vec![0i64; config.seats];
    let mut outage = false;
    for index in 0..config.hands {
        let played = match table.next_hand(&mut agents) {
            Ok(played) => played,
            // Infrastructure outlasted every retry. The unfinished hand is dropped, not played
            // with a fallback; the session ends incomplete and can be re-run.
            Err(e) if e == crate::arena::OUTAGE => {
                outage = true;
                if !env.quiet {
                    println!("Provider outage during hand {}: hand abandoned", index + 1);
                }
                break;
            }
            Err(e) => return Err(e),
        };
        completed += 1;
        for error in &played.lifecycle_errors {
            lifecycle_errors.push(
                json!({"hand":index + 1,"seat":error.seat,"phase":error.phase,"error":error.error}),
            );
        }
        let record = &played.record;
        for (seat, total) in net.iter_mut().enumerate() {
            *total += record.final_stacks[seat] as i64 - record.initial_stacks[seat] as i64;
        }
        write!(
            history,
            "{}",
            report::hand(record, index + 1, Some(config.model_seat))?
        )
        .and_then(|_| history.flush())
        .map_err(|e| e.to_string())?;
        line(&mut public, &record.events)?;
        line(&mut evaluator, record)?;
        for (file, views) in private.iter_mut().zip(&played.private_views) {
            line(file, views)?;
        }
        let stats = stats.borrow();
        if !env.quiet {
            println!(
                "Hand {} stacks={:?} requests={} reported_cost=${:.6}",
                index + 1,
                table.stacks,
                stats.requests,
                stats.reported_cost_usd
            );
        }
        if let Some(reason) = stats.halt_reason.as_deref() {
            if !env.quiet {
                println!(
                    "Stopped after completing current hand with fallback where needed: {reason}"
                );
            }
            break;
        }
    }
    // Workers are shut down before the summary so it carries their exit status. A worker that
    // cannot be reaped is recorded and then aborts the run.
    let cleanup_error = supervisor.as_mut().and_then(|s| s.shutdown().err());
    let contaminated = supervisor.as_ref().is_some_and(|s| s.contaminated());
    let clean = {
        let stats = stats.borrow();
        stats.fallbacks == 0
            && stats.memory_errors == 0
            && stats.halt_reason.is_none()
            && completed == config.hands
            && lifecycle_errors.is_empty()
            && !contaminated
            && cleanup_error.is_none()
    };
    let result = json!({"completed_hands":completed,"requested_hands":config.hands,"llm_seats":llm_seats,
        "seat_labels":labels,"stats":*stats.borrow(),"final_stacks":table.stacks,"rebuys":table.rebuys,
        "net_chips":net,"outage":outage,"pilot_only":true,"execution":config.execution,"clean":clean,
        "lifecycle_errors":lifecycle_errors,"cleanup_error":cleanup_error,
        "participants":supervisor.as_ref().map(|s| s.report())});
    serde_json::to_writer_pretty(&mut summary, &result).map_err(|e| e.to_string())?;
    summary.flush().map_err(|e| e.to_string())?;
    if let Some(error) = cleanup_error {
        return Err(format!("worker cleanup failed: {error}"));
    }
    if !env.quiet {
        println!("Pilot complete: {completed} hands; net chips by seat: {net:?}");
    }
    Ok(result)
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex.lock().map_err(|_| "batch state poisoned".to_owned())
}

/// Runs a batch schedule, `batch.parallel` sessions at a time, under one shared budget.
/// Sessions are independent: each has its own directory, agents, and (for the subprocess
/// backend) its own workers. A halt in any session stops new sessions from starting.
pub fn batch(batch: &Batch, root: &Path, env: &Env) -> Result<Value, String> {
    run_batch(batch, root, env, false)
}

/// Continues an interrupted batch in `root`. Sessions that completed every requested hand are
/// kept, whatever their outcome; incomplete ones are moved to `superseded/` and played again
/// on the same seeds. The batch file must be unchanged, and earlier spend counts against the cap.
pub fn resume(batch: &Batch, root: &Path, env: &Env) -> Result<Value, String> {
    run_batch(batch, root, env, true)
}

const OPERATIONAL: &[&str] = &[
    "outage_pause_ms",
    "outage_max_wait_s",
    "max_requests",
    "spend_limit_usd",
    "parallel",
    "unknown_cost_allowance",
    "unknown_cost_charge_usd",
];

/// Rows of sessions that finished all their hands in an earlier run of this batch.
fn completed_rows(batch: &Batch, root: &Path) -> Result<Vec<Value>, String> {
    let read =
        |name: &str| std::fs::read_to_string(root.join(name)).map_err(|e| format!("{name}: {e}"));
    // Parsed through `Batch` so fields added since the manifest was written take defaults.
    let recorded: Batch = serde_json::from_str(&read("batch-manifest.json")?)
        .map_err(|e| format!("batch-manifest.json: {e}"))?;
    let recorded = serde_json::to_value(recorded).map_err(|e| e.to_string())?;
    // Only the experimental design must match; budget, concurrency and the unknown-cost
    // allowance are operational and may change between attempts.
    let design = |mut value: Value| {
        for key in OPERATIONAL {
            value.as_object_mut().map(|o| o.remove(*key));
        }
        value
    };
    if design(recorded) != design(serde_json::to_value(batch).map_err(|e| e.to_string())?) {
        return Err("batch design differs from the recorded batch-manifest.json".into());
    }
    let mut kept = vec![];
    for text in read("sessions.jsonl")?.lines() {
        let row: Value = serde_json::from_str(text).map_err(|e| format!("sessions.jsonl: {e}"))?;
        let result = &row["result"];
        let index = row["session"].as_u64().unwrap_or(0);
        let summary = root.join(format!("session-{index:03}/summary.json"));
        if result["completed_hands"] == result["requested_hands"] && summary.exists() {
            kept.push(row);
        }
    }
    Ok(kept)
}

/// Batch-wide pause after the provider outlasts a session's retries: no new session starts
/// until the pause ends, and repeated outages double it. One shared pause, not one per session.
struct Breaker {
    state: Mutex<BreakerState>,
    pause: Duration,
    max_wait: Duration,
}
#[derive(Default)]
struct BreakerState {
    consecutive: u32,
    until: Option<Instant>,
    waited: Duration,
}
impl Breaker {
    fn new(batch: &Batch) -> Self {
        Self {
            state: Mutex::default(),
            pause: Duration::from_millis(batch.outage_pause_ms),
            max_wait: Duration::from_secs(batch.outage_max_wait_s),
        }
    }
    fn wait(&self) {
        loop {
            let until = lock(&self.state).ok().and_then(|s| s.until);
            match until {
                Some(t) if t > Instant::now() => std::thread::sleep(t - Instant::now()),
                _ => return,
            }
        }
    }
    /// Records an outage; false once the batch has waited longer than its limit.
    fn trip(&self) -> bool {
        let Ok(mut state) = lock(&self.state) else {
            return false;
        };
        // Sessions that fail together share the pause already in effect.
        if state.until.is_some_and(|t| t > Instant::now()) {
            return true;
        }
        state.consecutive += 1;
        let pause =
            (self.pause * 2u32.pow((state.consecutive - 1).min(5))).min(Duration::from_secs(600));
        state.waited += pause;
        state.until = Some(Instant::now() + pause);
        state.waited <= self.max_wait
    }
    fn healthy(&self) {
        if let Ok(mut state) = lock(&self.state) {
            state.consecutive = 0;
        }
    }
}

/// The batch's `superseded/` directory, created on first use.
fn superseded_dir(root: &Path) -> Result<std::path::PathBuf, String> {
    let dir = root.join("superseded");
    if !dir.exists() {
        private_fs::create_dir(&dir).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn run_batch(batch: &Batch, root: &Path, env: &Env, resume: bool) -> Result<Value, String> {
    let schedule = batch.schedule()?;
    let global = GlobalBudget::new(batch.max_requests, batch.spend_limit_usd)
        .with_unknown_allowance(batch.unknown_cost_allowance, batch.unknown_cost_charge_usd);
    let mut done = std::collections::BTreeSet::new();
    let mut kept_rows = vec![];
    if resume {
        kept_rows = completed_rows(batch, root)?;
        let superseded = superseded_dir(root)?;
        let attempt = (1..)
            .find(|n| !superseded.join(format!("sessions-{n}.jsonl")).exists())
            .expect("unbounded range");
        let move_aside = |from: std::path::PathBuf, name: String| -> Result<(), String> {
            std::fs::rename(&from, superseded.join(name))
                .map_err(|e| format!("{}: {e}", from.display()))
        };
        move_aside(
            root.join("sessions.jsonl"),
            format!("sessions-{attempt}.jsonl"),
        )?;
        let mut manifest = create(superseded.join(format!("resume-{attempt}-batch.json")))?;
        serde_json::to_writer_pretty(&mut manifest, batch).map_err(|e| e.to_string())?;
        manifest.flush().map_err(|e| e.to_string())?;
        if root.join("batch-summary.json").exists() {
            move_aside(
                root.join("batch-summary.json"),
                format!("batch-summary-{attempt}.json"),
            )?;
        }
        for row in &kept_rows {
            done.insert(row["session"].as_u64().unwrap_or(0) as usize - 1);
            let stats = &row["result"]["stats"];
            global.preload(
                stats["requests"].as_u64().unwrap_or(0),
                stats["reported_cost_usd"].as_f64().unwrap_or(0.0),
            );
        }
        for index in 0..schedule.len() {
            let name = format!("session-{:03}", index + 1);
            if !done.contains(&index) && root.join(&name).exists() {
                move_aside(root.join(&name), format!("{name}-attempt-{attempt}"))?;
            }
        }
    } else {
        private_fs::create_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
        let mut manifest = create(root.join("batch-manifest.json"))?;
        serde_json::to_writer_pretty(&mut manifest, batch).map_err(|e| e.to_string())?;
        manifest.flush().map_err(|e| e.to_string())?;
    }
    let mut log = create(root.join("sessions.jsonl"))?;
    for row in &kept_rows {
        line(&mut log, row)?;
    }
    let sessions_log = Mutex::new(log);
    let mut summary = create(root.join("batch-summary.json"))?;
    let parallel = batch.parallel.max(1);
    let env = Env {
        global: global.clone(),
        quiet: env.quiet || parallel > 1,
        ..env.clone()
    };
    if schedule
        .iter()
        .any(|c| c.execution == Execution::Subprocess)
    {
        supervisor::install_interrupt_cleanup();
    }
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let rows = Mutex::new(Vec::new());
    let stopped = Mutex::new(None::<String>);
    let breaker = Breaker::new(batch);
    let work = || -> Result<(), String> {
        loop {
            let index = next.fetch_add(1, Ordering::SeqCst);
            if index >= schedule.len() || stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            if done.contains(&index) {
                continue;
            }
            if let Some(reason) = global.halt_reason() {
                stop.store(true, Ordering::SeqCst);
                lock(&stopped)?.get_or_insert(reason.to_owned());
                return Ok(());
            }
            let config = &schedule[index];
            let dir = root.join(format!("session-{:03}", index + 1));
            let mut outages = 0;
            let result = loop {
                breaker.wait();
                let result = match session(config.clone(), &dir, &env) {
                    Ok(result) => result,
                    Err(e) => {
                        stop.store(true, Ordering::SeqCst);
                        lock(&stopped)?.get_or_insert(format!("session_error: {e}"));
                        return Ok(());
                    }
                };
                if result["outage"] != true {
                    breaker.healthy();
                    break result;
                }
                // The provider outlasted every retry: pause the batch and re-run this session.
                if stop.load(Ordering::SeqCst) || !breaker.trip() {
                    break result;
                }
                outages += 1;
                let aside = superseded_dir(root)?
                    .join(format!("session-{:03}-outage-{outages}", index + 1));
                std::fs::rename(&dir, &aside).map_err(|e| format!("{}: {e}", dir.display()))?;
                println!(
                    "Session {} hit a provider outage; batch paused, will re-run it",
                    index + 1
                );
            };
            let hands = result["completed_hands"].as_u64().unwrap_or(0);
            let net = result["net_chips"][config.model_seat].as_i64().unwrap_or(0);
            let focal = !config.all_models;
            let row = json!({"session":index + 1,"seed":config.seed,"model_seat":config.model_seat,
                "model_offset":config.model_offset,"memory":config.memory,"scaffold":config.scaffold,"condition":config.condition(),"hands":hands,
                "seat_labels":result["seat_labels"],
                "model_net_chips":focal.then_some(net),
                "bb_per_100":(focal && hands > 0).then(|| net as f64 / 2.0 / hands as f64 * 100.0),
                "clean":result["clean"],"result":result});
            line(&mut *lock(&sessions_log)?, &row)?;
            println!(
                "Session {} done: seed={} seat={} condition={} hands={hands} clean={}",
                index + 1,
                config.seed,
                config.model_seat,
                config.condition(),
                row["clean"]
            );
            if let Some(reason) = row["result"]["stats"]["halt_reason"].as_str() {
                // These end only their own session: an unknown cost absorbed by the batch
                // allowance, or a model that kept faulting (its other sessions may be fine).
                let absorbed = (reason == "unknown_request_cost" && global.halt_reason().is_none())
                    || reason == "three_consecutive_failures";
                if !absorbed {
                    stop.store(true, Ordering::SeqCst);
                    lock(&stopped)?.get_or_insert(reason.to_owned());
                }
            }
            lock(&rows)?.push(row);
        }
    };
    let outcomes: Vec<Result<(), String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..parallel.min(schedule.len()))
            .map(|_| scope.spawn(work))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("session thread panicked".into()))
            })
            .collect()
    });
    outcomes.into_iter().collect::<Result<Vec<()>, String>>()?;
    let mut rows = rows.into_inner().map_err(|_| "batch state poisoned")?;
    rows.extend(kept_rows);
    rows.sort_by_key(|row| row["session"].as_u64());
    let (requests, spent) = global.totals();
    let value = json!({"scheduled_sessions":schedule.len(),"completed_sessions":rows.len(),"parallel":parallel,
        "resumed":resume,"resumed_sessions":done.len(),"absorbed_unknown_cost_requests":global.unknown_requests(),
        "requests":requests,"reported_cost_usd":spent,
        "stop_reason":stopped.into_inner().map_err(|_| "batch state poisoned")?,"sessions":rows,
        "inference":"descriptive only; run `poker-lab analyze` on this directory for seed-block intervals"});
    serde_json::to_writer_pretty(&mut summary, &value).map_err(|e| e.to_string())?;
    summary.flush().map_err(|e| e.to_string())?;
    Ok(value)
}
