// ABOUTME: Estimates an exploitation ceiling: the best win rate a simple tunable policy achieves
// ABOUTME: against a fixed scripted field, selected on training seeds and confirmed on held-out seeds.
use crate::{
    analyze::interval,
    arena::{Agent, Session},
    opponents::{Params, Solid, Style, Variation, seat_bots},
};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub struct Search {
    /// Five scripted opponents; the hero takes the sixth seat and rotates through all seats.
    pub field: Vec<Style>,
    /// Less predictable bots, drawn from each seed as in the experiments (none by default).
    pub variation: Variation,
    /// Also measure the chosen policy and default `solid` on these seeds with the hero in this
    /// seat only: the exact deals and bots an experiment's model seat faced.
    pub check: Option<(std::ops::Range<u64>, usize)>,
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

/// Hero win rate in bb/100 for each seed, averaged over the `heroes` seats. Stacks reset every
/// hand so hands are close to independent; the same seeds give every candidate the same deals
/// and, with `variation`, the same drawn bots.
pub fn evaluate(
    params: Params,
    field: &[Style],
    variation: &Variation,
    seeds: std::ops::Range<u64>,
    hands: usize,
    heroes: &[usize],
) -> Result<Vec<f64>, String> {
    let mut blocks = vec![];
    for seed in seeds {
        let mut net = 0i64;
        for &hero in heroes {
            // The hero's offset is its seat, as `styles` always placed the field.
            let mut agents: Vec<Box<dyn Agent>> =
                seat_bots(SEATS, hero, hero, field, variation, seed)
                    .into_iter()
                    .map(|bot| match bot {
                        Some(bot) => bot.agent(),
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
        blocks.push(net as f64 / 2.0 / (heroes.len() * hands) as f64 * 100.0);
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
    search.variation.validate()?;
    let all_seats: Vec<usize> = (0..SEATS).collect();
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
                        &search.variation,
                        1..search.train_seeds + 1,
                        search.hands,
                        &all_seats,
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
    let measure = |params: Params, seeds: std::ops::Range<u64>, heroes: &[usize]| {
        evaluate(
            params,
            &search.field,
            &search.variation,
            seeds,
            search.hands,
            heroes,
        )
    };
    let confirm = measure(candidates[best], held_out.clone(), &all_seats)?;
    let solid = measure(Params::default(), held_out, &all_seats)?;
    let check = match &search.check {
        Some((seeds, seat)) => {
            if *seat >= SEATS {
                return Err(format!(
                    "check seat {seat} is not a seat of a six-seat table"
                ));
            }
            json!({"seeds": [seeds.start, seeds.end - 1], "hero_seat": seat,
                "best_bb_per_100": interval(&measure(candidates[best], seeds.clone(), &[*seat])?),
                "default_solid_bb_per_100": interval(&measure(Params::default(), seeds.clone(), &[*seat])?)})
        }
        None => Value::Null,
    };
    let seat_hands = |seeds: u64| seeds as usize * SEATS * search.hands;
    Ok(json!({
        "field": search.field, "opponent_variation": search.variation,
        "hero_seats_rotated": SEATS, "hands_per_session": search.hands,
        "stacks": "reset to 100 BB every hand", "candidates": candidates.len(),
        "training": {"seeds": search.train_seeds, "hands_per_candidate": seat_hands(search.train_seeds),
            "best_bb_per_100": scores[best], "worst_bb_per_100": scores.iter().copied().fold(f64::INFINITY, f64::min)},
        "best_params": candidates[best],
        "held_out": {"seeds": search.test_seeds, "hands": seat_hands(search.test_seeds),
            "best_bb_per_100": interval(&confirm), "default_solid_bb_per_100": interval(&solid)},
        "check": check,
        "interpretation": "held_out.best_bb_per_100 is a lower bound on the true best response to this field: only this policy family was searched. Treat it as the exploitation ceiling reference for the field, not an optimum."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opponents::DEFAULT_FIELD;

    #[test]
    fn varied_bots_replay_and_differ_from_the_fixed_field() {
        let varied = Variation {
            personalities: true,
            mixing: 0.1,
            random_field: true,
        };
        let run = |v: &Variation| evaluate(Params::default(), &DEFAULT_FIELD, v, 1..4, 18, &[0]);
        assert_eq!(run(&varied).unwrap(), run(&varied).unwrap());
        assert_ne!(run(&varied).unwrap(), run(&Variation::default()).unwrap());
    }
}
