// ABOUTME: Host-side supervision of one native worker process per seat: spawn, deadlines, framing,
// ABOUTME: kill/reap, the table-wide budget ledger, and the SubprocessAgent used by the sync engine.
//
// Layout: a dedicated thread owns a current-thread Tokio runtime and every child process. The
// synchronous engine thread talks to it through a bounded channel, one exchange at a time.
// Budget, correlation and lifecycle state stay on the engine thread in `Shared`.
use crate::{
    arena::Agent,
    engine::{Action, PlayerView},
    openrouter::Stats,
    participant::{
        ErrorClass, FRAME_LIMIT, Identity, Message, NoteResult, Outcome, Policy, Request, Response,
        Usage, VERSION,
    },
    private_fs,
    worker::KEY_VARIABLE,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Write,
    path::PathBuf,
    process::Stdio,
    rc::Rc,
    sync::{Arc, Mutex, Once},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::{Notify, mpsc, oneshot},
    time::timeout,
};

/// Retained stderr per worker per session; more is a resource failure.
pub const STDERR_LIMIT: usize = 1_048_576;
/// Variables forwarded to model workers when the host has them; everything else is cleared.
const NETWORK_VARIABLES: [&str; 10] = [
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "all_proxy",
    "no_proxy",
];

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Deadlines {
    pub init: Duration,
    /// StartHand and Shutdown acknowledgements.
    pub lifecycle: Duration,
    /// Added to the provider timeout for Decide/HandEnd; the whole budget for scripted workers.
    pub operation_grace: Duration,
    /// Time allowed for a normal exit after Shutdown is acknowledged.
    pub exit: Duration,
    /// Time allowed to reap a worker after its process group is killed.
    pub reap: Duration,
}
impl Default for Deadlines {
    fn default() -> Self {
        Self {
            init: Duration::from_secs(5),
            lifecycle: Duration::from_secs(2),
            operation_grace: Duration::from_secs(5),
            exit: Duration::from_secs(2),
            reap: Duration::from_secs(2),
        }
    }
}

/// The executable for a worker. The CLI always uses `Launch::current_exe`; nothing here is
/// read from user configuration or passed through a shell.
#[derive(Clone, Debug)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
}
impl Launch {
    pub fn current_exe() -> Result<Self, String> {
        Ok(Self {
            program: std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?,
            args: vec!["participant".into()],
        })
    }
}

pub struct Options {
    pub deadlines: Deadlines,
    /// When set, `workers/seat-N/` private directories and `participants.jsonl` are written here.
    pub run_dir: Option<PathBuf>,
    pub max_requests: u64,
    pub spend_limit_usd: f64,
    /// Supplied only to model workers, only through their environment.
    pub key: Option<String>,
    /// Table-wide ledger, shared with the runner's summary.
    pub stats: Rc<RefCell<Stats>>,
}
impl Options {
    /// A scripted-only table: no credential, no budget, no artifacts.
    pub fn scripted() -> Self {
        Self {
            deadlines: Deadlines::default(),
            run_dir: None,
            max_requests: 0,
            spend_limit_usd: 0.0,
            key: None,
            stats: Rc::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub kind: &'static str,
    pub detail: String,
}
impl Fault {
    fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    /// The host killed the process group rather than observing a voluntary exit.
    pub killed: bool,
    /// The direct child was waited for; false means cleanup failed.
    pub reaped: bool,
    pub stderr_bytes: usize,
    pub stderr_overflow: bool,
}

enum Command {
    Spawn {
        seat: usize,
        launch: Launch,
        dir: PathBuf,
        keep_stderr: bool,
        env: Vec<(String, String)>,
        reply: oneshot::Sender<Result<u32, String>>,
    },
    Exchange {
        seat: usize,
        frame: String,
        deadline: Duration,
        reply: oneshot::Sender<Result<String, (Fault, Exit)>>,
    },
    /// Wait up to `wait` for a voluntary exit (None: kill immediately), then kill and reap.
    Stop {
        seat: usize,
        wait: Option<Duration>,
        reply: oneshot::Sender<Exit>,
    },
    Quit,
}

struct Proc {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: tokio::task::JoinHandle<(Vec<u8>, bool)>,
    overflow: Arc<Notify>,
    pid: i32,
    stderr_path: Option<PathBuf>,
}

/// Process groups of live workers, so an interrupt can kill them before the host exits.
static LIVE_GROUPS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

fn kill_group(pid: i32) {
    // SAFETY: kill(2) has no memory-safety preconditions. A negative pid addresses the process
    // group this host created for the worker; the group id is only used while the direct child
    // is unreaped, so it cannot have been recycled.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}

/// Opt-in for CLI runs: on Ctrl-C, kill every live worker group and exit with status 130.
/// Workers also exit on stdin end-of-file, which covers hosts that die without this handler.
pub fn install_interrupt_cleanup() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                if tokio::signal::ctrl_c().await.is_ok() {
                    for pid in LIVE_GROUPS.lock().map(|g| g.clone()).unwrap_or_default() {
                        kill_group(pid);
                    }
                    eprintln!("interrupted: worker processes killed");
                    std::process::exit(130);
                }
            });
        });
    });
}

