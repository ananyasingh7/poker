// ABOUTME: Milestone 5 acceptance tests: supervised worker processes, fault handling, scripted
// ABOUTME: equivalence with the in-process backend, the table-wide budget ledger, and memory privacy.
use poker_lab::{
    analyze,
    arena::{Agent, PlayedHand, Record, Session, replay},
    bench,
    engine::Event,
    experiment::Batch,
    memory::Mode,
    openrouter::{Config, Execution, Format, NotebookStyle, Opponents, Stats},
    opponents::Style,
    participant::{ModelSettings, Policy},
    runner,
    supervisor::{Deadlines, Launch, Options, Supervisor},
    worker,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const KEY: &str = "secret-test-key-0123456789";

fn real() -> Launch {
    Launch {
        program: env!("CARGO_BIN_EXE_poker-lab").into(),
        args: vec!["participant".into()],
    }
}
fn faulty(fault: &str, healthy_decisions: usize) -> Launch {
    Launch {
        program: env!("CARGO_BIN_EXE_fault-worker").into(),
        args: vec![fault.into(), healthy_decisions.to_string()],
    }
}
fn quick() -> Deadlines {
    Deadlines {
        init: Duration::from_millis(1500),
        lifecycle: Duration::from_millis(500),
        operation_grace: Duration::from_millis(400),
        exit: Duration::from_millis(500),
        reap: Duration::from_secs(2),
    }
}
fn scripted_options() -> Options {
    Options {
        deadlines: quick(),
        ..Options::scripted()
    }
}
fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}
fn gone(pids: &[u32]) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while pids.iter().any(|pid| alive(*pid)) {
        if Instant::now() > deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
    true
}
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "poker-lab-test-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    poker_lab::private_fs::create_dir(&dir).unwrap();
    dir
}
fn chips(record: &Record) -> (u64, u64) {
    (
        record.initial_stacks.iter().sum(),
        record.final_stacks.iter().sum(),
    )
}

/// Plays one seeded session on both backends and returns the subprocess records.
fn both_backends(policies: &[Policy], stacks: Vec<u64>, seed: u64, hands: usize) -> Vec<Record> {
    let run = |agents: &mut [Box<dyn Agent>]| {
        let mut session = Session::new(policies.len(), seed).unwrap();
        session.stacks = stacks.clone();
        (0..hands)
            .map(|_| {
                let played = session.next_hand(agents).unwrap();
                assert!(played.lifecycle_errors.is_empty());
                played.record
            })
            .collect::<Vec<_>>()
    };
    let mut locals: Vec<Box<dyn Agent>> = policies.iter().filter_map(worker::scripted).collect();
    let expected = run(&mut locals);
    let mut supervisor = Supervisor::start(scripted_options()).unwrap();
    let mut remotes: Vec<Box<dyn Agent>> = vec![];
    for (seat, policy) in policies.iter().enumerate() {
        remotes.push(Box::new(
            supervisor.spawn(seat, policy.clone(), real()).unwrap(),
        ));
    }
    let pids: Vec<u32> = (0..policies.len())
        .map(|s| supervisor.pid(s).unwrap())
        .collect();
    let actual = run(&mut remotes);
    supervisor.shutdown().unwrap();
    assert!(!supervisor.contaminated(), "{}", supervisor.report());
    assert!(gone(&pids));
    assert_eq!(expected, actual, "backends diverged for seed {seed}");
    for record in &actual {
        replay(record).unwrap();
        assert!(record.decisions.iter().all(|d| d.error.is_none()));
    }
    actual
}

#[test]
fn scripted_tables_match_in_process_backend() {
    let mut all = vec![];
    for seed in 1..=4 {
        all.extend(both_backends(&bench::FIELD, vec![200; 6], seed, 12));
        // Uneven short stacks force all-ins and side pots.
        all.extend(both_backends(
            &bench::FIELD,
            vec![200, 27, 89, 3, 150, 60],
            seed,
            12,
        ));
        all.extend(both_backends(
            &[Policy::Aggressive, Policy::Loose],
            vec![40, 200],
            seed,
            12,
        ));
        // Version-2 archetypes. Adaptive is stateful: its worker must learn from hand-end
        // summaries exactly as the in-process agent does.
        all.extend(both_backends(
            &[
                Policy::Station,
                Policy::Overfolder,
                Policy::Solid,
                Policy::Adaptive,
                Policy::Aggressive,
                Policy::Loose,
            ],
            vec![200; 6],
            seed,
            30,
        ));
    }
    let awards = |r: &Record| {
        r.events
            .iter()
            .filter(|e| matches!(e, Event::Award { .. }))
            .count()
    };
    let reveals = |r: &Record| r.events.iter().any(|e| matches!(e, Event::Reveal { .. }));
    assert!(all.iter().any(|r| awards(r) >= 2), "no side pot exercised");
    assert!(all.iter().any(reveals), "no showdown exercised");
    assert!(all.iter().any(|r| !reveals(r)), "no fold-out exercised");
    assert!(all.iter().any(|r| r.final_stacks.contains(&0)), "no bust");
}

#[test]
fn benchmark_runs_and_backends_agree() {
    let result = bench::run(2, 4, &real()).unwrap();
    assert_eq!(result["records_identical"], true);
    assert_eq!(result["hands"], 8);
    assert!(
        result["subprocess"]["decide_round_trip"]["count"]
            .as_u64()
            .unwrap()
            > 0
    );
}

struct Table {
    supervisor: Supervisor,
    agents: Vec<Box<dyn Agent>>,
    session: Session,
    stats: Rc<RefCell<Stats>>,
    pids: Vec<u32>,
}
/// Three seats: real callers around one fixture worker in seat 1.
fn table_with_fault(launch: Launch) -> Table {
    let stats: Rc<RefCell<Stats>> = Rc::default();
    let mut supervisor = Supervisor::start(Options {
        stats: stats.clone(),
        ..scripted_options()
    })
    .unwrap();
    let mut agents: Vec<Box<dyn Agent>> = vec![];
    for seat in 0..3 {
        let launch = if seat == 1 { launch.clone() } else { real() };
        agents.push(Box::new(
            supervisor.spawn(seat, Policy::Caller, launch).unwrap(),
        ));
    }
    let pids = (0..3).map(|s| supervisor.pid(s).unwrap()).collect();
    Table {
        supervisor,
        agents,
        session: Session::new(3, 11).unwrap(),
        stats,
        pids,
    }
}
fn seat_errors(played: &PlayedHand, seat: usize) -> Vec<String> {
    played
        .record
        .decisions
        .iter()
        .filter(|d| d.seat == seat)
        .filter_map(|d| d.error.clone())
        .collect()
}

