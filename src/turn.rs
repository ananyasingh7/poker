// ABOUTME: Approximate turn grading against the scripted field: each turn alternative is played out over
// ABOUTME: the bots' possible holdings and sampled river cards, with the seat's later play chosen by what it saw.
//!
//! The river grade (`crate::river`) is exact because the hand ends after the river. A turn
//! decision also depends on how the seat plays the river, which the record does not say for the
//! alternatives it did not take. Here the seat's later play is a stand-in: at each later choice it
//! may check, bet the pot or go all-in (when it can check), or fold or call (when facing a bet),
//! and takes the option that does best over every deal in which it saw the same things (the
//! river card and the opponents' actions). That is a player who knows how the bots play but not
//! their cards, the benchmark of the river grade, restricted to three river bet sizes and no
//! raises. Its values are therefore estimates, comparable across conditions on the same deals.
//!
//! Every river card the seat cannot rule out is tried, each with up to `HOLDINGS_PER_RIVER`
//! opponent holdings drawn deterministically from the ranges, so each river card is judged over
//! the same number of deals. The later choices are made over those few deals, so they see more
//! than a real player would (an optimistic continuation); every alternative gets the same
//! continuation, which is what makes the comparison between them fair.

use crate::{
    arena::{Agent, Record},
    cards::{Card, Rng},
    engine::{Action, Chips, PlayerView},
    river::{Bot, RiverGrade, candidates, deal, finish, joint, range, rebuild},
};
use std::collections::BTreeMap;

/// Opponent holdings tried with each possible river card. Fewer would let the seat's later
/// choices fit the few deals behind each river card (close to seeing the cards); more costs
/// time linearly.
pub const HOLDINGS_PER_RIVER: usize = 12;

/// Grades one turn decision (`number` in the record) of `view.seat`.
pub(crate) fn grade(
    record: &Record,
    bots: &[Option<Bot>],
    hand_index: usize,
    number: usize,
    view: &PlayerView,
    chosen: Action,
) -> Result<RiverGrade, String> {
    let seat = view.seat;
    let seen: Vec<Card> = view.hole.iter().chain(&view.board).copied().collect();
    let unseen: Vec<Card> = (0..52u8)
        .map(|c| Card::try_from(c).expect("valid card"))
        .filter(|c| !seen.contains(c))
        .collect();
    let mut ranges = vec![];
    for p in view.players.iter().filter(|p| !p.folded && p.seat != seat) {
        let bot = bots[p.seat]
            .as_ref()
            .ok_or("a live opponent is not a scripted style")?;
        ranges.push((p.seat, range(record, bot, p.seat, number, &unseen)?));
    }
    let holdings = joint(&ranges, record.seed ^ number as u64);
    if holdings.is_empty() {
        return Err(format!(
            "hand {hand_index}: no holdings consistent with the bots' actions"
        ));
    }
    let samples = samples(&holdings, &unseen, record.seed ^ (number as u64) << 20);
    let decks: Vec<Vec<Card>> = samples
        .iter()
        .map(|(holding, river)| with_river(record, &deal(record, holding), *river))
        .collect();
    let mut results = vec![];
    for action in candidates(view, chosen)? {
        let mut total = Value::default();
        for cards in &decks {
            total.add(explore(record, bots, cards, number, seat, &[action])?);
        }
        results.push((action, total.best() / decks.len() as f64));
    }
    finish(hand_index, number, view, chosen, &ranges, &results)
}

/// Live opponents' hole cards in one deal.
type Holding = Vec<(usize, [Card; 2])>;

/// (Holding, river card) pairs: every unseen river card, each with up to `HOLDINGS_PER_RIVER`
/// distinct holdings that do not use it.
fn samples(holdings: &[Holding], unseen: &[Card], seed: u64) -> Vec<(Holding, Card)> {
    let mut rng = Rng(seed);
    let mut out = vec![];
    for river in unseen {
        let mut free: Vec<&Holding> = holdings
            .iter()
            .filter(|h| !h.iter().any(|(_, hole)| hole.contains(river)))
            .collect();
        for _ in 0..HOLDINGS_PER_RIVER.min(free.len()) {
            let pick = rng.below(free.len() as u64) as usize;
            out.push((free.swap_remove(pick).clone(), *river));
        }
    }
    out
}

