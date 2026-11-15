// ABOUTME: Evaluator-only scoring that removes card luck: all-in equity adjustment of hand results
// ABOUTME: and a pot-odds grade of every call-or-fold decision against the opponents' real cards.
use crate::{
    arena::Record,
    cards::{Card, Rng, deck},
    engine::{Action, Chips, Event, Hand, Street},
    equity::equity_vs,
};

/// Run-outs sampled when two or more board cards are still to come (fewer are enumerated).
pub const RUNOUT_SAMPLES: u32 = 400;
/// Run-outs sampled per graded decision when two or more board cards are still to come.
pub const GRADE_SAMPLES: u32 = 200;

/// Every seat's hole cards, reconstructed from the record's seed exactly as the engine deals.
pub fn holes(record: &Record) -> Vec<[Card; 2]> {
    let mut cards = deck(record.seed);
    let n = record.initial_stacks.len();
    let mut holes = vec![[cards[0]; 2]; n];
    for round in [0, 1] {
        for offset in 1..=n {
            holes[(record.button + offset) % n][round] = cards.pop().expect("52 cards");
        }
    }
    holes
}

/// Expected final stacks when the money went in before the board was complete: the hand's
/// actions replayed over every possible remainder of the board (sampled for two or more
/// missing cards), averaging the payouts. `None` when the result involved no luck after the
/// last decision (a fold ended it, or the last decision was on the river).
pub fn allin_adjusted(record: &Record) -> Result<Option<Vec<f64>>, String> {
    if !record
        .events
        .iter()
        .any(|e| matches!(e, Event::Reveal { .. }))
    {
        return Ok(None);
    }
    // Board cards known when the last decision was made.
    let mut hand = Hand::new(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        record.seed,
    )?;
    let mut known = hand_board(&hand);
    for d in &record.decisions {
        known = hand_board(&hand);
        hand.act(d.seat, d.applied)?;
    }
    if known >= 5 {
        return Ok(None);
    }
    let n = record.initial_stacks.len();
    let original = deck(record.seed);
    // Pop order: 2n hole cards, then burn, 3 flop, burn, turn, burn, river.
    let slot = |pop: usize| original.len() - 1 - pop;
    let board_pops = [2 * n + 1, 2 * n + 2, 2 * n + 3, 2 * n + 5, 2 * n + 7];
    let fixed: Vec<usize> = (0..2 * n)
        .chain(board_pops[..known].iter().copied())
        .map(slot)
        .collect();
    let future: Vec<usize> = board_pops[known..].iter().map(|p| slot(*p)).collect();
    let free: Vec<usize> = (0..original.len()).filter(|i| !fixed.contains(i)).collect();
    let mut totals = vec![0.0; n];
    let mut runs = 0u32;
    let mut play = |cards: &[Card]| -> Result<(), String> {
        let mut shuffled = original.clone();
        // Put the chosen cards in the future board slots by swapping them out of free slots.
        for (target, card) in future.iter().zip(cards) {
            let from = shuffled
                .iter()
                .position(|c| c == card)
                .expect("card in deck");
            shuffled.swap(*target, from);
        }
        let mut hand = Hand::with_deck(
            record.rules.clone(),
            record.initial_stacks.clone(),
            record.button,
            shuffled,
        )?;
        for d in &record.decisions {
            hand.act(d.seat, d.applied)?;
        }
        for (total, stack) in totals.iter_mut().zip(hand.stacks()) {
            *total += stack as f64;
        }
        runs += 1;
        Ok(())
    };
    let pool: Vec<Card> = free.iter().map(|i| original[*i]).collect();
    match future.len() {
        1 => {
            for card in &pool {
                play(&[*card])?;
            }
        }
        missing => {
            let mut rng = Rng(record.seed ^ 0xa11_1a5d);
            let mut pool = pool;
            for _ in 0..RUNOUT_SAMPLES {
                for i in 0..missing {
                    let j = i + rng.below((pool.len() - i) as u64) as usize;
                    pool.swap(i, j);
                }
                play(&pool[..missing])?;
            }
        }
    }
    Ok(Some(
        totals.into_iter().map(|t| t / f64::from(runs)).collect(),
    ))
}

fn hand_board(hand: &Hand) -> usize {
    hand.events()
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Board { cards, .. } => Some(cards.len()),
            _ => None,
        })
        .unwrap_or(0)
}

/// A seat's showdown share against its live opponents' actual cards, with the rest of the
/// board unknown: exact for one missing card, `GRADE_SAMPLES` run-outs otherwise.
pub fn equity_against(hole: [Card; 2], board: &[Card], opponents: &[[Card; 2]]) -> f64 {
    equity_vs(hole, board, opponents, GRADE_SAMPLES)
}

/// One graded call-or-fold decision. Chip values are immediate pot-odds expectations, ignoring
/// later betting: EV(call) = equity × (pot + call) − call, EV(fold) = 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Grade {
    pub seat: usize,
    pub street: Street,
    pub called: bool,
    pub equity: f64,
    pub required: f64,
    /// Chips the chosen action gave up against the better of call and fold (0 when correct).
    pub loss: f64,
}