#[test]
fn transport_faults_kill_the_worker_and_contaminate_the_session() {
    let cases = [
        ("hang", "deadline"),
        ("partial_line", "deadline"),
        ("delayed", "deadline"),
        ("oversized", "oversized_frame"),
        ("stderr_flood", "stderr_overflow"),
        ("wrong_id", "wrong_correlation"),
        ("wrong_seat", "wrong_correlation"),
        ("wrong_hand", "wrong_correlation"),
        ("eof", "eof"),
        ("crash", "eof"),
        ("garbage", "malformed_response"),
        ("invalid_utf8", "invalid_utf8"),
        ("extra_field", "malformed_response"),
        ("overclaim", "usage_violation"),
        ("unexpected", "unexpected_outcome"),
    ];
    for (fault, kind) in cases {
        let mut t = table_with_fault(faulty(fault, 1));
        let started = Instant::now();
        let played = t.session.next_hand(&mut t.agents).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{fault}: arena was held up for {:?}",
            started.elapsed()
        );
        let errors = seat_errors(&played, 1);
        assert_eq!(
            errors.first().map(String::as_str),
            Some(format!("participant_transport:{kind}").as_str()),
            "{fault}: {errors:?}"
        );
        // After the fault the seat only ever falls back; nothing it sends later is accepted.
        assert!(
            errors[1..]
                .iter()
                .all(|e| e.starts_with("participant_dead:"))
        );
        let first_bad = played
            .record
            .decisions
            .iter()
            .position(|d| d.seat == 1 && d.error.is_some())
            .unwrap();
        assert!(
            played.record.decisions[first_bad..]
                .iter()
                .filter(|d| d.seat == 1)
                .all(|d| d.error.is_some()),
            "{fault}: a response was accepted after the fault"
        );
        // Healthy seats were unaffected, money is conserved and the hand replays offline.
        assert!(seat_errors(&played, 0).is_empty() && seat_errors(&played, 2).is_empty());
        let (before, after) = chips(&played.record);
        assert_eq!(before, after);
        replay(&played.record).unwrap();
        assert!(t.supervisor.contaminated());
        assert_eq!(
            t.stats.borrow().halt_reason.as_deref(),
            Some(format!("participant_failure:seat 1:{kind}").as_str())
        );
        assert!(gone(&t.pids[1..2]), "{fault}: failed worker not reaped");
        assert!(played.lifecycle_errors.iter().any(|e| e.seat == 1));
        let report = t.supervisor.report();
        assert_eq!(report["failures"][0]["fault"], kind);
        assert_eq!(report["seats"][1]["exit"]["reaped"], true, "{report}");
        t.supervisor.shutdown().unwrap();
        assert!(gone(&t.pids), "{fault}: workers left behind");
    }
}

#[test]
fn unsolicited_output_is_caught_before_the_next_request() {
    let mut t = table_with_fault(faulty("unsolicited", 0));
    let played = t.session.next_hand(&mut t.agents).unwrap();
    // The first decision was answered correctly; the stray line fails the following exchange.
    let seat_decisions: Vec<_> = played
        .record
        .decisions
        .iter()
        .filter(|d| d.seat == 1)
        .collect();
    assert!(seat_decisions[0].error.is_none());
    let later: Vec<String> = seat_errors(&played, 1)
        .into_iter()
        .chain(played.lifecycle_errors.iter().map(|e| e.error.clone()))
        .collect();
    assert!(
        later
            .iter()
            .any(|e| e == "participant_transport:unsolicited_output"),
        "{later:?}"
    );
    assert!(t.supervisor.contaminated());
    replay(&played.record).unwrap();
    t.supervisor.shutdown().unwrap();
    assert!(gone(&t.pids));
}

#[test]
fn hand_end_crash_is_reported_as_a_lifecycle_failure() {
    let mut t = table_with_fault(faulty("crash_on_hand_end", 0));
    let played = t.session.next_hand(&mut t.agents).unwrap();
    assert!(played.record.decisions.iter().all(|d| d.error.is_none()));
    assert_eq!(played.lifecycle_errors.len(), 1);
    assert_eq!(played.lifecycle_errors[0].seat, 1);
    assert_eq!(played.lifecycle_errors[0].phase, "hand_end");
    assert_eq!(
        t.supervisor.report()["seats"][1]["exit"]["code"],
        9,
        "nonzero exit status is recorded"
    );
    assert!(t.supervisor.contaminated());
    t.supervisor.shutdown().unwrap();
    assert!(gone(&t.pids));
}

#[test]
fn initialization_failure_cleans_up_started_workers() {
    let mut supervisor = Supervisor::start(scripted_options()).unwrap();
    let _first = supervisor.spawn(0, Policy::Caller, real()).unwrap();
    let first_pid = supervisor.pid(0).unwrap();
    let started = Instant::now();
    let error = supervisor
        .spawn(1, Policy::Caller, faulty("init_hang", 0))
        .err()
        .unwrap();
    assert!(error.contains("deadline"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(4));
    let hung_pid = supervisor.pid(1).unwrap();
    assert!(gone(&[hung_pid]));
    assert!(supervisor.spawn(0, Policy::Caller, real()).is_err());
    let missing = Launch {
        program: "/nonexistent/poker-lab-worker".into(),
        args: vec![],
    };
    assert!(supervisor.spawn(2, Policy::Caller, missing).is_err());
    // A host error path: dropping the supervisor without shutdown still reaps everything.
    drop(supervisor);
    assert!(gone(&[first_pid]));
}

#[test]
fn shutdown_kills_workers_that_ignore_it() {
    for fault in ["ignore_shutdown", "ack_no_exit"] {
        let mut t = table_with_fault(faulty(fault, usize::MAX));
        let played = t.session.next_hand(&mut t.agents).unwrap();
        assert!(played.record.decisions.iter().all(|d| d.error.is_none()));
        assert!(!t.supervisor.contaminated());
        let started = Instant::now();
        t.supervisor.shutdown().unwrap();
        assert!(started.elapsed() < Duration::from_secs(4));
        let report = t.supervisor.report();
        assert_eq!(
            report["seats"][1]["exit"]["killed"], true,
            "{fault}: {report}"
        );
        assert_eq!(report["seats"][1]["exit"]["reaped"], true);
        assert_eq!(report["seats"][0]["exit"]["code"], 0);
        assert_eq!(report["seats"][0]["exit"]["killed"], false);
        assert!(t.supervisor.contaminated(), "{fault}");
        assert!(gone(&t.pids));
    }
}

#[test]
fn worker_exits_when_its_host_goes_away() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_poker-lab"))
        .arg("participant")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    assert_eq!(child.wait().unwrap().code(), Some(0));
    // A v1 development frame is rejected with a clear diagnostic rather than answered.
    let mut child = Command::new(env!("CARGO_BIN_EXE_poker-lab"))
        .arg("participant")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"version\":1,\"id\":1,\"message\":{\"kind\":\"reset\"}}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported request frame"));
}