async fn spawn(
    launch: &Launch,
    dir: PathBuf,
    keep_stderr: bool,
    env: Vec<(String, String)>,
) -> Result<Proc, String> {
    let mut command = tokio::process::Command::new(&launch.program);
    command
        .args(&launch.args)
        .env_clear()
        .envs(env)
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not start worker: {e}"))?;
    let pid = child.id().ok_or("worker exited during startup")? as i32;
    LIVE_GROUPS.lock().map_err(|_| "poisoned")?.push(pid);
    let stdin = child.stdin.take();
    let stdout = BufReader::new(child.stdout.take().ok_or("no worker stdout")?);
    let mut stderr = child.stderr.take().ok_or("no worker stderr")?;
    let overflow = Arc::new(Notify::new());
    let notify = overflow.clone();
    // Drain continuously so a full stderr pipe can never block a worker mid-request.
    let stderr = tokio::spawn(async move {
        let mut kept = Vec::new();
        let mut overflowed = false;
        let mut chunk = [0u8; 8192];
        while let Ok(n) = stderr.read(&mut chunk).await {
            if n == 0 {
                break;
            }
            let room = STDERR_LIMIT - kept.len();
            kept.extend_from_slice(&chunk[..n.min(room)]);
            if n > room && !overflowed {
                overflowed = true;
                notify.notify_one();
            }
        }
        (kept, overflowed)
    });
    Ok(Proc {
        child,
        stdin,
        stdout,
        stderr,
        overflow,
        pid,
        stderr_path: keep_stderr.then(|| dir.join("stderr.log")),
    })
}

/// Reads one frame with a hard length bound; never an unbounded `read_until`.
async fn read_frame(stdout: &mut BufReader<ChildStdout>) -> Result<String, Fault> {
    let mut frame = Vec::new();
    loop {
        let available = stdout
            .fill_buf()
            .await
            .map_err(|e| Fault::new("io", e.to_string()))?;
        if available.is_empty() {
            return Err(if frame.is_empty() {
                Fault::new("eof", "worker closed stdout before responding")
            } else {
                Fault::new("partial_frame", "worker closed stdout inside a response")
            });
        }
        let newline = available.iter().position(|b| *b == b'\n');
        let take = newline.map_or(available.len(), |i| i + 1);
        if frame.len() + take > FRAME_LIMIT {
            return Err(Fault::new("oversized_frame", "response exceeds 1 MiB"));
        }
        frame.extend_from_slice(&available[..take]);
        stdout.consume(take);
        if newline.is_some() {
            frame.pop();
            return String::from_utf8(frame)
                .map_err(|_| Fault::new("invalid_utf8", "response is not UTF-8"));
        }
    }
}

