// ABOUTME: End-to-end checks of the game thread against the local mock endpoint: thoughts are
// ABOUTME: captured, settings change between hands, and saved runs are readable by the lab's tools.
use poker_table::{
    mock,
    session::{self, Brain, Env, HERO, Notes, Phase, Settings, Shared},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("poker-table-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn start(settings: Settings, env: Env) -> Arc<Shared> {
    let shared = Shared::new(settings);
    session::spawn(shared.clone(), env);
    shared.control(|state| {
        state.speed = 0.0;
        state.phase = Phase::Playing;
    });
    shared
}

/// Blocks until `done` holds, pausing the table as soon as it does.
fn wait_for(shared: &Shared, what: &str, done: impl Fn(&session::State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        {
            let mut state = shared.lock();
            if done(&state) {
                state.paused = true;
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: {:?}",
                state.notice
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Lets the hand in progress finish after a pause was requested.
fn settle(shared: &Shared) {
    let (mut seen, mut stable_since) = (usize::MAX, Instant::now());
    while stable_since.elapsed() < Duration::from_millis(1500) {
        let hands = shared.lock().hands.len();
        if hands != seen {
            (seen, stable_since) = (hands, Instant::now());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn stand_in_session_plays_offline_and_saves_a_lab_run() {
    let runs = temp_dir("standin");
    let settings = Settings {
        brain: Brain::StandIn,
        ..Settings::default()
    };
    let shared = start(
        settings,
        Env {
            key: None,
            endpoint: None,
            runs_root: Some(runs.clone()),
        },
    );
    wait_for(&shared, "30 hands", |s| s.hands.len() >= 30);
    settle(&shared);
    let (run_dir, hands, hero_row) = {
        let state = shared.lock();
        assert!(
            state.thoughts.is_empty(),
            "the stand-in bot makes no requests"
        );
        assert_eq!(state.names[HERO], "STAND-IN");
        assert_eq!(state.stats.lab.requests, 0);
        (
            state.run_dir.clone().unwrap(),
            state.hands.len(),
            state.stats.rows[HERO].clone(),
        )
    };
    assert_eq!(hero_row["seat_hands"], hands as u64);
    assert_eq!(
        hero_row["reference_agreement"], 1.0,
        "the stand-in is the reference rule"
    );
    // The lab's own tools read the saved run.
    let analysis = poker_lab::analyze::directory(&run_dir).unwrap();
    assert_eq!(analysis["hands"], hands as u64);
    let report = poker_lab::report::directory(&run_dir.join("session-001"), Some(HERO)).unwrap();
    assert!(report.contains("HAND 1 |"));
    let reloaded = session::load_run(&run_dir.join("session-001")).unwrap();
    assert_eq!(reloaded.hands.len(), hands);
    assert_eq!(
        reloaded.stats.rows[HERO]["net_chips"],
        hero_row["net_chips"]
    );
    shared.control(|s| s.quit = true);
    let _ = std::fs::remove_dir_all(runs);
}

#[test]
fn model_session_captures_thoughts_and_applies_new_help_between_hands() {
    let runs = temp_dir("model");
    let env = Env {
        key: Some("mock-key".into()),
        endpoint: Some(mock::serve().unwrap()),
        runs_root: Some(runs.clone()),
    };
    let settings = Settings {
        model: "mock/model".into(),
        notes: Notes::Off,
        ..Settings::default()
    };
    let shared = start(settings, env);
    wait_for(&shared, "6 hands with facts sheet", |s| s.hands.len() >= 6);
    settle(&shared);
    let first_segment = {
        let state = shared.lock();
        assert_eq!(state.notice, None);
        let hand = &state.hands[0];
        assert_eq!(hand.help, "FACTS SHEET");
        let decisions = hand.analysis.decisions.len();
        assert!(decisions > 0);
        // Every decision of the model's seat has exactly one captured thought, with reasoning.
        for hand in &state.hands {
            for index in 0..hand.analysis.decisions.len() {
                let thoughts = state.thoughts_for(hand.number, index);
                assert_eq!(thoughts.len(), 1, "hand {} decision {index}", hand.number);
                let thought = thoughts[0];
                assert_eq!(thought.outcome, "ok");
                assert!(
                    thought
                        .reasoning
                        .as_deref()
                        .is_some_and(|r| r.starts_with("I hold"))
                );
                assert!(thought.aids().is_some(), "facts sheet was sent");
                assert!(thought.notes_shown().is_none());
                assert!(!thought.waiting);
            }
        }
        assert_eq!(state.stats.lab.fallbacks, 0);
        state.hands.len()
    };
    // During the pause: switch to baseline with the cheat sheet, thinking off.
    shared.control(|state| {
        state.settings.facts_sheet = false;
        state.settings.notes = Notes::CheatSheet;
        state.settings.thinking = false;
        state.paused = false;
    });
    wait_for(&shared, "hands under the cheat sheet", |s| {
        s.hands.len() >= first_segment + 4
    });
    settle(&shared);
    let second_segment = {
        let state = shared.lock();
        let hand = state.hands.last().unwrap();
        assert_eq!(hand.help, "CHEAT SHEET");
        let thought = state
            .thoughts
            .iter()
            .rev()
            .find(|t| t.is_action())
            .expect("the model acted under the new settings");
        assert!(thought.aids().is_none(), "facts sheet is off");
        assert!(
            thought
                .notes_shown()
                .is_some_and(|n| n.contains("Calling station"))
        );
        assert!(thought.reasoning.is_none(), "thinking is off");
        assert!(state.stats.by_help.len() == 2);
        state.hands.len()
    };
    // Own notes: the model rewrites them after every hand and sees them at its next decision.
    shared.control(|state| {
        state.settings.notes = Notes::Own;
        state.paused = false;
    });
    wait_for(&shared, "hands with own notes", |s| {
        s.hands.len() >= second_segment + 4
    });
    settle(&shared);
    let run_dir = {
        let state = shared.lock();
        assert!(
            state.notebook.contains("Mock note"),
            "notebook: {:?}",
            state.notebook
        );
        let last = state.hands.last().unwrap().number;
        assert!(
            state
                .notebook_thought(last)
                .is_some_and(|t| t.notes_after.is_some())
        );
        // The hand in progress at the pause finishes under the old settings, so at least three
        // of the four new hands were played with own notes.
        assert!(
            state.stats.lab.memory_requests >= 3,
            "{:?}",
            state.stats.lab
        );
        let seen = state.thoughts.iter().rev().find(|t| t.is_action()).unwrap();
        assert!(seen.notes_shown().is_some_and(|n| n.contains("Mock note")));
        state.run_dir.clone().unwrap()
    };
    // Each stretch of settings is its own lab session with its own condition label.
    let analysis = poker_lab::analyze::directory(&run_dir).unwrap();
    assert_eq!(analysis["sessions"], 3);
    let conditions: Vec<&str> = analysis["model_style_by_condition"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["condition"].as_str().unwrap())
        .collect();
    assert_eq!(conditions, ["none+features", "notes", "oracle"]);
    let reloaded = session::load_run(&run_dir.join("session-001")).unwrap();
    assert!(reloaded.thoughts.iter().any(|t| t.reasoning.is_some()));
    shared.control(|s| s.quit = true);
    let _ = std::fs::remove_dir_all(runs);
}

#[test]
fn the_spend_cap_and_a_missing_key_stop_the_table_with_a_message() {
    let shared = start(
        Settings::default(),
        Env {
            key: None,
            endpoint: None,
            runs_root: None,
        },
    );
    wait_for(&shared, "missing key notice", |s| s.notice.is_some());
    {
        let state = shared.lock();
        assert!(
            state
                .notice
                .as_deref()
                .unwrap()
                .contains("no OpenRouter key")
        );
        assert!(state.hands.is_empty());
    }
    // The stand-in needs no key: the same table continues after the change.
    shared.control(|state| {
        state.settings.brain = Brain::StandIn;
        state.paused = false;
    });
    wait_for(&shared, "a hand from the stand-in", |s| !s.hands.is_empty());
    shared.control(|s| s.quit = true);
}

#[test]
fn thinking_leaves_room_for_long_river_reasoning_and_waits_for_it() {
    // The first live run truncated 11 of 79 river decisions at a 4,096-token limit.
    let thinking = Settings::default().lab_config();
    assert_eq!(thinking.max_tokens, session::THINKING_MAX_TOKENS);
    assert!(thinking.max_tokens >= 12_288);
    // The timeout must cover a full-length answer at the slowest decode speed seen (~69 tokens/s).
    assert!(thinking.timeout_ms >= u64::from(thinking.max_tokens) * 1000 / 60);
    let quick = Settings {
        thinking: false,
        ..Settings::default()
    }
    .lab_config();
    assert_eq!((quick.max_tokens, quick.timeout_ms), (256, 30_000));
}