#[test]
fn interrupt_kills_worker_processes() {
    let mut host = Command::new(env!("CARGO_BIN_EXE_poker-lab"))
        .args(["bench", "1000000", "10"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let children = || -> Vec<u32> {
        let out = Command::new("pgrep")
            .args(["-P", &host.id().to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = children();
    while seen.is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
        seen = children();
    }
    assert!(!seen.is_empty(), "benchmark never started workers");
    Command::new("kill")
        .args(["-INT", &host.id().to_string()])
        .status()
        .unwrap();
    assert_eq!(host.wait().unwrap().code(), Some(130));
    assert!(gone(&seen), "workers survived the interrupt");
}

// ---- Model workers against a local mock provider. No test uses a real API key. ----

type Responder = dyn Fn(&Value) -> (u16, Value) + Send + Sync;
/// A minimal HTTP server that records every request body and answers with `responder`.
fn provider(responder: Arc<Responder>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            let header_end = loop {
                let n = stream.read(&mut buf).unwrap();
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                assert!(n > 0);
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]).to_lowercase();
            assert!(headers.contains(&format!("authorization: bearer {}", KEY.to_lowercase())));
            let length: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while bytes.len() < header_end + length {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            let request: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
            let (status, body) = responder(&request);
            log.lock().unwrap().push(request);
            let body = body.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, seen)
}
fn completion(content: &str) -> Value {
    json!({"id":"gen-test","model":"test/model-v1","provider":"mock",
        "choices":[{"finish_reason":"stop","message":{"content":content}}],
        "usage":{"prompt_tokens":100,"completion_tokens":10,"cost":0.001}})
}
fn observation(request: &Value) -> Value {
    serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap()
}
fn is_notebook(request: &Value) -> bool {
    request["response_format"]["json_schema"]["name"] == "notebook_v1"
}
/// Checks or calls every action; writes a seat-tagged note on every notebook request.
fn calling_provider() -> (String, Arc<Mutex<Vec<Value>>>) {
    provider(Arc::new(|request| {
        if is_notebook(request) {
            let seat = observation(request)["seat"].as_u64().unwrap();
            let note = json!({"operation":"replace","notebook":format!("NOTE-OF-SEAT-{seat}")});
            (200, completion(&note.to_string()))
        } else {
            let passive = if observation(request)["legal"]["check"] == true {
                "check"
            } else {
                "call"
            };
            let action = json!({"action":passive,"amount":null});
            (200, completion(&action.to_string()))
        }
    }))
}
fn settings(endpoint: &str, memory: Mode) -> ModelSettings {
    ModelSettings {
        model: "test/model".into(),
        timeout_ms: 2000,
        max_tokens: 64,
        max_request_bytes: 65_536,
        response_format: Format::JsonSchema,
        reasoning_enabled: Some(false),
        reasoning_effort: None,
        memory,
        notebook_style: poker_lab::openrouter::NotebookStyle::Journal,
        normalize_call_amount: false,
        scaffold: poker_lab::scaffold::Scaffold::None,
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
        notebook_chars: 400,
        max_requests: 1000,
        spend_limit_usd: 10.0,
        local_port: None,
        local_json_prefill: false,
        endpoint: Some(endpoint.into()),
    }
}
struct ModelTable {
    supervisor: Supervisor,
    agents: Vec<Box<dyn Agent>>,
    session: Session,
    stats: Rc<RefCell<Stats>>,
    dir: PathBuf,
}
fn model_table(
    name: &str,
    seats: usize,
    max_requests: u64,
    spend_limit_usd: f64,
    policy: impl Fn(usize) -> (Policy, Launch),
) -> ModelTable {
    let dir = temp_dir(name);
    let stats: Rc<RefCell<Stats>> = Rc::default();
    let mut supervisor = Supervisor::start(Options {
        deadlines: quick(),
        run_dir: Some(dir.clone()),
        max_requests,
        spend_limit_usd,
        key: Some(KEY.into()),
        stats: stats.clone(),
    })
    .unwrap();
    let mut agents: Vec<Box<dyn Agent>> = vec![];
    for seat in 0..seats {
        let (policy, launch) = policy(seat);
        agents.push(Box::new(supervisor.spawn(seat, policy, launch).unwrap()));
    }
    ModelTable {
        supervisor,
        agents,
        session: Session::new(seats, 5).unwrap(),
        stats,
        dir,
    }
}
fn read_tree(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut files = vec![];
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
                files.push((path, text));
            }
        }
    }
    files
}

