use poker_lab::{
    arena::{Agent, Record, Scripted, play, replay},
    cards::Rng,
    engine::{Action, Event, Hand, Rules},
    experiment::Batch,
    memory::{Memory, Mode},
    openrouter::{Config, Opponents},
    opponents::{ADAPT_AFTER, Adaptive, Opponent, Params, Solid, Style},
    report,
    scaffold::Scaffold,
};
#[test]
fn mixed_opponents_are_legal_reproducible_and_conserve() {
    for seed in 0..200 {
        let run = || {
            let mut agents: Vec<Box<dyn Agent>> = vec![
                Box::new(Opponent(Style::Tight)),
                Box::new(Opponent(Style::Loose)),
                Box::new(Opponent(Style::Aggressive)),
                Box::new(Scripted::Random(Rng(seed))),
            ];
            play(
                Rules::default(),
                vec![200, 27, 89, 3],
                seed as usize % 4,
                seed,
                &mut agents,
            )
            .unwrap()
        };
        let a = run();
        let b = run();
        assert_eq!(a.record, b.record);
        assert!(a.record.decisions.iter().all(|d| d.error.is_none()));
        replay(&a.record).unwrap();
    }
}
#[test]
fn policy_styles_take_distinct_actions_on_same_cards() {
    let mut view = Hand::new(Rules::default(), vec![200; 2], 0, 1)
        .unwrap()
        .view(0)
        .unwrap();
    view.hole = [
        poker_lab::cards::Card::try_from(11).unwrap(),
        poker_lab::cards::Card::try_from(20).unwrap(),
    ]; // K9 offsuit: score 22
    assert_eq!(Opponent(Style::Tight).decide(&view).unwrap(), Action::Fold);
    assert_eq!(Opponent(Style::Loose).decide(&view).unwrap(), Action::Call);
    view.hole = [
        poker_lab::cards::Card::try_from(11).unwrap(),
        poker_lab::cards::Card::try_from(23).unwrap(),
    ]; // KQ: score25, aggressive plays but not strong
    assert!(Opponent(Style::Aggressive).decide(&view).is_ok());
}
#[test]
fn stats_count_hands_not_actions_and_notes_are_bounded() {
    let p = play(
        Rules::default(),
        vec![200; 2],
        0,
        42,
        &mut [Box::new(Scripted::Caller), Box::new(Scripted::Caller)],
    )
    .unwrap();
    let view = p.private_views[0].last().unwrap();
    let mut m = Memory::default();
    m.observe(view);
    assert_eq!(m.stats[0].vpip, 1);
    assert_eq!(m.stats[1].vpip, 0);
    assert_eq!(m.stats[0].preflop_opportunities, 1);
    assert_eq!(m.stats[0].showdowns, 1);
    assert_eq!(m.stats[0].pfr, 0);
    m.update("replace", "tentative read", 20).unwrap();
    let old = m.notebook.clone();
    assert!(m.update("replace", "too long", 2).is_err());
    assert_eq!(m.notebook, old);
    m.update("keep", "", 20).unwrap();
    assert_eq!(m.revision, 1);
    assert!(Memory::default().notebook.is_empty());
}
#[test]
fn report_defaults_to_public_and_rejects_corrupt_replay() {
    let p = play(
        Rules::default(),
        vec![200; 2],
        0,
        42,
        &mut [Box::new(Scripted::Folder), Box::new(Scripted::Caller)],
    )
    .unwrap();
    let public = report::hand(&p.record, 1, None).unwrap();
    assert!(!public.contains("private cards"));
    assert!(!public.contains("reveals"));
    let private = report::hand(&p.record, 1, Some(0)).unwrap();
    assert!(private.contains("Seat 0 private cards"));
    assert!(!private.contains("Seat 1 private cards"));
    let mut corrupt = p.record;
    corrupt.final_stacks[0] += 1;
    assert!(report::hand(&corrupt, 1, None).is_err());
}
#[test]
fn schedule_crosses_conditions_seeds_and_seats() {
    let b = Batch {
        pilot: Config {
            model: "test/model".into(),
            opponents: Opponents::Mixed,
            ..Config::default()
        },
        seeds: vec![1, 2],
        model_seats: vec![0, 1, 2, 3, 4, 5],
        conditions: vec![Mode::None, Mode::Notes, Mode::Stats, Mode::Both],
        max_requests: 1000,
        spend_limit_usd: 1.0,
        parallel: 0,
        allow_partial_orbits: true,
        scaffolds: vec![],
        unknown_cost_allowance: 0,
        unknown_cost_charge_usd: 0.01,
        outage_pause_ms: 20,
        outage_max_wait_s: 5,
    };
    let schedule = b.schedule().unwrap();
    assert_eq!(schedule.len(), 48);
    assert_eq!(schedule[6].seed, 2);
    assert_eq!(schedule[12].memory, Mode::Notes);
    assert_eq!(schedule[5].opponent_offset, 5);
    assert!(schedule.iter().all(|c| c.scaffold == Scaffold::None));
}
#[test]
fn schedule_crosses_scaffolds_with_conditions() {
    let b: Batch = serde_json::from_value(serde_json::json!({
        "pilot": {"model":"test/model","opponents":"mixed","hands":6},
        "seeds": [1, 2], "model_seats": [0, 3], "conditions": ["none", "stats"],
        "scaffolds": ["none", "features", "equity", "advice"],
        "max_requests": 1000, "spend_limit_usd": 1.0
    }))
    .unwrap();
    let schedule = b.schedule().unwrap();
    assert_eq!(schedule.len(), 2 * 4 * 2 * 2);
    let labels: Vec<String> = schedule.iter().step_by(4).map(|c| c.condition()).collect();
    assert_eq!(
        labels,
        [
            "none",
            "none+features",
            "none+equity",
            "none+advice",
            "stats",
            "stats+features",
            "stats+equity",
            "stats+advice"
        ]
    );
    // Every treatment plays the same seeds and seats, so paired differences are possible.
    for chunk in schedule.chunks(4) {
        let seeds: Vec<_> = chunk.iter().map(|c| (c.seed, c.model_seat)).collect();
        assert_eq!(seeds, [(1, 0), (1, 3), (2, 0), (2, 3)]);
    }
}
#[test]
fn folded_summaries_keep_opponent_cards_hidden() {
    let p = play(
        Rules::default(),
        vec![200; 2],
        0,
        7,
        &mut [Box::new(Scripted::Folder), Box::new(Scripted::Caller)],
    )
    .unwrap();
    let v = p.private_views[0].last().unwrap();
    assert!(v.history.iter().all(|e| !matches!(e, Event::Reveal { .. })));
    assert!(poker_lab::memory::completed(v));
}

