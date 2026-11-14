// ABOUTME: Overhead benchmark: identical scripted tables on the in-process and subprocess backends.
// ABOUTME: Checks the two backends replay identically and reports startup and per-decision latency.
use crate::{
    arena::{Agent, Record, Session},
    engine::{Action, PlayerView},
    participant::Policy,
    supervisor::{Launch, Options, Supervisor},
    worker,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, time::Instant};

/// Fixed six-seat field; deterministic policies only, so both backends must agree exactly.
pub const FIELD: [Policy; 6] = [
    Policy::Tight,
    Policy::Loose,
    Policy::Aggressive,
    Policy::Caller,
    Policy::Tight,
    Policy::Loose,
];

/// Wraps an agent and records the wall-clock time of every decision in microseconds.
struct Timed {
    inner: Box<dyn Agent>,
    samples: Rc<RefCell<Vec<u64>>>,
}
impl Agent for Timed {
    fn decide(&mut self, view: &PlayerView) -> Result<Action, String> {
        let started = Instant::now();
        let result = self.inner.decide(view);
        self.samples
            .borrow_mut()
            .push(started.elapsed().as_micros() as u64);
        result
    }
    fn start_hand(&mut self, number: usize) -> Result<(), String> {
        self.inner.start_hand(number)
    }
    fn finish_hand(&mut self, view: &PlayerView) -> Result<(), String> {
        self.inner.finish_hand(view)
    }
}

fn timed(agents: Vec<Box<dyn Agent>>, samples: &Rc<RefCell<Vec<u64>>>) -> Vec<Box<dyn Agent>> {
    agents
        .into_iter()
        .map(|inner| {
            Box::new(Timed {
                inner,
                samples: samples.clone(),
            }) as Box<dyn Agent>
        })
        .collect()
}

fn play(seed: u64, hands: usize, agents: &mut [Box<dyn Agent>]) -> Result<Vec<Record>, String> {
    let mut session = Session::new(agents.len(), seed)?;
    let mut records = vec![];
    for _ in 0..hands {
        let played = session.next_hand(agents)?;
        if !played.lifecycle_errors.is_empty() {
            return Err(format!("lifecycle errors: {:?}", played.lifecycle_errors));
        }
        records.push(played.record);
    }
    Ok(records)
}

fn summary(samples: &mut [u64]) -> Value {
    samples.sort_unstable();
    let at = |q: f64| {
        samples
            .get(((samples.len() as f64 * q).ceil() as usize).saturating_sub(1))
            .copied()
    };
    json!({"count":samples.len(),"median_us":at(0.5),"p95_us":at(0.95),"p99_us":at(0.99),"max_us":samples.last()})
}

/// Runs `sessions` seeded sessions of `hands` hands on both backends and compares every record.
pub fn run(sessions: usize, hands: usize, launch: &Launch) -> Result<Value, String> {
    let (local_decisions, remote_decisions) = (Rc::default(), Rc::default());
    let (mut startups, mut shutdowns) = (vec![], vec![]);
    let (mut local_sessions, mut remote_sessions) = (vec![], vec![]);
    let mut total_hands = 0;
    for index in 0..sessions {
        let seed = index as u64 + 1;
        let started = Instant::now();
        let locals = FIELD.iter().filter_map(worker::scripted).collect();
        let expected = play(seed, hands, &mut timed(locals, &local_decisions))?;
        local_sessions.push(started.elapsed().as_micros() as u64);

        let started = Instant::now();
        let mut supervisor = Supervisor::start(Options::scripted())?;
        let mut remotes: Vec<Box<dyn Agent>> = vec![];
        for (seat, policy) in FIELD.iter().enumerate() {
            remotes.push(Box::new(supervisor.spawn(
                seat,
                policy.clone(),
                launch.clone(),
            )?));
        }
        startups.push(started.elapsed().as_micros() as u64);
        let played = Instant::now();
        let actual = play(seed, hands, &mut timed(remotes, &remote_decisions))?;
        remote_sessions.push(played.elapsed().as_micros() as u64);
        let stopping = Instant::now();
        supervisor.shutdown()?;
        shutdowns.push(stopping.elapsed().as_micros() as u64);
        if supervisor.contaminated() {
            return Err(format!(
                "session {seed} contaminated: {}",
                supervisor.report()
            ));
        }
        if expected != actual {
            return Err(format!("backends diverged in session seed {seed}"));
        }
        total_hands += actual.len();
    }
    let local_total: u64 = local_sessions.iter().sum();
    let remote_total: u64 = remote_sessions.iter().sum();
    let decisions = remote_decisions.borrow().len();
    Ok(json!({
        "sessions": sessions, "hands_per_session": hands, "hands": total_hands, "seats": FIELD.len(),
        "decisions": decisions, "records_identical": true,
        "in_process": {"decide": summary(&mut local_decisions.borrow_mut()),
            "session_play": summary(&mut local_sessions),
            "hands_per_second": total_hands as f64 / (local_total as f64 / 1e6)},
        "subprocess": {"decide_round_trip": summary(&mut remote_decisions.borrow_mut()),
            "session_play": summary(&mut remote_sessions),
            "hands_per_second": total_hands as f64 / (remote_total as f64 / 1e6),
            "startup_six_workers": summary(&mut startups),
            "shutdown_six_workers": summary(&mut shutdowns)},
        "note": "scripted policies only; excludes model/API latency. session_play excludes worker startup and shutdown."
    }))
}