#[test]
fn six_model_workers_share_one_request_budget() {
    let (url, seen) = calling_provider();
    let mut t = model_table("budget", 6, 5, 10.0, |_| {
        (Policy::OpenRouter(settings(&url, Mode::None)), real())
    });
    let played = t.session.next_hand(&mut t.agents).unwrap();
    replay(&played.record).unwrap();
    // Exactly five provider requests in total, not five per worker.
    assert_eq!(seen.lock().unwrap().len(), 5);
    let stats = t.stats.borrow().clone();
    assert_eq!(stats.requests, 5);
    assert_eq!(stats.halt_reason.as_deref(), Some("request_limit"));
    assert!((stats.reported_cost_usd - 0.005).abs() < 1e-9);
    assert_eq!((stats.prompt_tokens, stats.completion_tokens), (500, 50));
    assert_eq!(stats.unknown_cost_requests, 0);
    let fallbacks = played
        .record
        .decisions
        .iter()
        .filter(|d| d.error.is_some())
        .count();
    assert!(fallbacks > 0);
    assert_eq!(stats.fallbacks, fallbacks as u64);
    assert!(
        played
            .record
            .decisions
            .iter()
            .filter_map(|d| d.error.as_deref())
            .all(|e| e == "openrouter:Budget")
    );
    t.supervisor.shutdown().unwrap();
    // A budget halt is not a transport failure: workers stayed healthy and exited cleanly.
    assert!(!t.supervisor.contaminated());
    let report = t.supervisor.report();
    let per_seat: u64 = report["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["usage"]["requests"].as_u64().unwrap())
        .sum();
    assert_eq!(per_seat, 5);
    // The credential reaches the provider but no artifact, and no seed crosses the boundary.
    let files = read_tree(&t.dir);
    assert!(
        files
            .iter()
            .any(|(p, _)| p.ends_with("workers/seat-3/model-private.jsonl"))
    );
    assert!(
        files
            .iter()
            .any(|(p, _)| p.ends_with("workers/seat-3/wire.jsonl"))
    );
    for (path, text) in &files {
        assert!(!text.contains(KEY), "credential leaked into {path:?}");
        assert!(!text.contains("seed"), "seed leaked into {path:?}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{path:?}");
        }
    }
    let lifecycle = &files
        .iter()
        .find(|(p, _)| p.ends_with("participants.jsonl"))
        .unwrap()
        .1;
    assert_eq!(lifecycle.matches("\"kind\":\"spawn\"").count(), 6);
    assert_eq!(lifecycle.matches("\"kind\":\"dispatch\"").count(), 5);
    assert_eq!(lifecycle.matches("\"kind\":\"exit\"").count(), 6);
}