const ALL_STYLES: [Style; 7] = [
    Style::Tight,
    Style::Loose,
    Style::Aggressive,
    Style::Station,
    Style::Overfolder,
    Style::Solid,
    Style::Adaptive,
];
#[test]
fn every_style_is_legal_reproducible_and_described() {
    for seed in 0..60u64 {
        let run = || {
            let mut agents: Vec<Box<dyn Agent>> = (0..6)
                .map(|i| ALL_STYLES[(i + seed as usize) % 7].agent())
                .collect();
            let mut session = poker_lab::arena::Session::new(6, seed).unwrap();
            session.reset_stacks = seed % 2 == 0;
            (0..12)
                .map(|_| session.next_hand(&mut agents).unwrap().record)
                .collect::<Vec<_>>()
        };
        let records = run();
        assert_eq!(records, run());
        for record in &records {
            assert!(record.decisions.iter().all(|d| d.error.is_none()));
            replay(record).unwrap();
        }
    }
    for style in ALL_STYLES {
        assert!(style.describe().len() > 40);
        assert_eq!(
            serde_json::to_value(style).unwrap(),
            style.policy().name(),
            "config names match protocol policy names"
        );
    }
}
/// A river decision for seat 0 holding `hole` on `board`, facing `bet` (0: checked to).
fn river_view(hole: [u8; 2], board: [u8; 5], bet: u64) -> poker_lab::engine::PlayerView {
    let card = |v: u8| poker_lab::cards::Card::try_from(v).unwrap();
    let mut view = Hand::new(Rules::default(), vec![200; 2], 0, 1)
        .unwrap()
        .view(0)
        .unwrap();
    view.hole = [card(hole[0]), card(hole[1])];
    view.board = board.iter().map(|v| card(*v)).collect();
    view.street = poker_lab::engine::Street::River;
    view.pot = 20 + bet;
    view.history.clear();
    let legal = view.legal.as_mut().unwrap();
    legal.check = bet == 0;
    legal.call = (bet > 0).then_some(bet);
    legal.min_raise_to = Some(if bet == 0 { 2 } else { bet * 2 });
    legal.max_raise_to = Some(190);
    view.players[0].stack = 190;
    view.players[0].street_committed = 0;
    view
}
#[test]
fn bounded_leaks_behave_as_documented() {
    // Board 2c 7d 9h Jc 3s (no flush, no straight). Kc Kd: one pair. 7c 9d: two pair.
    let board = [0, 18, 33, 9, 40];
    let (pair, two_pair, air) = ([11, 24], [5, 20], [3, 15]);
    // Bets up to half the 190-chip stack, the over-folder's general calling cap.
    for bet in [1, 20, 90] {
        // The station calls any size with a pair and never raises; it folds nothing-hands.
        let station = |hole| Opponent(Style::Station).decide(&river_view(hole, board, bet));
        assert_eq!(station(pair).unwrap(), Action::Call);
        assert_eq!(station(two_pair).unwrap(), Action::Call);
        assert_eq!(station(air).unwrap(), Action::Fold);
        // The over-folder gives up one pair to any river bet but continues with two pair.
        let overfolder = |hole| Opponent(Style::Overfolder).decide(&river_view(hole, board, bet));
        assert_eq!(overfolder(pair).unwrap(), Action::Fold);
        assert_eq!(overfolder(two_pair).unwrap(), Action::Call);
    }
    assert_eq!(
        Opponent(Style::Station)
            .decide(&river_view(two_pair, board, 0))
            .unwrap(),
        Action::Check,
        "a station never bets"
    );
    // Solid: bets strong hands, folds weak ones to a large bet, calls strong ones.
    let mut solid = Solid::new(Params {
        bluff: 0.0,
        ..Params::default()
    });
    assert!(matches!(
        solid.decide(&river_view(two_pair, board, 0)).unwrap(),
        Action::RaiseTo(_)
    ));
    solid.start_hand(2).unwrap();
    assert_eq!(
        solid.decide(&river_view(air, board, 0)).unwrap(),
        Action::Check
    );
    solid.start_hand(3).unwrap();
    assert_eq!(
        solid.decide(&river_view(air, board, 150)).unwrap(),
        Action::Fold
    );
    solid.start_hand(4).unwrap();
    assert_ne!(
        solid.decide(&river_view(two_pair, board, 10)).unwrap(),
        Action::Fold
    );
}
#[test]
fn adaptive_policy_follows_its_published_rule() {
    use poker_lab::engine::Street;
    let summary = |response: Action| {
        let mut view = river_view([11, 24], [0, 18, 33, 9, 40], 0);
        view.street = Street::Complete;
        view.legal = None;
        view.history = vec![
            Event::Board {
                street: Street::Flop,
                cards: vec![],
            },
            Event::Act {
                seat: 0,
                action: Action::RaiseTo(4),
            },
            Event::Act {
                seat: 1,
                action: response,
            },
        ];
        view
    };
    let mut bluffed_out = Adaptive::default();
    let mut called_down = Adaptive::default();
    for hand in 0..ADAPT_AFTER {
        // Below the evidence threshold the defaults stay in force.
        assert_eq!(bluffed_out.params(), Params::default(), "hand {hand}");
        bluffed_out.finish_hand(&summary(Action::Fold)).unwrap();
        called_down.finish_hand(&summary(Action::Call)).unwrap();
    }
    assert_eq!(
        (bluffed_out.responses, bluffed_out.folds),
        (ADAPT_AFTER, ADAPT_AFTER)
    );
    assert_eq!(bluffed_out.params().bluff, 0.8);
    assert_eq!(called_down.params().bluff, 0.0);
    assert_eq!(called_down.params().value, 0.55);
    // Evidence the other way moves it back: the rule uses the running fold rate.
    for _ in 0..ADAPT_AFTER {
        bluffed_out.finish_hand(&summary(Action::Call)).unwrap();
    }
    assert_eq!(bluffed_out.params(), Params::default());
}
#[test]
fn ceiling_search_is_deterministic_and_selects_on_training_seeds() {
    use poker_lab::ceiling::{Search, evaluate, grid, run};
    let field = poker_lab::opponents::DEFAULT_FIELD;
    let fixed = poker_lab::opponents::Variation::default();
    let all: Vec<usize> = (0..6).collect();
    let blocks = evaluate(Params::default(), &field, &fixed, 1..3, 6, &all).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks,
        evaluate(Params::default(), &field, &fixed, 1..3, 6, &all).unwrap()
    );
    let search = Search {
        field: field.to_vec(),
        variation: fixed,
        check: Some((21..23, 0)),
        train_seeds: 1,
        test_seeds: 2,
        hands: 6,
        threads: 4,
    };
    let result = run(&search).unwrap();
    assert_eq!(result["candidates"], grid().len());
    assert_eq!(result, run(&search).unwrap());
    assert_eq!(result["held_out"]["best_bb_per_100"]["blocks"], 2);
    assert_eq!(result["check"]["seeds"], serde_json::json!([21, 22]));
    assert_eq!(result["check"]["best_bb_per_100"]["blocks"], 2);
    assert!(
        result["training"]["best_bb_per_100"].as_f64().unwrap()
            >= result["training"]["worst_bb_per_100"].as_f64().unwrap()
    );
    assert!(run(&Search { hands: 7, ..search }).is_err());
}
#[test]
fn analysis_intervals_and_style_counts() {
    use poker_lab::analyze::interval;
    let one = interval(&[5.0]);
    assert_eq!(one["blocks"], 1);
    assert!(one.get("ci95").is_none());
    let three = interval(&[1.0, 2.0, 3.0]);
    assert_eq!(three["mean"], 2.0);
    let se = three["se"].as_f64().unwrap();
    assert!((se - (1.0f64 / 3.0).sqrt()).abs() < 1e-12);
    // df = 2 -> t = 4.303
    assert!((three["ci95"][1].as_f64().unwrap() - (2.0 + 4.303 * se)).abs() < 1e-9);
}