/// `cards` with `river` moved to the position the river is dealt from (after the hole cards,
/// three burns, the flop and the turn; cards are popped from the end).
fn with_river(record: &Record, cards: &[Card], river: Card) -> Vec<Card> {
    let mut cards = cards.to_vec();
    let pops = 2 * record.initial_stacks.len() + 7;
    let target = cards.len() - 1 - pops;
    let from = cards
        .iter()
        .position(|c| *c == river)
        .expect("card in deck");
    cards.swap(target, from);
    cards
}

/// A play-out from the graded action: the hand ended, or the seat chose again, keyed by what it
/// had seen, with the play-out of each option.
enum Node {
    Done(Chips),
    Choice {
        seen: String,
        options: Vec<(Action, Node)>,
    },
}

/// Final stacks summed over samples, with the seat's later choices merged by what it saw: at each
/// key the seat takes the option with the highest total over all samples that reached it.
#[derive(Default)]
struct Value {
    done: f64,
    choices: BTreeMap<String, BTreeMap<String, Value>>,
}
impl Value {
    fn add(&mut self, node: Node) {
        match node {
            Node::Done(stack) => self.done += stack as f64,
            Node::Choice { seen, options } => {
                let at = self.choices.entry(seen).or_default();
                for (action, child) in options {
                    at.entry(format!("{action:?}")).or_default().add(child);
                }
            }
        }
    }
    fn best(&self) -> f64 {
        self.done
            + self
                .choices
                .values()
                .map(|options| {
                    options
                        .values()
                        .map(Value::best)
                        .fold(f64::NEG_INFINITY, f64::max)
                })
                .sum::<f64>()
    }
}

/// The seat's options at a later choice: check, a pot bet or all-in when it can check; fold or
/// call when facing a bet. No later raises.
fn later_options(view: &PlayerView) -> Vec<Action> {
    let Some(legal) = view.legal.as_ref() else {
        return vec![];
    };
    let mut out = vec![];
    if legal.check {
        out.push(Action::Check);
        if let (Some(min), Some(max)) = (legal.min_raise_to, legal.max_raise_to) {
            let committed = view.players[view.seat].street_committed;
            out.push(Action::RaiseTo((committed + view.pot).clamp(min, max)));
            out.push(Action::RaiseTo(max));
        }
    } else {
        out.push(Action::Fold);
        out.push(Action::Call);
    }
    let mut unique = vec![];
    for action in out {
        if legal.accepts(action) && !unique.contains(&action) {
            unique.push(action);
        }
    }
    unique
}