#[test]
fn spend_threshold_and_unknown_cost_stop_every_seat() {
    let (url, seen) = calling_provider();
    let mut t = model_table("spend", 3, 100, 0.0015, |_| {
        (Policy::OpenRouter(settings(&url, Mode::None)), real())
    });
    t.session.next_hand(&mut t.agents).unwrap();
    // Post-response threshold: the second request crosses it, and nobody makes a third.
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(
        t.stats.borrow().halt_reason.as_deref(),
        Some("reported_spend_limit")
    );
    t.supervisor.shutdown().unwrap();

    let (url, seen) = provider(Arc::new(|_| {
        let mut body = completion(r#"{"action":"call","amount":null}"#);
        body["usage"] = Value::Null;
        (200, body)
    }));
    let mut t = model_table("unknown", 3, 100, 10.0, |_| {
        (Policy::OpenRouter(settings(&url, Mode::None)), real())
    });
    let played = t.session.next_hand(&mut t.agents).unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    let stats = t.stats.borrow().clone();
    assert_eq!(stats.halt_reason.as_deref(), Some("unknown_request_cost"));
    assert_eq!((stats.requests, stats.unknown_cost_requests), (1, 1));
    // The one answered decision was applied; every later model decision fell back.
    assert_eq!(
        played
            .record
            .decisions
            .iter()
            .filter(|d| d.error.is_none())
            .count(),
        1
    );
    t.supervisor.shutdown().unwrap();

    let (url, seen) = provider(Arc::new(|_| (401, json!({"error":{"message":"no"}}))));
    let mut t = model_table("http", 3, 100, 10.0, |_| {
        (Policy::OpenRouter(settings(&url, Mode::None)), real())
    });
    let played = t.session.next_hand(&mut t.agents).unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(
        played.record.decisions[0].error.as_deref(),
        Some("openrouter:Http")
    );
    // A provider error is a fallback, not a transport failure.
    t.supervisor.shutdown().unwrap();
    assert!(!t.supervisor.contaminated());
}

#[test]
fn crash_after_dispatch_keeps_the_slot_charged_with_unknown_cost() {
    let (url, seen) = calling_provider();
    // Heads-up: seat 0 is the button and acts first, so the crashing worker is granted first.
    let mut t = model_table("crash", 2, 100, 10.0, |seat| {
        let policy = Policy::OpenRouter(settings(&url, Mode::None));
        if seat == 0 {
            (policy, faulty("crash", 0))
        } else {
            (policy, real())
        }
    });
    let played = t.session.next_hand(&mut t.agents).unwrap();
    replay(&played.record).unwrap();
    let stats = t.stats.borrow().clone();
    assert_eq!((stats.requests, stats.unknown_cost_requests), (1, 1));
    assert_eq!(
        stats.halt_reason.as_deref(),
        Some("participant_failure:seat 0:eof")
    );
    // The healthy model worker is refused further provider calls for the rest of the hand.
    assert_eq!(seen.lock().unwrap().len(), 0);
    assert!(played.record.decisions.iter().all(|d| d.error.is_some()));
    assert!(t.supervisor.contaminated());
    t.supervisor.shutdown().unwrap();
    let lifecycle = std::fs::read_to_string(t.dir.join("participants.jsonl")).unwrap();
    // The parent's record of the dispatched operation survives the missing worker trace.
    assert!(lifecycle.contains("\"kind\":\"dispatch\""));
    assert!(lifecycle.contains("\"kind\":\"failure\""));
    assert!(!t.dir.join("workers/seat-0/model-private.jsonl").exists());
}

#[test]
fn notebooks_stay_private_per_seat_and_reset_with_fresh_processes() {
    let (url, seen) = calling_provider();
    let play_two_hands = |name: &str| {
        let mut t = model_table(name, 2, 100, 10.0, |_| {
            (Policy::OpenRouter(settings(&url, Mode::Notes)), real())
        });
        for _ in 0..2 {
            let played = t.session.next_hand(&mut t.agents).unwrap();
            assert!(played.lifecycle_errors.is_empty());
            assert!(played.record.decisions.iter().all(|d| d.error.is_none()));
        }
        t.supervisor.shutdown().unwrap();
        assert!(!t.supervisor.contaminated());
        let stats = t.stats.borrow().clone();
        // One notebook request per seat per hand, charged to the same shared ledger.
        assert_eq!(stats.memory_requests, 4);
        assert_eq!(stats.memory_errors, 0);
        t
    };
    let first = play_two_hands("notes-a");
    let requests = std::mem::take(&mut *seen.lock().unwrap());
    assert_eq!(first.stats.borrow().requests, requests.len() as u64);
    let mut saw_own_note = [false; 2];
    for request in requests.iter().filter(|r| !is_notebook(r)) {
        let context = observation(request);
        let seat = context["seat"].as_u64().unwrap() as usize;
        let notebook = context["private_notebook"].as_str().unwrap();
        assert!(notebook.is_empty() || notebook == format!("NOTE-OF-SEAT-{seat}"));
        saw_own_note[seat] |= !notebook.is_empty();
        assert!(context.get("completed_hand_stats").is_none());
        assert!(context.get("seed").is_none());
    }
    assert_eq!(
        saw_own_note,
        [true, true],
        "hand two must see hand one's note"
    );
    for (path, text) in read_tree(&first.dir) {
        assert!(!text.contains(KEY), "credential leaked into {path:?}");
        for (seat, other) in [(0, 1), (1, 0)] {
            if path.to_string_lossy().contains(&format!("seat-{seat}")) {
                assert!(
                    !text.contains(&format!("NOTE-OF-SEAT-{other}")),
                    "seat {other}'s note reached {path:?}"
                );
            }
        }
    }
    // A new session means new processes: no note survives from the previous one.
    play_two_hands("notes-b");
    let requests = seen.lock().unwrap();
    let first_action = requests.iter().find(|r| !is_notebook(r)).unwrap();
    assert_eq!(observation(first_action)["private_notebook"], "");
}

#[test]
fn memory_conditions_expose_only_their_layers_through_workers() {
    for mode in [Mode::None, Mode::Notes, Mode::Stats, Mode::Both] {
        let (url, seen) = calling_provider();
        let mut t = model_table("modes", 2, 100, 10.0, |seat| {
            if seat == 0 {
                (Policy::OpenRouter(settings(&url, mode)), real())
            } else {
                (Policy::Caller, real())
            }
        });
        for _ in 0..2 {
            let played = t.session.next_hand(&mut t.agents).unwrap();
            assert!(played.lifecycle_errors.is_empty(), "{mode:?}");
        }
        t.supervisor.shutdown().unwrap();
        let requests = seen.lock().unwrap();
        let notebook_requests = requests.iter().filter(|r| is_notebook(r)).count();
        assert_eq!(
            notebook_requests,
            if mode.notes() { 2 } else { 0 },
            "{mode:?}"
        );
        assert_eq!(t.stats.borrow().memory_requests, notebook_requests as u64);
        let last_action = requests.iter().rfind(|r| !is_notebook(r)).unwrap();
        let context = observation(last_action);
        assert_eq!(context.get("private_notebook").is_some(), mode.notes());
        assert_eq!(context.get("completed_hand_stats").is_some(), mode.stats());
        if mode.stats() {
            // Statistics are updated from hand summaries without any provider call.
            assert_eq!(context["completed_hand_stats"][1]["dealt"], 1);
        }
        // The scripted seat never calls the provider and holds no credential.
        assert!(requests.iter().all(|r| observation(r)["seat"] == 0));
    }
}

#[test]
fn scaffold_aids_reach_the_model_through_the_worker() {
    let (url, seen) = calling_provider();
    let mut t = model_table("scaffold", 2, 100, 10.0, |seat| {
        if seat == 0 {
            let mut s = settings(&url, Mode::None);
            s.scaffold = poker_lab::scaffold::Scaffold::Advice;
            (Policy::OpenRouter(s), real())
        } else {
            (Policy::Caller, real())
        }
    });
    let played = t.session.next_hand(&mut t.agents).unwrap();
    assert!(played.lifecycle_errors.is_empty());
    replay(&played.record).unwrap();
    t.supervisor.shutdown().unwrap();
    let requests = seen.lock().unwrap();
    assert!(!requests.is_empty());
    for request in requests.iter() {
        let aids = &observation(request)["decision_aids"];
        assert!(aids["equity_vs_random_hands"].is_number());
        assert!(aids["suggested_action"]["action"].is_string());
    }
}

#[test]
fn rejected_notebook_update_keeps_old_notes_and_the_worker() {
    let (url, seen) = calling_provider();
    let mut t = model_table("oversized-note", 2, 100, 10.0, |seat| {
        if seat == 0 {
            let mut s = settings(&url, Mode::Notes);
            s.notebook_chars = 3; // every mock note is longer than this
            (Policy::OpenRouter(s), real())
        } else {
            (Policy::Caller, real())
        }
    });
    for _ in 0..2 {
        let played = t.session.next_hand(&mut t.agents).unwrap();
        assert_eq!(played.lifecycle_errors.len(), 1);
        assert_eq!(played.lifecycle_errors[0].error, "notebook:Malformed");
        assert!(played.record.decisions.iter().all(|d| d.error.is_none()));
    }
    let stats = t.stats.borrow().clone();
    assert_eq!((stats.memory_requests, stats.memory_errors), (2, 2));
    assert!(stats.halt_reason.is_none());
    t.supervisor.shutdown().unwrap();
    // A memory error is reported, not fatal: no transport failure, and the old notebook stays.
    assert!(!t.supervisor.contaminated());
    let requests = seen.lock().unwrap();
    let last_action = requests.iter().rfind(|r| !is_notebook(r)).unwrap();
    assert_eq!(observation(last_action)["private_notebook"], "");
}

// ---- Milestones 6-8: runner, parallel batches, analysis, mixed models, oracle notes. ----

fn env(url: &str) -> runner::Env {
    runner::Env {
        endpoint: Some(url.into()),
        launch: Some(real()),
        quiet: true,
        ..runner::Env::new(KEY.into())
    }
}
fn small_config(execution: Execution) -> Config {
    Config {
        model: "test/model".into(),
        seats: 3,
        hands: 3,
        timeout_ms: 2000,
        max_requests: 1000,
        spend_limit_usd: 10.0,
        reasoning_enabled: Some(false),
        opponents: Opponents::Mixed,
        opponent_field: vec![Style::Station, Style::Solid],
        reset_stacks: true,
        execution,
        max_attempts: 3,
        retry_backoff_ms: 1,
        retry_backoff_max_ms: 4,
        ..Config::default()
    }
}
fn small_batch(execution: Execution, parallel: usize) -> Batch {
    Batch {
        pilot: small_config(execution),
        seeds: vec![1, 2, 3],
        model_seats: vec![0, 1, 2],
        conditions: vec![Mode::None, Mode::Stats],
        max_requests: 10_000,
        spend_limit_usd: 100.0,
        parallel,
        allow_partial_orbits: false,
        scaffolds: vec![],
        unknown_cost_allowance: 0,
        unknown_cost_charge_usd: 0.01,
        outage_pause_ms: 20,
        outage_max_wait_s: 5,
    }
}
fn passive_action(request: &Value) -> String {
    let passive = if observation(request)["legal"]["check"] == true {
        "check"
    } else {
        "call"
    };
    json!({"action":passive,"amount":null}).to_string()
}
/// Like `calling_provider`, but request number `fail_at` (1-based) is answered without any
/// usage or cost, so its cost is unknown (a strict halt, unlike a retryable 503).
fn costless_provider(fail_at: usize) -> (String, Arc<Mutex<Vec<Value>>>) {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    provider(Arc::new(move |request| {
        let mut body = completion(&passive_action(request));
        if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 == fail_at {
            body["usage"] = Value::Null;
        }
        (200, body)
    }))
}
/// Answers 503 to every request for which `down(request_number)` holds (1-based), else plays.
fn outage_provider(
    down: impl Fn(usize) -> bool + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<Value>>>) {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    provider(Arc::new(move |request| {
        let n = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if down(n) {
            return (503, json!("error code: 1102"));
        }
        (200, completion(&passive_action(request)))
    }))
}

#[test]
fn unknown_cost_allowance_ends_only_the_affected_session() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let (url, _) = costless_provider(5);
        let mut batch = small_batch(execution, 1);
        batch.unknown_cost_allowance = 1;
        let root = temp_dir("unknown-allowance").join("batch");
        let result = runner::batch(&batch, &root, &env(&url)).unwrap();
        assert!(result["stop_reason"].is_null(), "{result}");
        assert_eq!(result["completed_sessions"], 18);
        assert_eq!(result["absorbed_unknown_cost_requests"], 1);
        let rows = result["sessions"].as_array().unwrap();
        let unclean: Vec<_> = rows.iter().filter(|r| r["clean"] == false).collect();
        assert_eq!(unclean.len(), 1, "{execution:?}");
        assert_eq!(
            unclean[0]["result"]["stats"]["halt_reason"],
            "unknown_request_cost"
        );
        // The absorbed request is charged at the conservative rate.
        assert!(result["reported_cost_usd"].as_f64().unwrap() >= 0.01);
    }
}