async fn exchange(proc: &mut Proc, frame: &str, deadline: Duration) -> Result<String, Fault> {
    let Proc {
        stdin,
        stdout,
        overflow,
        ..
    } = proc;
    // Nothing may be waiting on stdout before a request: that would be an unsolicited line or
    // a response to an earlier request, and must never be matched to this one.
    tokio::select! {
        biased;
        pending = stdout.fill_buf() => {
            return Err(match pending {
                Ok([]) => Fault::new("eof", "worker exited between requests"),
                Ok(_) => Fault::new("unsolicited_output", "worker wrote without a request"),
                Err(e) => Fault::new("io", e.to_string()),
            });
        }
        _ = std::future::ready(()) => {}
    }
    let round_trip = async {
        let stdin = stdin
            .as_mut()
            .ok_or_else(|| Fault::new("eof", "worker stdin closed"))?;
        let write = async {
            stdin.write_all(frame.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        };
        write
            .await
            .map_err(|e| Fault::new("io", format!("write to worker failed: {e}")))?;
        read_frame(stdout).await
    };
    tokio::select! {
        biased;
        _ = overflow.notified() => Err(Fault::new("stderr_overflow", "worker exceeded 1 MiB of stderr")),
        result = timeout(deadline, round_trip) => result.unwrap_or_else(|_| {
            Err(Fault::new("deadline", format!("no complete response within {deadline:?}")))
        }),
    }
}

/// Ends a worker: optional grace for a voluntary exit, then kill the group and reap the child.
async fn finish(mut proc: Proc, wait: Option<Duration>, reap: Duration) -> Exit {
    drop(proc.stdin.take());
    let mut status = match wait {
        Some(grace) => timeout(grace, proc.child.wait())
            .await
            .ok()
            .and_then(Result::ok),
        None => None,
    };
    let killed = status.is_none();
    if killed {
        kill_group(proc.pid);
        status = timeout(reap, proc.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
    }
    if status.is_some()
        && let Ok(mut groups) = LIVE_GROUPS.lock()
    {
        groups.retain(|pid| *pid != proc.pid);
    }
    let (stderr, stderr_overflow) = timeout(reap, &mut proc.stderr)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    if let Some(path) = &proc.stderr_path
        && let Ok(mut file) = private_fs::create_file(path)
    {
        let _ = file.write_all(&stderr);
    }
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.and_then(|s| s.signal())
    };
    #[cfg(not(unix))]
    let signal = None;
    Exit {
        code: status.and_then(|s| s.code()),
        signal,
        killed,
        reaped: status.is_some(),
        stderr_bytes: stderr.len(),
        stderr_overflow,
    }
}

fn serve(mut commands: mpsc::Receiver<Command>, reap: Duration) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("supervisor runtime");
    runtime.block_on(async move {
        let mut procs: HashMap<usize, Proc> = HashMap::new();
        while let Some(command) = commands.recv().await {
            match command {
                Command::Spawn {
                    seat,
                    launch,
                    dir,
                    keep_stderr,
                    env,
                    reply,
                } => {
                    let result = spawn(&launch, dir, keep_stderr, env).await.map(|proc| {
                        let pid = proc.pid as u32;
                        procs.insert(seat, proc);
                        pid
                    });
                    let _ = reply.send(result);
                }
                Command::Exchange {
                    seat,
                    frame,
                    deadline,
                    reply,
                } => {
                    let result = match procs.get_mut(&seat) {
                        Some(proc) => exchange(proc, &frame, deadline).await,
                        None => Err(Fault::new("eof", "no live worker for seat")),
                    };
                    let result = match result {
                        Ok(frame) => Ok(frame),
                        // Any transport fault is terminal for that worker: kill and reap now so
                        // a late response can never be read on a later turn.
                        Err(fault) => {
                            let exit = match procs.remove(&seat) {
                                Some(proc) => finish(proc, None, reap).await,
                                None => Exit::default(),
                            };
                            Err((fault, exit))
                        }
                    };
                    let _ = reply.send(result);
                }
                Command::Stop { seat, wait, reply } => {
                    let exit = match procs.remove(&seat) {
                        Some(proc) => finish(proc, wait, reap).await,
                        None => Exit::default(),
                    };
                    let _ = reply.send(exit);
                }
                Command::Quit => break,
            }
        }
        for (_, proc) in procs.drain() {
            finish(proc, None, reap).await;
        }
    });
}

struct Seat {
    policy: &'static str,
    is_model: bool,
    notes: bool,
    operation_deadline: Duration,
    /// Provider attempts one operation may use (the retry budget the host grants).
    attempts: u32,
    pid: u32,
    next_id: u64,
    hand: Option<usize>,
    dead: Option<String>,
    exit: Option<Exit>,
    wire: Option<File>,
    usage: Stats,
}

/// Engine-thread state shared by every SubprocessAgent of one session.
struct Shared {
    commands: mpsc::Sender<Command>,
    session: String,
    deadlines: Deadlines,
    max_requests: u64,
    spend_limit_usd: f64,
    stats: Rc<RefCell<Stats>>,
    seats: BTreeMap<usize, Seat>,
    log: Option<File>,
    contaminated: bool,
    failures: Vec<Value>,
}