/// Grades every decision where `seat` faced a bet and called or folded (raises and checks are
/// not graded). Equity is computed against the cards the remaining opponents actually held,
/// with the rest of the board unknown: a perfect-information grade, not a range-based one.
pub fn grade_seat(record: &Record, seat: usize) -> Result<Vec<Grade>, String> {
    let holes = holes(record);
    let mut hand = Hand::new(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        record.seed,
    )?;
    let mut grades = vec![];
    for d in &record.decisions {
        if d.seat == seat {
            let view = hand.view(seat)?;
            let call = view.legal.as_ref().and_then(|l| l.call).unwrap_or(0);
            let chosen = d.applied;
            if call > 0 && matches!(chosen, Action::Fold | Action::Call) {
                let opponents: Vec<[Card; 2]> = view
                    .players
                    .iter()
                    .filter(|p| !p.folded && p.seat != seat)
                    .map(|p| holes[p.seat])
                    .collect();
                let equity = equity_against(holes[seat], &view.board, &opponents);
                let pot: Chips = view.pot;
                let ev_call = equity * (pot + call) as f64 - call as f64;
                let called = chosen == Action::Call;
                let best = ev_call.max(0.0);
                let got = if called { ev_call } else { 0.0 };
                grades.push(Grade {
                    seat,
                    street: view.street,
                    called,
                    equity,
                    required: call as f64 / (pot + call) as f64,
                    loss: best - got,
                });
            }
        }
        hand.act(d.seat, d.applied)?;
    }
    Ok(grades)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arena::{Scripted, play},
        engine::Rules,
    };

    fn all_in_record(seed: u64) -> Record {
        // Raiser shoves preflop, callers call: every hand is all-in before the flop.
        struct Shove;
        impl crate::arena::Agent for Shove {
            fn decide(&mut self, v: &crate::engine::PlayerView) -> Result<Action, String> {
                let l = v.legal.as_ref().unwrap();
                Ok(l.max_raise_to
                    .or(l.short_all_in_to)
                    .map(Action::RaiseTo)
                    .unwrap_or(if l.check { Action::Check } else { Action::Call }))
            }
        }
        play(
            Rules::default(),
            vec![200; 3],
            0,
            seed,
            &mut [
                Box::new(Shove),
                Box::new(Scripted::Caller),
                Box::new(Scripted::Caller),
            ],
        )
        .unwrap()
        .record
    }

    #[test]
    fn holes_match_the_engine_deal() {
        let record = all_in_record(11);
        let holes = holes(&record);
        for event in &record.events {
            if let Event::Reveal { seat, cards } = event {
                assert_eq!(cards.as_slice(), holes[*seat].as_slice());
            }
        }
    }

    #[test]
    fn allin_adjustment_conserves_chips_and_is_deterministic() {
        for seed in 0..20 {
            let record = all_in_record(seed);
            let adjusted = allin_adjusted(&record).unwrap().expect("all-in preflop");
            let total: f64 = adjusted.iter().sum();
            let initial: Chips = record.initial_stacks.iter().sum();
            assert!((total - initial as f64).abs() < 1e-6, "seed {seed}");
            assert_eq!(adjusted, allin_adjusted(&record).unwrap().unwrap());
            assert!(adjusted.iter().all(|s| *s >= 0.0));
        }
        // A hand that ended by folding has no luck to remove.
        let folded = play(
            Rules::default(),
            vec![200; 2],
            0,
            3,
            &mut [Box::new(Scripted::Folder), Box::new(Scripted::Caller)],
        )
        .unwrap()
        .record;
        assert_eq!(allin_adjusted(&folded).unwrap(), None);
    }

    #[test]
    fn allin_adjustment_averages_out_the_board() {
        // Over many all-in hands the adjusted and actual results agree on average (unbiased),
        // and the adjusted results vary less.
        let (mut actual, mut adjusted) = (vec![], vec![]);
        for seed in 0..200 {
            let record = all_in_record(seed);
            actual.push(record.final_stacks[0] as f64 - 200.0);
            adjusted.push(allin_adjusted(&record).unwrap().unwrap()[0] - 200.0);
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let var = |v: &[f64]| {
            let m = mean(v);
            v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64
        };
        assert!((mean(&actual) - mean(&adjusted)).abs() < 40.0);
        assert!(var(&adjusted) < var(&actual) * 0.6);
    }

    #[test]
    fn grades_follow_pot_odds() {
        // The caller always calls; every graded decision is a call, and its loss is zero
        // exactly when its equity meets the price.
        let mut graded = 0;
        for seed in 0..30 {
            let record = all_in_record(seed);
            for grade in grade_seat(&record, 1).unwrap() {
                graded += 1;
                assert!(grade.called);
                assert!(grade.loss >= 0.0);
                assert_eq!(grade.loss == 0.0, grade.equity >= grade.required - 1e-12);
            }
        }
        assert!(graded > 0);
    }
}