#[test]
fn interrupted_batches_resume_without_replaying_finished_sessions() {
    let (url, _) = costless_provider(25);
    let mut batch = small_batch(Execution::InProcess, 1);
    let root = temp_dir("resume").join("batch");
    let first = runner::batch(&batch, &root, &env(&url)).unwrap();
    assert_eq!(first["stop_reason"], "unknown_request_cost");
    let finished = first["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["result"]["completed_hands"] == r["result"]["requested_hands"])
        .count();
    assert!(finished > 0 && finished < 18);
    let before = std::fs::read_to_string(root.join("session-001/evaluator.jsonl")).unwrap();
    assert_eq!(first["sessions"][0]["clean"], true);
    // A changed design is refused; a changed budget or allowance is not.
    let mut redesigned = small_batch(Execution::InProcess, 1);
    redesigned.seeds = vec![9];
    assert!(
        runner::resume(&redesigned, &root, &env(&url))
            .unwrap_err()
            .contains("design")
    );
    batch.unknown_cost_allowance = 3;
    batch.parallel = 4;
    let (url, seen) = calling_provider();
    let second = runner::resume(&batch, &root, &env(&url)).unwrap();
    assert!(second["stop_reason"].is_null(), "{second}");
    assert_eq!(second["completed_sessions"], 18);
    assert_eq!(second["resumed_sessions"], finished);
    // Finished sessions were not replayed; the interrupted one moved aside as evidence.
    assert_eq!(
        std::fs::read_to_string(root.join("session-001/evaluator.jsonl")).unwrap(),
        before
    );
    assert!(root.join("superseded/sessions-1.jsonl").exists());
    assert!(root.join("superseded/resume-1-batch.json").exists());
    assert!(
        std::fs::read_dir(root.join("superseded"))
            .unwrap()
            .any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("-attempt-1"))
    );
    // The cap counts both attempts' requests.
    assert!(second["requests"].as_u64().unwrap() > seen.lock().unwrap().len() as u64);
    let analysis = analyze::directory(&root).unwrap();
    assert_eq!(analysis["sessions"], 18);
    assert_eq!(analysis["hands"], 54);
    let sessions = std::fs::read_to_string(root.join("sessions.jsonl")).unwrap();
    assert_eq!(sessions.lines().count(), 18);
}

#[test]
fn parallel_batches_share_one_budget_and_can_be_analyzed() {
    for (execution, name) in [
        (Execution::InProcess, "batch-local"),
        (Execution::Subprocess, "batch-workers"),
    ] {
        let (url, seen) = calling_provider();
        let root = temp_dir(name).join("batch");
        let result = runner::batch(&small_batch(execution, 4), &root, &env(&url)).unwrap();
        assert_eq!(result["completed_sessions"], 18, "{result}");
        assert!(result["stop_reason"].is_null());
        assert_eq!(result["parallel"], 4);
        // The shared ledger saw exactly the requests the provider received.
        assert_eq!(
            result["requests"].as_u64().unwrap(),
            seen.lock().unwrap().len() as u64
        );
        let rows = result["sessions"].as_array().unwrap();
        assert!(rows.iter().all(|r| r["clean"] == true), "{result}");
        assert!(rows.iter().all(|r| r["hands"] == 3));
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row["session"], i + 1);
        }
        let analysis = analyze::directory(&root).unwrap();
        assert_eq!(analysis["sessions"], 18);
        assert_eq!(analysis["hands"], 54);
        assert_eq!(analysis["clean_sessions"], 18);
        let model = analysis["win_rates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["label"] == "test/model" && r["condition"] == "none")
            .unwrap();
        // Three seeds are three independent blocks, each averaged over three seat rotations.
        assert_eq!(model["blocks"], 3);
        assert_eq!(model["seat_hands"], 27);
        assert!(model["ci95"].is_array());
        let paired = &analysis["paired_differences"][0];
        assert_eq!(
            (&paired["condition"], &paired["versus"]),
            (&json!("stats"), &json!("none"))
        );
        assert_eq!(paired["blocks"], 3);
        let labels: Vec<&str> = analysis["style"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["label"].as_str().unwrap())
            .collect();
        assert_eq!(labels, ["solid", "station", "test/model"]);
        // The mock always checks or calls and never folds or raises.
        let style = &analysis["style"][2];
        assert_eq!(style["is_model"], true);
        assert_eq!(style["pfr"], 0.0);
        assert_eq!(style["fold_to_bet"], 0.0);
        assert!(analyze::render(&analysis).contains("Paired difference"));
        // Stacks reset every hand.
        let record: Record = serde_json::from_str(
            std::fs::read_to_string(root.join("session-001/evaluator.jsonl"))
                .unwrap()
                .lines()
                .last()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(record.initial_stacks, [200, 200, 200]);
    }
}