impl Shared {
    fn record(&mut self, value: Value) {
        let Some(log) = &mut self.log else { return };
        if writeln!(log, "{value}").and_then(|_| log.flush()).is_err() {
            self.log = None;
            self.contaminated = true;
            self.halt("lifecycle_log_write_failed");
        }
    }
    fn halt(&mut self, reason: &str) {
        self.stats.borrow_mut().halt_reason = Some(reason.into());
    }
    fn send<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<T>) -> Command,
    ) -> Result<T, &'static str> {
        let (reply, response) = oneshot::channel();
        self.commands
            .blocking_send(command(reply))
            .map_err(|_| "supervisor thread is gone")?;
        response
            .blocking_recv()
            .map_err(|_| "supervisor thread is gone")
    }
    /// Marks a worker dead after a fatal fault. The session is contaminated and no further
    /// provider calls are granted to any seat.
    fn fail(&mut self, seat: usize, fault: &Fault, exit: Exit) {
        let reason = format!("{}: {}", fault.kind, fault.detail);
        self.contaminated = true;
        self.halt(&format!("participant_failure:seat {seat}:{}", fault.kind));
        let failure = json!({"kind":"failure","seat":seat,"fault":fault.kind,"detail":fault.detail,"exit":exit});
        self.failures.push(failure.clone());
        self.record(failure);
        let state = self.seats.get_mut(&seat).expect("known seat");
        state.dead = Some(reason);
        state.exit = Some(exit);
    }
    /// Kills a worker whose transport was intact but whose message violated the protocol.
    fn reject(&mut self, seat: usize, fault: Fault) -> Fault {
        let exit = self
            .send(|reply| Command::Stop {
                seat,
                wait: None,
                reply,
            })
            .unwrap_or_default();
        self.fail(seat, &fault, exit);
        fault
    }
    /// One correlated request/response exchange. Any error returned here is fatal for the
    /// worker, which has already been killed and reaped.
    fn request(
        &mut self,
        seat: usize,
        message: Message,
        deadline: Duration,
        allowance: u32,
    ) -> Result<Response, Fault> {
        let operation = message.operation();
        let state = self.seats.get_mut(&seat).expect("known seat");
        if let Some(reason) = &state.dead {
            return Err(Fault::new("dead", reason.clone()));
        }
        let identity = Identity {
            version: VERSION,
            id: state.next_id,
            session: self.session.clone(),
            seat,
            hand: if matches!(message, Message::Init { .. } | Message::Shutdown) {
                None
            } else {
                state.hand
            },
        };
        state.next_id += 1;
        let frame = serde_json::to_string(&Request {
            identity: identity.clone(),
            message,
        })
        .expect("serializable request");
        if frame.len() + 1 > FRAME_LIMIT {
            return Err(self.reject(
                seat,
                Fault::new("request_too_large", "request exceeds 1 MiB"),
            ));
        }
        if allowance > 0 {
            // Written before the exchange so a granted provider operation is on record even if
            // the worker, its trace, or the host is lost before any response.
            self.record(json!({"kind":"dispatch","seat":seat,"id":identity.id,"hand":identity.hand,"operation":operation,"allowance":allowance}));
        }
        let state = self.seats.get_mut(&seat).expect("known seat");
        if let Some(wire) = &mut state.wire {
            let _ = writeln!(wire, "{{\"to_worker\":{frame}}}");
        }
        let started = Instant::now();
        let reply = self.send(|reply| Command::Exchange {
            seat,
            frame,
            deadline,
            reply,
        });
        let elapsed_us = started.elapsed().as_micros() as u64;
        let mut entry = json!({"kind":"exchange","seat":seat,"id":identity.id,"hand":identity.hand,
            "operation":operation,"allowance":allowance,"elapsed_us":elapsed_us});
        let raw = match reply {
            Ok(Ok(raw)) => raw,
            Ok(Err((fault, exit))) => {
                entry["outcome"] = json!("transport_error");
                entry["detail"] = json!(format!("{}: {}", fault.kind, fault.detail));
                self.record(entry);
                self.fail(seat, &fault, exit);
                return Err(fault);
            }
            Err(gone) => {
                let fault = Fault::new("io", gone);
                self.fail(seat, &fault, Exit::default());
                return Err(fault);
            }
        };
        let state = self.seats.get_mut(&seat).expect("known seat");
        if let Some(wire) = &mut state.wire {
            let _ = match serde_json::from_str::<Value>(&raw) {
                Ok(_) => writeln!(wire, "{{\"from_worker\":{raw}}}"),
                Err(_) => writeln!(wire, "{}", json!({"from_worker_unparsed":raw})),
            };
            let _ = wire.flush();
        }
        let fault = match serde_json::from_str::<Response>(&raw) {
            Err(e) => Some(Fault::new("malformed_response", e.to_string())),
            Ok(response) if response.identity != identity => Some(Fault::new(
                "wrong_correlation",
                format!("expected {identity:?}, got {:?}", response.identity),
            )),
            Ok(response) => {
                entry["outcome"] = json!(response.outcome.name());
                if let Outcome::Error { class, detail } = &response.outcome {
                    entry["error_class"] = json!(class);
                    entry["detail"] = json!(detail);
                }
                entry["usage"] = json!(response.usage);
                self.record(entry);
                return Ok(response);
            }
        };
        let fault = fault.expect("set above");
        entry["outcome"] = json!("protocol_error");
        entry["detail"] = json!(format!("{}: {}", fault.kind, fault.detail));
        self.record(entry);
        Err(self.reject(seat, fault))
    }

    /// Reserves up to `wanted` provider request slots (one operation's attempts), or returns 0
    /// when no further call may be made. Unused slots are refunded when the worker reports.
    fn reserve(&mut self, wanted: u32) -> u32 {
        let mut stats = self.stats.borrow_mut();
        if stats.halt_reason.is_some() || stats.requests >= self.max_requests {
            return 0;
        }
        let room = (self.max_requests - stats.requests).min(u64::from(wanted)) as u32;
        let granted = stats.global.reserve_up_to(room);
        if granted == 0 {
            stats.halt_reason = Some(stats.global.halt_reason().unwrap_or("global_budget").into());
            return 0;
        }
        stats.requests += u64::from(granted);
        granted
    }
    /// A granted operation whose response never arrived intact: the slot stays charged and the
    /// cost is unknown. Killing a worker does not cancel provider billing.
    fn lost(&mut self, seat: usize, allowance: u32) {
        if allowance == 0 {
            return;
        }
        self.stats.borrow_mut().unknown_cost_requests += 1;
        self.stats.borrow().global.settle(None);
        let state = self.seats.get_mut(&seat).expect("known seat");
        // Every granted slot stays charged: how many attempts the lost worker made is unknown.
        state.usage.requests += u64::from(allowance);
        state.usage.unknown_cost_requests += 1;
    }
    /// Applies a worker's reported usage to the table ledger before any further grant.
    fn reconcile(
        &mut self,
        seat: usize,
        usage: &Usage,
        allowance: u32,
        notebook: bool,
    ) -> Result<(), Fault> {
        if usage.attempts > allowance
            || usage.unknown_cost_attempts > usage.attempts
            || usage.infra_failures > usage.attempts
            || !usage.reported_cost_usd.is_finite()
            || usage.reported_cost_usd < 0.0
            || !usage.assumed_cost_usd.is_finite()
            || usage.assumed_cost_usd < 0.0
            || (usage.attempts == 0
                && (usage.reported_cost_usd != 0.0
                    || usage.assumed_cost_usd != 0.0
                    || usage.infra_failures != 0
                    || usage.prompt_tokens + usage.completion_tokens != 0))
        {
            return Err(Fault::new(
                "usage_violation",
                format!("reported {usage:?} against a grant of {allowance}"),
            ));
        }
        let failed_note = usage.note == NoteResult::Failed;
        let attempts = u64::from(usage.attempts);
        let state = self.seats.get_mut(&seat).expect("known seat");
        for stats in [&mut *self.stats.borrow_mut(), &mut state.usage] {
            stats.prompt_tokens += usage.prompt_tokens;
            stats.completion_tokens += usage.completion_tokens;
            stats.reported_cost_usd += usage.reported_cost_usd;
            stats.assumed_cost_usd += usage.assumed_cost_usd;
            stats.infra_failures += u64::from(usage.infra_failures);
            stats.unknown_cost_requests += u64::from(usage.unknown_cost_attempts);
            stats.memory_errors += u64::from(failed_note);
            stats.normalized_actions += u64::from(usage.normalized);
            stats.repaired_actions += u64::from(usage.repaired);
            if notebook {
                stats.memory_requests += attempts;
            }
        }
        state.usage.requests += attempts;
        let mut stats = self.stats.borrow_mut();
        // Refund reserved slots only on an acknowledged response proving no attempt was made.
        stats.requests -= u64::from(allowance - usage.attempts);
        if allowance > usage.attempts {
            stats.global.refund(allowance - usage.attempts);
        }
        if attempts > 0 {
            stats.global.settle(
                (usage.unknown_cost_attempts == 0)
                    .then_some(usage.reported_cost_usd + usage.assumed_cost_usd),
            );
        }
        // Same precedence as the in-process adapter: later reasons overwrite earlier ones.
        if usage.unknown_cost_attempts > 0 {
            stats.halt_reason = Some("unknown_request_cost".into());
        } else if attempts > 0
            && stats.reported_cost_usd + stats.assumed_cost_usd >= self.spend_limit_usd
        {
            stats.halt_reason = Some("reported_spend_limit".into());
        }
        if let Some(reason) = &usage.halt {
            stats.halt_reason = Some(reason.clone());
        }
        if attempts > 0 && stats.requests >= self.max_requests {
            stats.halt_reason = Some("request_limit".into());
        }
        Ok(())
    }
    fn fallback(&mut self, seat: usize) {
        self.stats.borrow_mut().fallbacks += 1;
        self.seats
            .get_mut(&seat)
            .expect("known seat")
            .usage
            .fallbacks += 1;
    }

    fn decide(&mut self, seat: usize, view: &PlayerView) -> Result<Action, String> {
        let state = &self.seats[&seat];
        if let Some(reason) = &state.dead {
            let error = format!("participant_dead:{reason}");
            self.fallback(seat);
            return Err(error);
        }
        let (is_model, deadline, wanted) =
            (state.is_model, state.operation_deadline, state.attempts);
        let allowance = if is_model { self.reserve(wanted) } else { 0 };
        if is_model && allowance == 0 {
            // No grant, no provider call, no round trip: recorded as a fallback in the replay.
            self.fallback(seat);
            return Err("openrouter:Budget".into());
        }
        let message = Message::Decide {
            view: view.clone(),
            allowance,
        };
        let response = match self.request(seat, message, deadline, allowance) {
            Ok(response) => response,
            Err(fault) => {
                self.lost(seat, allowance);
                self.fallback(seat);
                return Err(format!("participant_transport:{}", fault.kind));
            }
        };
        if let Err(fault) = self.reconcile(seat, &response.usage, allowance, false) {
            let fault = self.reject(seat, fault);
            self.lost(seat, allowance);
            self.fallback(seat);
            return Err(format!("participant_transport:{}", fault.kind));
        }
        let fault = match response.outcome {
            Outcome::Action { action } => return Ok(action),
            Outcome::Error {
                class: ErrorClass::Lifecycle,
                detail,
            } => Fault::new("lifecycle_error", detail),
            // Infrastructure alone prevented the decision: the hand is abandoned, not played on.
            Outcome::Error { detail, .. } if detail == crate::arena::OUTAGE => {
                self.stats.borrow_mut().outage_decisions += 1;
                return Err(detail);
            }
            Outcome::Error { detail, .. } => {
                self.fallback(seat);
                return Err(detail);
            }
            other => Fault::new("unexpected_outcome", other.name()),
        };
        let fault = self.reject(seat, fault);
        self.fallback(seat);
        Err(format!("participant_transport:{}", fault.kind))
    }

    fn acknowledge(&mut self, seat: usize, response: Response) -> Result<(), String> {
        let fault = match response.outcome {
            Outcome::Ack => return Ok(()),
            Outcome::Error {
                class: ErrorClass::Memory,
                detail,
            } => return Err(detail),
            Outcome::Error { detail, .. } => Fault::new("lifecycle_error", detail),
            other => Fault::new("unexpected_outcome", other.name()),
        };
        let fault = self.reject(seat, fault);
        Err(format!("participant_transport:{}", fault.kind))
    }
    fn start_hand(&mut self, seat: usize, number: usize) -> Result<(), String> {
        let state = self.seats.get_mut(&seat).expect("known seat");
        if let Some(reason) = &state.dead {
            return Err(format!("participant_dead:{reason}"));
        }
        state.hand = Some(number);
        let deadline = self.deadlines.lifecycle;
        let response = self
            .request(seat, Message::StartHand, deadline, 0)
            .map_err(|fault| format!("participant_transport:{}", fault.kind))?;
        if let Err(fault) = self.reconcile(seat, &response.usage, 0, false) {
            return Err(format!(
                "participant_transport:{}",
                self.reject(seat, fault).kind
            ));
        }
        self.acknowledge(seat, response)
    }
    fn finish_hand(&mut self, seat: usize, view: &PlayerView) -> Result<(), String> {
        let state = &self.seats[&seat];
        if let Some(reason) = &state.dead {
            return Err(format!("participant_dead:{reason}"));
        }
        let (wants_note, deadline, wanted) = (
            state.is_model && state.notes,
            state.operation_deadline,
            state.attempts,
        );
        let allowance = if wants_note { self.reserve(wanted) } else { 0 };
        let message = Message::HandEnd {
            view: view.clone(),
            allowance,
        };
        let response = match self.request(seat, message, deadline, allowance) {
            Ok(response) => response,
            Err(fault) => {
                self.lost(seat, allowance);
                return Err(format!("participant_transport:{}", fault.kind));
            }
        };
        if let Err(fault) = self.reconcile(seat, &response.usage, allowance, true) {
            let fault = self.reject(seat, fault);
            self.lost(seat, allowance);
            return Err(format!("participant_transport:{}", fault.kind));
        }
        self.acknowledge(seat, response)
    }
}

