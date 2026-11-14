// ABOUTME: Estimates an exploitation ceiling: the best win rate a simple tunable policy achieves
// ABOUTME: against a fixed scripted field, selected on training seeds and confirmed on held-out seeds.
use crate::{
    analyze::interval,
    arena::{Agent, Session},
    opponents::{Params, Solid, Style, styles},
};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub struct Search {
    /// Five scripted opponents; the hero takes the sixth seat and rotates through all seats.
    pub field: Vec<Style>,
    pub train_seeds: u64,
    pub test_seeds: u64,
    /// Hands per session; a multiple of six so every seat posts the same blinds.
    pub hands: usize,
    pub threads: usize,
}

const SEATS: usize = 6;

/// The candidate family: every combination of these values.
pub fn grid() -> Vec<Params> {
    let mut candidates = vec![];
    for open in [21, 25, 29] {
        for value in [0.5, 0.62, 0.75] {
            for margin in [-0.05, 0.05, 0.15] {
                for bluff in [0.0, 0.4, 0.8] {
                    for bet in [0.66, 1.2] {
                        candidates.push(Params {
                            open,
                            value,
                            margin,
                            bluff,
                            bet,
                            ..Params::default()
                        });
                    }
                }
            }
        }
    }
    candidates
}

/// Hero win rate in bb/100 for each seed, averaged over all six hero seats. Stacks reset every
/// hand so hands are close to independent; the same seeds give every candidate the same deals.
pub fn evaluate(
    params: Params,
    field: &[Style],
    seeds: std::ops::Range<u64>,
    hands: usize,
) -> Result<Vec<f64>, String> {
    let mut blocks = vec![];
    for seed in seeds {
        let mut net = 0i64;
        for hero in 0..SEATS {
            let mut agents: Vec<Box<dyn Agent>> = styles(SEATS, hero, hero, field)
                .into_iter()
                .map(|style| match style {
                    Some(style) => style.agent(),
                    None => Box::new(Solid::new(params)),
                })
                .collect();
            let mut session = Session::new(SEATS, seed)?;
            session.reset_stacks = true;
            for _ in 0..hands {
                let record = session.next_hand(&mut agents)?.record;
                net += record.final_stacks[hero] as i64 - record.initial_stacks[hero] as i64;
            }
        }
        blocks.push(net as f64 / 2.0 / (SEATS * hands) as f64 * 100.0);
    }
    Ok(blocks)
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn run(search: &Search) -> Result<Value, String> {
    if search.field.is_empty() || search.hands == 0 || !search.hands.is_multiple_of(SEATS) {
        return Err("need a non-empty field and a hand count that is a multiple of six".into());
    }
    if search.train_seeds == 0 || search.test_seeds < 2 {
        return Err("need at least one training seed and two held-out seeds".into());
    }
    let candidates = grid();
    let next = AtomicUsize::new(0);
    let scores = Mutex::new(vec![None; candidates.len()]);
    let failure = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..search.threads.max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(params) = candidates.get(index) else {
                        return;
                    };
                    match evaluate(
                        *params,
                        &search.field,
                        1..search.train_seeds + 1,
                        search.hands,
                    ) {
                        Ok(blocks) => scores.lock().unwrap()[index] = Some(mean(&blocks)),
                        Err(e) => *failure.lock().unwrap() = Some(e),
                    }
                }
            });
        }
    });
    if let Some(error) = failure.into_inner().map_err(|_| "poisoned")? {
        return Err(error);
    }
    let scores: Vec<f64> = scores
        .into_inner()
        .map_err(|_| "poisoned")?
        .into_iter()
        .map(|s| s.ok_or("candidate not evaluated"))
        .collect::<Result<_, _>>()?;
    let best = (0..candidates.len())
        .max_by(|a, b| scores[*a].total_cmp(&scores[*b]))
        .expect("non-empty grid");
    // Held-out seeds: the training maximum is biased upward by selection, so the chosen
    // candidate is re-measured on deals it was not selected on.
    let held_out = 1_000_001..1_000_001 + search.test_seeds;
    let confirm = evaluate(
        candidates[best],
        &search.field,
        held_out.clone(),
        search.hands,
    )?;
    let solid = evaluate(Params::default(), &search.field, held_out, search.hands)?;
    let seat_hands = |seeds: u64| seeds as usize * SEATS * search.hands;
    Ok(json!({
        "field": search.field, "hero_seats_rotated": SEATS, "hands_per_session": search.hands,
        "stacks": "reset to 100 BB every hand", "candidates": candidates.len(),
        "training": {"seeds": search.train_seeds, "hands_per_candidate": seat_hands(search.train_seeds),
            "best_bb_per_100": scores[best], "worst_bb_per_100": scores.iter().copied().fold(f64::INFINITY, f64::min)},
        "best_params": candidates[best],
        "held_out": {"seeds": search.test_seeds, "hands": seat_hands(search.test_seeds),
            "best_bb_per_100": interval(&confirm), "default_solid_bb_per_100": interval(&solid)},
        "interpretation": "held_out.best_bb_per_100 is a lower bound on the true best response to this field: only this policy family was searched. Treat it as the exploitation ceiling reference for the field, not an optimum."
    }))
}