#[test]
fn batch_budget_stops_all_concurrent_sessions() {
    let (url, seen) = calling_provider();
    let mut batch = small_batch(Execution::InProcess, 4);
    batch.max_requests = 7;
    let root = temp_dir("batch-budget").join("batch");
    let result = runner::batch(&batch, &root, &env(&url)).unwrap();
    assert_eq!(result["stop_reason"], "global_request_limit");
    assert_eq!(seen.lock().unwrap().len(), 7);
    assert_eq!(result["requests"], 7);
    assert!(result["completed_sessions"].as_u64().unwrap() < 18);
    assert!(
        result["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["clean"] == false)
    );

    let mut uneven = small_batch(Execution::InProcess, 1);
    uneven.pilot.hands = 4;
    assert!(uneven.schedule().unwrap_err().contains("multiple of seats"));
    uneven.allow_partial_orbits = true;
    assert!(uneven.schedule().is_ok());
}

#[test]
fn mixed_model_tables_rotate_models_through_seats() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let (url, seen) = calling_provider();
        let mut batch = small_batch(execution, 2);
        batch.pilot.all_models = true;
        batch.pilot.models = vec!["lab/alpha".into(), "lab/beta".into(), "lab/gamma".into()];
        batch.seeds = vec![1, 2];
        batch.conditions = vec![Mode::None];
        let root = temp_dir("mixed-models").join("batch");
        let result = runner::batch(&batch, &root, &env(&url)).unwrap();
        assert_eq!(result["completed_sessions"], 6, "{result}");
        let rows = result["sessions"].as_array().unwrap();
        assert_eq!(
            rows[0]["seat_labels"],
            json!(["lab/alpha", "lab/beta", "lab/gamma"])
        );
        assert_eq!(
            rows[1]["seat_labels"],
            json!(["lab/beta", "lab/gamma", "lab/alpha"])
        );
        assert!(rows[0]["bb_per_100"].is_null());
        // Every request names the model assigned to the seat that is acting.
        let expected: Vec<Vec<String>> = rows
            .iter()
            .map(|r| serde_json::from_value(r["seat_labels"].clone()).unwrap())
            .collect();
        for request in seen.lock().unwrap().iter() {
            let seat = observation(request)["seat"].as_u64().unwrap() as usize;
            let model = request["model"].as_str().unwrap();
            assert!(expected.iter().any(|labels| labels[seat] == model));
        }
        let analysis = analyze::directory(&root).unwrap();
        let rates = analysis["win_rates"].as_array().unwrap();
        assert_eq!(rates.len(), 3);
        for rate in rates {
            assert_eq!(rate["blocks"], 2);
            assert_eq!(rate["seat_hands"], 18);
        }
        // Chips only move between the three models.
        let total: f64 = analysis["style"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["net_chips"].as_f64().unwrap())
            .sum();
        assert_eq!(total, 0.0);
    }
    let mut config = small_config(Execution::InProcess);
    config.models = vec!["a".into(), "b".into(), "c".into()];
    assert!(config.validate().is_err(), "models requires all_models");
}

#[test]
fn oracle_notes_and_profile_prompt_reach_only_the_model() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let (url, seen) = calling_provider();
        let mut config = small_config(execution);
        config.memory = Mode::Oracle;
        let dir = temp_dir("oracle").join("run");
        let result = runner::session(config.clone(), &dir, &env(&url)).unwrap();
        assert_eq!(result["clean"], true, "{result}");
        let requests = std::mem::take(&mut *seen.lock().unwrap());
        // Oracle notes are fixed text: shown at every decision, never rewritten by the model.
        assert!(!requests.is_empty() && requests.iter().all(|r| !is_notebook(r)));
        assert_eq!(result["stats"]["memory_requests"], 0);
        for request in &requests {
            let notes = observation(request)["private_notebook"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(notes.contains("Seat 1: Calling station"), "{notes}");
            assert!(notes.contains("Seat 2: Solid"));
            assert!(!notes.contains("Seat 0:"));
        }

        let mut config = small_config(execution);
        config.memory = Mode::Notes;
        config.notebook_style = NotebookStyle::Profiles;
        let dir = temp_dir("profiles").join("run");
        runner::session(config, &dir, &env(&url)).unwrap();
        let requests = seen.lock().unwrap();
        let update = requests.iter().find(|r| is_notebook(r)).unwrap();
        let prompt = update["messages"][0]["content"].as_str().unwrap();
        assert!(
            prompt.contains("scouting notes") && prompt.contains("Do NOT keep a hand-by-hand log")
        );
    }
    let mut config = small_config(Execution::InProcess);
    config.memory = Mode::Oracle;
    config.all_models = true;
    assert!(config.validate().is_err());
}

fn evaluator_log(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("evaluator.jsonl")).unwrap()
}
fn summary(dir: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("summary.json")).unwrap()).unwrap()
}