fn decisions_dir(
    name: &str,
    manifest: Option<serde_json::Value>,
) -> (std::path::PathBuf, Vec<Record>) {
    let dir = std::env::temp_dir().join(format!(
        "poker-lab-decisions-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    poker_lab::private_fs::create_dir(&dir).unwrap();
    let mut session = poker_lab::arena::Session::new(6, 7).unwrap();
    let mut agents: Vec<Box<dyn Agent>> = (0..6)
        .map(|i| {
            Box::new(Opponent(
                [Style::Station, Style::Solid, Style::Tight][i % 3],
            )) as Box<dyn Agent>
        })
        .collect();
    let records: Vec<Record> = (0..40)
        .map(|_| session.next_hand(&mut agents).unwrap().record)
        .collect();
    let lines: Vec<String> = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();
    std::fs::write(dir.join("evaluator.jsonl"), lines.join("\n") + "\n").unwrap();
    if let Some(m) = manifest {
        std::fs::write(dir.join("manifest.json"), m.to_string()).unwrap();
    }
    (dir, records)
}

#[test]
fn decisions_list_every_decision_with_pot_odds_and_match_the_grades() {
    let (dir, records) = decisions_dir("all", None);
    let rows = poker_lab::analyze::decisions(&dir).unwrap();
    // Without a manifest every seat is listed.
    let total: usize = records.iter().map(|r| r.decisions.len()).sum();
    assert_eq!(rows.len(), total);
    for row in &rows {
        let (pot, call) = (row["pot"].as_f64().unwrap(), row["call"].as_f64().unwrap());
        let equity = row["equity"].as_f64().unwrap();
        assert!((0.0..=1.0).contains(&equity));
        if call > 0.0 {
            let required = row["required"].as_f64().unwrap();
            assert!((required - call / (pot + call)).abs() < 1e-9);
            let ev = row["ev_call"].as_f64().unwrap();
            assert!((ev - (equity * (pot + call) - call)).abs() < 1e-9);
        } else {
            assert!(row["required"].is_null() && row["ev_call"].is_null());
            assert!(row["bettor"].is_null());
        }
        assert_eq!(row["hole"].as_str().unwrap().len(), 4, "{row}");
    }
    // Every call or fold facing a bet carries the same equity as the lab's decision grade.
    for seat in 0..6 {
        let graded: Vec<f64> = records
            .iter()
            .flat_map(|r| poker_lab::evaluation::grade_seat(r, seat).unwrap())
            .map(|g| g.equity)
            .collect();
        let listed: Vec<f64> = rows
            .iter()
            .filter(|r| r["seat"] == seat && r["call"].as_u64().unwrap() > 0)
            .filter(|r| r["action"]["type"] == "fold" || r["action"]["type"] == "call")
            .map(|r| r["equity"].as_f64().unwrap())
            .collect();
        assert_eq!(graded, listed, "seat {seat}");
    }
    // Postflop bets faced name who made them.
    assert!(
        rows.iter()
            .any(|r| r["street"] != "Preflop" && r["bettor"].is_string())
    );
}

#[test]
fn decisions_list_only_model_seats_when_a_manifest_names_them() {
    let config = Config {
        model: "test/model".into(),
        opponents: Opponents::Mixed,
        ..Config::default()
    };
    let (dir, records) = decisions_dir(
        "model",
        Some(serde_json::json!({ "config": serde_json::to_value(&config).unwrap() })),
    );
    let rows = poker_lab::analyze::decisions(&dir).unwrap();
    let seat0: usize = records
        .iter()
        .map(|r| r.decisions.iter().filter(|d| d.seat == 0).count())
        .sum();
    assert_eq!(rows.len(), seat0);
    assert!(
        rows.iter()
            .all(|r| r["seat"] == 0 && r["label"] == "test/model")
    );
    // These bots are not the manifest's field, so river grading is skipped with a reason.
    assert!(
        rows.iter()
            .all(|r| r["river"].is_null() && r["river_error"].is_string())
    );
    let analysis = poker_lab::analyze::directory(&dir).unwrap();
    assert_eq!(analysis["river_grade_errors"].as_array().unwrap().len(), 1);
    assert!(poker_lab::analyze::render(&analysis).contains("River grading skipped"));
}

#[test]
fn analyze_grades_model_rivers_against_the_scripted_field() {
    // A lab-format run whose model seat is played by the solid bot: every other seat follows
    // the manifest's field, so river grading can replay them.
    let config = Config {
        model: "test/model".into(),
        opponents: Opponents::Mixed,
        ..Config::default()
    };
    let styles = config.seat_styles();
    let mut agents: Vec<Box<dyn Agent>> = styles
        .iter()
        .map(|s| s.unwrap_or(Style::Solid).agent())
        .collect();
    let mut session = poker_lab::arena::Session::new(6, 3).unwrap();
    session.reset_stacks = true;
    let lines: Vec<String> = (0..120)
        .map(|_| serde_json::to_string(&session.next_hand(&mut agents).unwrap().record).unwrap())
        .collect();
    let dir = std::env::temp_dir().join(format!(
        "poker-lab-river-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    poker_lab::private_fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("evaluator.jsonl"), lines.join("\n") + "\n").unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::json!({ "config": serde_json::to_value(&config).unwrap() }).to_string(),
    )
    .unwrap();
    let analysis = poker_lab::analyze::directory(&dir).unwrap();
    let model = &analysis["model_style_by_condition"][0];
    assert!(model["river_graded"].as_u64().unwrap() > 0, "{model}");
    assert!(model["river_cost_bb_per_100"].as_f64().unwrap() >= 0.0);
    assert!(model["bluffs"].is_u64() && model["truncations"] == 0);
    let text = poker_lab::analyze::render(&analysis);
    assert!(text.contains("River decisions graded exactly") && text.contains("Leaks"));
    let rows = poker_lab::analyze::decisions(&dir).unwrap();
    let graded = rows.iter().filter(|r| r["river"].is_object()).count();
    assert_eq!(graded as u64, model["river_graded"].as_u64().unwrap());
    assert!(
        rows.iter()
            .filter(|r| r["street"] != "River")
            .all(|r| r["river"].is_null())
    );
}

#[test]
fn every_example_config_parses_and_validates() {
    // Batch configs have a "pilot" block; the rest are single-run configs.
    // Configs are grouped in topic folders (examples/1-pilots, ...), so walk the whole tree.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files = vec![];
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    let mut checked = 0;
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
    {
        let text = std::fs::read_to_string(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value.get("pilot").is_some() {
            let batch: Batch =
                serde_json::from_value(value).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            batch
                .schedule()
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        } else {
            let mut config: Config =
                serde_json::from_value(value).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            // Placeholder model IDs in templates are allowed; everything else must validate.
            if config.model == "provider/model" {
                config.model = "test/model".into();
            }
            config
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        checked += 1;
    }
    assert!(checked > 50, "only {checked} example configs");
}

#[test]
fn analyze_grades_a_varied_field_from_its_manifest() {
    // Personalities, mixing and a random field are rebuilt from the manifest's config: if
    // analyze rebuilt the wrong bots, their real cards would fall outside the graded ranges and
    // the session would report a grading error.
    let config = Config {
        model: "test/model".into(),
        opponents: Opponents::Mixed,
        seed: 21,
        opponent_field: vec![
            Style::Station,
            Style::Overfolder,
            Style::Solid,
            Style::Adaptive,
            Style::Tight,
        ],
        opponent_variation: poker_lab::opponents::Variation {
            personalities: true,
            mixing: 0.15,
            random_field: true,
        },
        ..Config::default()
    };
    let bots = config.seat_bots();
    let mut agents: Vec<Box<dyn Agent>> = bots
        .iter()
        .map(|b| b.map_or_else(|| Style::Solid.agent(), |b| b.agent()))
        .collect();
    let mut session = poker_lab::arena::Session::new(6, 21).unwrap();
    session.reset_stacks = true;
    let lines: Vec<String> = (0..60)
        .map(|_| serde_json::to_string(&session.next_hand(&mut agents).unwrap().record).unwrap())
        .collect();
    let dir = std::env::temp_dir().join(format!(
        "poker-lab-varied-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    poker_lab::private_fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("evaluator.jsonl"), lines.join("\n") + "\n").unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::json!({ "config": serde_json::to_value(&config).unwrap() }).to_string(),
    )
    .unwrap();
    let analysis = poker_lab::analyze::directory_graded(&dir, true, true).unwrap();
    assert_eq!(analysis["river_grade_errors"], serde_json::json!([]));
    let model = &analysis["model_style_by_condition"][0];
    assert!(model["river_graded"].as_u64().unwrap() > 0, "{model}");
    assert!(model["turn_graded"].as_u64().unwrap() > 0, "{model}");
    // The oracle notes say the session's bots vary.
    assert!(config.oracle_notes().contains("varies"));
}