/// Plays the hand dealt from `cards` from decision `from`: the seat takes `path`, the bots answer
/// with their policies; at the seat's next choice every option is explored.
fn explore(
    record: &Record,
    bots: &[Option<Bot>],
    cards: &[Card],
    from: usize,
    seat: usize,
    path: &[Action],
) -> Result<Node, String> {
    let mut hand = rebuild(record, cards, from)?;
    let mut policies: Vec<Option<Box<dyn Agent>>> =
        bots.iter().map(|b| b.as_ref().map(Bot::policy)).collect();
    let mut taken = 0;
    let mut seen = vec![];
    while let Some(actor) = hand.actor() {
        let legal = hand.legal();
        let action = if actor == seat {
            if taken == path.len() {
                let view = hand.view(seat)?;
                let mut options = vec![];
                for option in later_options(&view) {
                    let mut next = path.to_vec();
                    next.push(option);
                    options.push((option, explore(record, bots, cards, from, seat, &next)?));
                }
                // Everything seen since the graded action: opponents' moves and the board. The
                // seat's own earlier choices are the path to this node, so need no key.
                return Ok(Node::Choice {
                    seen: format!("{seen:?}{:?}", view.board),
                    options,
                });
            }
            taken += 1;
            path[taken - 1]
        } else {
            let policy = policies[actor]
                .as_mut()
                .ok_or("an ungraded seat without a policy acted")?;
            let a = match policy.decide(&hand.view(actor)?) {
                Ok(a) if legal.accepts(a) => a,
                _ => legal.fallback(),
            };
            seen.push((actor, a));
            a
        };
        hand.act(actor, action)?;
    }
    Ok(Node::Done(hand.stacks()[seat]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arena::Session,
        cards::deck,
        engine::Street,
        evaluation::holes,
        opponents::Style,
        river::{Kind, grade_session_with_turn},
    };

    const FIELD: [Style; 6] = [
        Style::Solid,
        Style::Station,
        Style::Overfolder,
        Style::Adaptive,
        Style::Tight,
        Style::Solid,
    ];

    fn session(hands: usize, seed: u64) -> Vec<Record> {
        let mut session = Session::new(FIELD.len(), seed).unwrap();
        session.reset_stacks = true;
        let mut agents: Vec<Box<dyn Agent>> = FIELD.iter().map(|s| s.agent()).collect();
        (0..hands)
            .map(|_| session.next_hand(&mut agents).unwrap().record)
            .collect()
    }

    #[test]
    fn the_real_river_card_and_holdings_rebuild_the_real_deck() {
        let records = session(60, 5);
        let mut checked = 0;
        for record in &records {
            let real = holes(record);
            let mut hand = rebuild(record, &deck(record.seed), 0).unwrap();
            for d in &record.decisions {
                hand.act(d.seat, d.applied).unwrap();
            }
            let view = hand.view(0).unwrap();
            if view.board.len() == 5 {
                let holding: Vec<(usize, [Card; 2])> = (1..6).map(|s| (s, real[s])).collect();
                let cards = with_river(record, &deal(record, &holding), view.board[4]);
                assert_eq!(cards, deck(record.seed), "the real deal is unchanged");
                checked += 1;
            }
        }
        assert!(checked > 3, "only {checked} hands reached the river");
    }

    #[test]
    fn turn_grades_are_consistent_and_leave_river_grades_unchanged() {
        let records = session(80, 11);
        let styles: Vec<Option<Style>> = FIELD.iter().map(|s| Some(*s)).collect();
        let river_only = crate::river::grade_session(&records, &styles, &[0]).unwrap();
        let both = grade_session_with_turn(&records, &styles, &[0]).unwrap();
        let rivers: Vec<_> = both
            .iter()
            .filter(|g| g.street == Street::River)
            .cloned()
            .collect();
        assert_eq!(
            rivers, river_only,
            "turn grading must not change river grades"
        );
        let turns: Vec<_> = both.iter().filter(|g| g.street == Street::Turn).collect();
        assert!(
            turns.len() > 5,
            "only {} turn decisions graded",
            turns.len()
        );
        for g in turns {
            assert!(g.best_value >= g.chosen_value);
            assert!((g.loss - (g.best_value - g.chosen_value)).abs() < 1e-9);
            assert_eq!(g.kind == Kind::Correct, g.best == g.chosen);
            assert!(g.range_sizes.iter().all(|n| *n > 0));
        }
    }

    #[test]
    fn a_later_bet_is_taken_only_where_it_pays() {
        // Two samples with the same view after the graded action: betting wins 10 in one and
        // loses 4 in the other, checking wins nothing in both. One choice for both: bet (+6).
        let mut v = Value::default();
        let node = |bet: Chips| Node::Choice {
            seen: "same".into(),
            options: vec![
                (Action::Check, Node::Done(100)),
                (Action::RaiseTo(10), Node::Done(bet)),
            ],
        };
        v.add(node(110));
        v.add(node(96));
        assert_eq!(v.best(), 206.0);
        // Different views: each takes its own best (bet in one, check in the other).
        let mut v = Value::default();
        v.add(node(110));
        v.add(Node::Choice {
            seen: "other".into(),
            options: vec![
                (Action::Check, Node::Done(100)),
                (Action::RaiseTo(10), Node::Done(96)),
            ],
        });
        assert_eq!(v.best(), 210.0);
    }
}