#[test]
fn infrastructure_faults_do_not_change_the_game() {
    // Same seed, a clean provider versus one that refuses every third request (503) and every
    // seventh (429): the retries must be invisible in the recorded hands.
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let mut config = small_config(execution);
        config.hands = 6;
        config.max_attempts = 5;
        let (clean_url, clean_seen) = calling_provider();
        let clean = temp_dir("fault-clean").join("run");
        let clean_result = runner::session(config.clone(), &clean, &env(&clean_url)).unwrap();
        let (url, seen) = outage_provider(|n| n % 3 == 0 || n % 7 == 0);
        let flaky = temp_dir("fault-flaky").join("run");
        let result = runner::session(config, &flaky, &env(&url)).unwrap();
        assert_eq!(result["clean"], true, "{execution:?} {result}");
        assert_eq!(result["stats"]["fallbacks"], 0);
        assert_eq!(result["outage"], false);
        assert_eq!(
            evaluator_log(&clean),
            evaluator_log(&flaky),
            "{execution:?}"
        );
        let failures = result["stats"]["infra_failures"].as_u64().unwrap();
        assert!(failures > 0);
        // Every attempt was a real request; the successful ones match the clean run.
        let (sent, clean_sent) = (seen.lock().unwrap().len(), clean_seen.lock().unwrap().len());
        assert_eq!(sent as u64, clean_sent as u64 + failures);
        assert_eq!(result["stats"]["requests"].as_u64().unwrap(), sent as u64);
        // Refused requests were not billed: spend equals the clean run's.
        assert_eq!(
            result["stats"]["reported_cost_usd"],
            clean_result["stats"]["reported_cost_usd"]
        );
        assert_eq!(result["stats"]["assumed_cost_usd"], 0.0);
    }
}

#[test]
fn an_outage_abandons_the_hand_and_records_no_fallback() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let mut config = small_config(execution);
        config.max_attempts = 2;
        // Healthy for the first few requests, then permanently down.
        let (url, _) = outage_provider(|n| n > 3);
        let dir = temp_dir("outage-session").join("run");
        let result = runner::session(config, &dir, &env(&url)).unwrap();
        assert_eq!(result["outage"], true, "{execution:?} {result}");
        assert_eq!(result["clean"], false);
        assert_eq!(result["stats"]["halt_reason"], "provider_outage");
        assert_eq!(result["stats"]["fallbacks"], 0);
        assert_eq!(result["stats"]["outage_decisions"], 1);
        let completed = result["completed_hands"].as_u64().unwrap();
        assert!(completed < 3);
        // Every hand on disk is whole, replays, and contains no fallback decision.
        let log = evaluator_log(&dir);
        assert_eq!(log.lines().count() as u64, completed);
        for text in log.lines() {
            let record: Record = serde_json::from_str(text).unwrap();
            replay(&record).unwrap();
            assert!(record.decisions.iter().all(|d| d.error.is_none()));
        }
        assert_eq!(summary(&dir)["completed_hands"], completed);
    }
}

#[test]
fn a_batch_pauses_through_an_outage_and_reruns_the_affected_session() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        let mut batch = small_batch(execution, 1);
        batch.pilot.max_attempts = 2;
        // Both attempts of the very first decision fail, then the provider recovers.
        let (url, _) = outage_provider(|n| n <= 2);
        let root = temp_dir("outage-batch").join("batch");
        let result = runner::batch(&batch, &root, &env(&url)).unwrap();
        assert!(result["stop_reason"].is_null(), "{execution:?} {result}");
        assert_eq!(result["completed_sessions"], 18);
        let rows = result["sessions"].as_array().unwrap();
        assert!(
            rows.iter().all(|r| r["clean"] == true && r["hands"] == 3),
            "{result}"
        );
        // The interrupted attempt is kept as evidence, not mixed into the results.
        assert!(root.join("superseded/session-001-outage-1").exists());
        assert_eq!(analyze::directory(&root).unwrap()["sessions"], 18);
    }
}

#[test]
fn a_model_that_keeps_faulting_ends_its_sessions_but_not_the_batch() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        // Every reply is an illegal raise, so every session halts after three faults.
        let (url, _) = provider(Arc::new(|_| {
            (200, completion(r#"{"action":"raise_to","amount":1}"#))
        }));
        let batch = small_batch(execution, 4);
        let root = temp_dir("faulting-model").join("batch");
        let result = runner::batch(&batch, &root, &env(&url)).unwrap();
        assert!(result["stop_reason"].is_null(), "{execution:?} {result}");
        assert_eq!(result["completed_sessions"], 18);
        let rows = result["sessions"].as_array().unwrap();
        assert!(rows.iter().all(|r| r["clean"] == false));
        // Sessions with fewer than three decisions end without the halt; the rest stop on it.
        let halted = rows
            .iter()
            .filter(|r| r["result"]["stats"]["halt_reason"] == "three_consecutive_failures")
            .count();
        assert!(
            halted > 0
                && rows.iter().all(|r| {
                    let reason = &r["result"]["stats"]["halt_reason"];
                    reason.is_null() || reason == "three_consecutive_failures"
                })
        );
    }
}

#[test]
fn illegal_checks_are_repaired_through_both_backends() {
    for execution in [Execution::InProcess, Execution::Subprocess] {
        // First answer is always "check"; the repair request (4 messages) gets a legal action.
        let (url, seen) = provider(Arc::new(|request| {
            let repair = request["messages"].as_array().unwrap().len() == 4;
            let answer = if repair {
                passive_action(request)
            } else {
                json!({"action":"check","amount":null}).to_string()
            };
            (200, completion(&answer))
        }));
        let mut config = small_config(execution);
        config.repair_illegal = true;
        let dir = temp_dir("repair").join("run");
        let result = runner::session(config, &dir, &env(&url)).unwrap();
        let stats = &result["stats"];
        assert_eq!(stats["fallbacks"], 0, "{execution:?} {result}");
        let repaired = stats["repaired_actions"].as_u64().unwrap();
        assert!(repaired > 0);
        assert_eq!(
            stats["requests"].as_u64().unwrap(),
            seen.lock().unwrap().len() as u64
        );
        assert_eq!(result["clean"], true);
    }
}

#[test]
fn a_batch_gives_up_after_its_outage_limit_and_can_be_resumed() {
    let mut batch = small_batch(Execution::InProcess, 1);
    batch.pilot.max_attempts = 2;
    batch.outage_max_wait_s = 0;
    let (down_url, _) = outage_provider(|_| true);
    let root = temp_dir("outage-giveup").join("batch");
    let first = runner::batch(&batch, &root, &env(&down_url)).unwrap();
    assert_eq!(first["stop_reason"], "provider_outage");
    assert!(first["completed_sessions"].as_u64().unwrap() < 18);
    let (url, _) = calling_provider();
    let second = runner::resume(&batch, &root, &env(&url)).unwrap();
    assert!(second["stop_reason"].is_null(), "{second}");
    assert_eq!(second["completed_sessions"], 18);
    assert!(
        second["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["clean"] == true)
    );
}