/// The engine-side handle for one worker. All state lives in the session's `Shared`.
pub struct SubprocessAgent {
    seat: usize,
    shared: Rc<RefCell<Shared>>,
}
impl Agent for SubprocessAgent {
    fn decide(&mut self, view: &PlayerView) -> Result<Action, String> {
        self.shared.borrow_mut().decide(self.seat, view)
    }
    fn start_hand(&mut self, number: usize) -> Result<(), String> {
        self.shared.borrow_mut().start_hand(self.seat, number)
    }
    fn finish_hand(&mut self, view: &PlayerView) -> Result<(), String> {
        self.shared.borrow_mut().finish_hand(self.seat, view)
    }
}

pub struct Supervisor {
    shared: Rc<RefCell<Shared>>,
    thread: Option<JoinHandle<()>>,
    run_dir: Option<PathBuf>,
    key: Option<String>,
}

impl Supervisor {
    pub fn start(options: Options) -> Result<Self, String> {
        let log = match &options.run_dir {
            Some(dir) => {
                private_fs::create_dir(dir.join("workers"))
                    .map_err(|e| format!("workers directory: {e}"))?;
                Some(
                    private_fs::create_file(dir.join("participants.jsonl"))
                        .map_err(|e| format!("participants.jsonl: {e}"))?,
                )
            }
            None => None,
        };
        // Bounded: the engine thread issues one command at a time and waits for its reply.
        let (commands, receiver) = mpsc::channel(4);
        let reap = options.deadlines.reap;
        let thread = std::thread::Builder::new()
            .name("participant-supervisor".into())
            .spawn(move || serve(receiver, reap))
            .map_err(|e| format!("supervisor thread: {e}"))?;
        // Unique per run and unrelated to any deck or session seed.
        let session = format!(
            "{:x}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        );
        Ok(Self {
            shared: Rc::new(RefCell::new(Shared {
                commands,
                session,
                deadlines: options.deadlines,
                max_requests: options.max_requests,
                spend_limit_usd: options.spend_limit_usd,
                stats: options.stats,
                seats: BTreeMap::new(),
                log,
                contaminated: false,
                failures: vec![],
            })),
            thread: Some(thread),
            run_dir: options.run_dir,
            key: options.key,
        })
    }

    /// Starts and initializes one worker. Call for every seat before the first hand; on error
    /// drop the supervisor, which kills and reaps every worker already started.
    pub fn spawn(
        &mut self,
        seat: usize,
        policy: Policy,
        launch: Launch,
    ) -> Result<SubprocessAgent, String> {
        let mut shared = self.shared.borrow_mut();
        if shared.seats.contains_key(&seat) {
            return Err(format!("seat {seat} already has a worker"));
        }
        let (dir, wire) = match &self.run_dir {
            Some(run) => {
                let dir = run.join("workers").join(format!("seat-{seat}"));
                private_fs::create_dir(&dir).map_err(|e| format!("worker directory: {e}"))?;
                let wire = private_fs::create_file(dir.join("wire.jsonl"))
                    .map_err(|e| format!("wire trace: {e}"))?;
                (dir, Some(wire))
            }
            None if policy.is_model() => {
                return Err("model workers need a run directory for private traces".into());
            }
            None => (std::env::temp_dir(), None),
        };
        let mut env = vec![];
        let mut operation_deadline = shared.deadlines.operation_grace;
        let mut notes = false;
        let mut attempts = 0;
        if let Policy::OpenRouter(settings) = &policy {
            let key = self.key.clone().ok_or("no credential for model worker")?;
            env.push((KEY_VARIABLE.to_owned(), key));
            for name in NETWORK_VARIABLES {
                if let Ok(value) = std::env::var(name) {
                    env.push((name.to_owned(), value));
                }
            }
            // Every attempt may time out and every backoff may run long; the worker retries
            // inside one operation, so the deadline covers all of it.
            operation_deadline += Duration::from_millis(crate::openrouter::worst_case_ms(
                settings.max_attempts,
                settings.timeout_ms,
                settings.retry_backoff_ms,
                settings.retry_backoff_max_ms,
            ));
            attempts = settings.max_attempts.max(1);
            notes = settings.memory.updates();
        }
        let started = Instant::now();
        let executable = launch.program.display().to_string();
        let pid = shared
            .send(|reply| Command::Spawn {
                seat,
                launch,
                dir,
                keep_stderr: wire.is_some(),
                env,
                reply,
            })
            .map_err(str::to_owned)??;
        shared.seats.insert(
            seat,
            Seat {
                policy: policy.name(),
                is_model: policy.is_model(),
                notes,
                operation_deadline,
                attempts,
                pid,
                next_id: 1,
                hand: None,
                dead: None,
                exit: None,
                wire,
                usage: Stats::default(),
            },
        );
        let deadline = shared.deadlines.init;
        let policy_name = policy.name();
        let response = shared
            .request(seat, Message::Init { policy }, deadline, 0)
            .map_err(|fault| {
                format!(
                    "seat {seat} initialization: {}: {}",
                    fault.kind, fault.detail
                )
            })?;
        let Outcome::Ready { crate_version } = &response.outcome else {
            let fault = shared.reject(
                seat,
                Fault::new("init_rejected", format!("{:?}", response.outcome)),
            );
            return Err(format!("seat {seat} initialization: {}", fault.detail));
        };
        shared.record(json!({"kind":"spawn","seat":seat,"pid":pid,"policy":policy_name,"executable":executable,
            "worker_crate_version":crate_version,"protocol":VERSION,"init_us":started.elapsed().as_micros() as u64,"restarts":0}));
        Ok(SubprocessAgent {
            seat,
            shared: self.shared.clone(),
        })
    }

    /// Shuts every worker down: Shutdown/ack, a bounded wait for exit, then kill and reap.
    /// `Err` means a process could not be reaped and the run must abort.
    pub fn shutdown(&mut self) -> Result<(), String> {
        let mut shared = self.shared.borrow_mut();
        let seats: Vec<usize> = shared.seats.keys().copied().collect();
        let mut unreaped = vec![];
        for seat in seats {
            if shared.seats[&seat].exit.is_none() {
                let (lifecycle, exit_wait) = (shared.deadlines.lifecycle, shared.deadlines.exit);
                let acknowledged = shared
                    .request(seat, Message::Shutdown, lifecycle, 0)
                    .map(|response| response.outcome == Outcome::Ack);
                match acknowledged {
                    Err(_) => {} // already killed, reaped and recorded by `request`
                    Ok(false) => {
                        shared.reject(
                            seat,
                            Fault::new("unexpected_outcome", "shutdown not acknowledged"),
                        );
                    }
                    Ok(true) => {
                        let exit = shared
                            .send(|reply| Command::Stop {
                                seat,
                                wait: Some(exit_wait),
                                reply,
                            })
                            .unwrap_or_default();
                        if exit.killed || exit.code != Some(0) {
                            let fault = Fault::new(
                                "unclean_exit",
                                "worker did not exit cleanly after shutdown",
                            );
                            shared.fail(seat, &fault, exit);
                        } else {
                            shared.seats.get_mut(&seat).expect("known seat").exit = Some(exit);
                        }
                    }
                }
            }
            let state = &shared.seats[&seat];
            let exit = state.exit.clone().unwrap_or_default();
            let record =
                json!({"kind":"exit","seat":seat,"pid":state.pid,"exit":exit,"dead":state.dead});
            if !exit.reaped {
                unreaped.push(seat);
            }
            shared.record(record);
        }
        if unreaped.is_empty() {
            Ok(())
        } else {
            Err(format!("could not reap workers for seats {unreaped:?}"))
        }
    }

    /// True once any transport, protocol, lifecycle-log or cleanup failure has occurred.
    pub fn contaminated(&self) -> bool {
        self.shared.borrow().contaminated
    }
    pub fn pid(&self, seat: usize) -> Option<u32> {
        self.shared.borrow().seats.get(&seat).map(|s| s.pid)
    }
    /// Per-seat identity, usage and exit status for the run summary. No cards or credentials.
    pub fn report(&self) -> Value {
        let shared = self.shared.borrow();
        let seats: Vec<Value> = shared
            .seats
            .iter()
            .map(|(seat, s)| {
                json!({"seat":seat,"policy":s.policy,"pid":s.pid,"usage":s.usage,"dead":s.dead,"exit":s.exit,"restarts":0})
            })
            .collect();
        json!({"protocol":VERSION,"session":shared.session,"contaminated":shared.contaminated,
            "deadlines_ms":{"init":shared.deadlines.init.as_millis() as u64,"lifecycle":shared.deadlines.lifecycle.as_millis() as u64,
                "operation_grace":shared.deadlines.operation_grace.as_millis() as u64,"exit":shared.deadlines.exit.as_millis() as u64,
                "reap":shared.deadlines.reap.as_millis() as u64},
            "failures":shared.failures,"seats":seats})
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Covers host errors and early returns: the thread kills and reaps whatever is left.
        if let Ok(shared) = self.shared.try_borrow() {
            let _ = shared.commands.blocking_send(Command::Quit);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
