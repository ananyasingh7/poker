// ABOUTME: River grading against the scripted field: each river decision's alternatives are replayed
// ABOUTME: over every hand each bot could hold given its policy and its actions, with its real responses.
use crate::{
    arena::{Agent, Record},
    cards::{Card, Rng, deck},
    engine::{Action, Chips, Hand, PlayerView, Street},
    opponents::{Adaptive, Solid, Style},
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Bet and raise sizes tried, as fractions of the pot (after the call, when facing a bet).
pub const SIZES: [f64; 5] = [0.33, 0.5, 0.75, 1.0, 1.5];
/// Joint opponent holdings evaluated per decision when there are more than this many
/// combinations (multiway pots); fewer are enumerated exactly.
pub const JOINT_SAMPLES: usize = 400;

/// What kind of river mistake a decision was, judged by its best alternative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// No alternative did better.
    Correct,
    /// Checked when a bet would have won more.
    MissedBet,
    /// Bet when checking would have done better.
    BadBet,
    /// Bet or raised, but a different size would have done better.
    Sizing,
    /// Folded when calling or raising would have done better.
    BadFold,
    /// Called when folding would have done better.
    BadCall,
    /// Called when a raise would have done better.
    MissedRaise,
    /// Raised when calling or folding would have done better.
    BadRaise,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Correct => "correct",
            Self::MissedBet => "missed bet",
            Self::BadBet => "bad bet",
            Self::Sizing => "sizing",
            Self::BadFold => "bad fold",
            Self::BadCall => "bad call",
            Self::MissedRaise => "missed raise",
            Self::BadRaise => "bad raise",
        }
    }
}

/// One graded river (or, approximately, turn) decision. Values are the seat's expected stack at
/// the end of the hand, averaged over the opponents' possible holdings.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RiverGrade {
    /// Index of the hand in the session (0-based) and of the decision within the hand.
    pub hand: usize,
    pub decision: usize,
    pub seat: usize,
    /// River (exact) or turn (approximate: see `crate::turn`).
    pub street: Street,
    pub facing_bet: bool,
    pub pot: Chips,
    /// Holdings each live opponent could have, given its policy and what it did.
    pub range_sizes: Vec<usize>,
    pub chosen: Action,
    pub chosen_value: f64,
    pub best: Action,
    pub best_value: f64,
    /// Expected chips the chosen action gave up against the best alternative (0 when best).
    pub loss: f64,
    pub kind: Kind,
}

/// A scripted seat as it stood at a point in the session: `Adaptive` carries what it has
/// learned from earlier hands; every other style is stateless between hands.
pub(crate) enum Bot {
    Plain(Style),
    Adaptive(Adaptive),
}
impl Bot {
    /// A fresh policy that decides exactly as this seat would now.
    pub(crate) fn policy(&self) -> Box<dyn Agent> {
        match self {
            Self::Plain(style) => style.agent(),
            Self::Adaptive(a) => Box::new(Solid::new(a.params())),
        }
    }
}

/// Grades every river decision of the `graded` seats in one session, replaying the session
/// from its first hand so stateful bots learn exactly as they did. Every seat that is not
/// graded must be a scripted style.
///
/// The benchmark is a player who knows exactly how each bot plays but not its cards: each
/// opponent's possible holdings are the hole cards (among those the graded seat cannot see) for
/// which its policy would have made every move it made in the hand so far. Every alternative is
/// played out against each joint holding with the bots' real responses, and averaged. When a
/// bot raises after the graded seat acts, the seat answers fold or call, whichever is better
/// given what it can see (one choice per sequence of opponent actions, not per hidden card).
pub fn grade_session(
    records: &[Record],
    styles: &[Option<Style>],
    graded: &[usize],
) -> Result<Vec<RiverGrade>, String> {
    grade_with(RiverGrader::new(styles, graded)?, records)
}

/// `grade_session` plus approximate turn grades (`crate::turn`), told apart by `street`.
pub fn grade_session_with_turn(
    records: &[Record],
    styles: &[Option<Style>],
    graded: &[usize],
) -> Result<Vec<RiverGrade>, String> {
    grade_with(RiverGrader::with_turn(styles, graded)?, records)
}

fn grade_with(mut grader: RiverGrader, records: &[Record]) -> Result<Vec<RiverGrade>, String> {
    let mut grades = vec![];
    for (index, record) in records.iter().enumerate() {
        for mut g in grader.grade_hand(record)? {
            g.hand = index;
            grades.push(g);
        }
    }
    Ok(grades)
}

/// `grade_session` one hand at a time, for a table that is still playing: feed every finished
/// hand in order, so the stateful bots learn as they did at the table.
pub struct RiverGrader {
    bots: Vec<Option<Bot>>,
    graded: Vec<usize>,
    hands: usize,
    /// Also grade turn decisions, approximately (`crate::turn`). Off unless asked for: it takes
    /// several times as long as the river grade.
    turn: bool,
}
impl RiverGrader {
    pub fn new(styles: &[Option<Style>], graded: &[usize]) -> Result<Self, String> {
        let bots = styles
            .iter()
            .enumerate()
            .map(|(seat, style)| match style {
                _ if graded.contains(&seat) => Ok(None),
                Some(Style::Adaptive) => Ok(Some(Bot::Adaptive(Adaptive::default()))),
                Some(style) => Ok(Some(Bot::Plain(*style))),
                None => Err(format!(
                    "seat {seat} is neither graded nor a scripted style"
                )),
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            bots,
            graded: graded.to_vec(),
            hands: 0,
            turn: false,
        })
    }
    /// As `new`, also grading turn decisions approximately (`crate::turn`).
    pub fn with_turn(styles: &[Option<Style>], graded: &[usize]) -> Result<Self, String> {
        let mut grader = Self::new(styles, graded)?;
        grader.turn = true;
        Ok(grader)
    }
    /// Grades the graded seats' river decisions in the next hand of the session (`hand` in each
    /// grade counts the hands fed so far, from 0), then lets the bots learn from it.
    pub fn grade_hand(&mut self, record: &Record) -> Result<Vec<RiverGrade>, String> {
        if record.initial_stacks.len() != self.bots.len() {
            return Err("seat count differs from the styles".into());
        }
        let mut grades = vec![];
        let mut hand = rebuild(record, &deck(record.seed), 0)?;
        for (number, d) in record.decisions.iter().enumerate() {
            let view = hand.view(d.seat)?;
            if self.graded.contains(&d.seat) && view.street == Street::River {
                grades.push(grade(
                    record, &self.bots, self.hands, number, &view, d.applied,
                )?);
            } else if self.turn && self.graded.contains(&d.seat) && view.street == Street::Turn {
                grades.push(crate::turn::grade(
                    record, &self.bots, self.hands, number, &view, d.applied,
                )?);
            }
            hand.act(d.seat, d.applied)?;
        }
        // Stateful bots learn from the finished hand, as `arena::play` tells them.
        for (seat, bot) in self.bots.iter_mut().enumerate() {
            if let Some(Bot::Adaptive(a)) = bot {
                a.finish_hand(&hand.view(seat)?)?;
            }
        }
        self.hands += 1;
        Ok(grades)
    }
}

/// The hand dealt from `cards`, as it stood before decision `upto`.
pub(crate) fn rebuild(record: &Record, cards: &[Card], upto: usize) -> Result<Hand, String> {
    let mut hand = Hand::with_deck(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        cards.to_vec(),
    )?;
    for d in &record.decisions[..upto] {
        hand.act(d.seat, d.applied)?;
    }
    Ok(hand)
}

/// The record's deck with the given seats' hole cards replaced. Cards are popped from the end:
/// one card per seat starting left of the button, twice, then the board.
pub(crate) fn deal(record: &Record, holes: &[(usize, [Card; 2])]) -> Vec<Card> {
    let mut cards = deck(record.seed);
    let n = record.initial_stacks.len();
    for (seat, hole) in holes {
        for (round, card) in hole.iter().enumerate() {
            let pop = round * n + (seat + n - record.button - 1) % n;
            let target = cards.len() - 1 - pop;
            let from = cards.iter().position(|c| c == card).expect("card in deck");
            cards.swap(target, from);
        }
    }
    cards
}

/// Every hole-card pair `seat` could hold given its policy and its decisions before `upto`,
/// drawn from the cards the graded seat cannot see.
pub(crate) fn range(
    record: &Record,
    bot: &Bot,
    seat: usize,
    upto: usize,
    unseen: &[Card],
) -> Result<Vec<[Card; 2]>, String> {
    // The real views of `seat` (hole cards are swapped in below).
    let mut hand = rebuild(record, &deck(record.seed), 0)?;
    let mut spots = vec![];
    for d in &record.decisions[..upto] {
        if d.seat == seat {
            spots.push((hand.view(seat)?, d.applied));
        }
        hand.act(d.seat, d.applied)?;
    }
    let mut out = vec![];
    for i in 0..unseen.len() {
        for j in i + 1..unseen.len() {
            let hole = [unseen[i], unseen[j]];
            let mut policy = bot.policy();
            let consistent = spots.iter().all(|(view, applied)| {
                let mut view = view.clone();
                view.hole = hole;
                policy.decide(&view).is_ok_and(|a| a == *applied)
            });
            if consistent {
                out.push(hole);
            }
        }
    }
    Ok(out)
}

/// Joint holdings of the live opponents: all non-overlapping combinations when there are at
/// most `JOINT_SAMPLES`, otherwise that many deterministic samples.
pub(crate) fn joint(ranges: &[(usize, Vec<[Card; 2]>)], seed: u64) -> Vec<Vec<(usize, [Card; 2])>> {
    let overlaps = |chosen: &[(usize, [Card; 2])], hole: &[Card; 2]| {
        chosen
            .iter()
            .any(|(_, h)| h.iter().any(|c| hole.contains(c)))
    };
    let total: usize = ranges
        .iter()
        .map(|(_, r)| r.len())
        .try_fold(1usize, |p, n| p.checked_mul(n))
        .unwrap_or(usize::MAX);
    let mut out = vec![];
    if total <= JOINT_SAMPLES {
        let mut stack = vec![vec![]];
        for (seat, holes) in ranges {
            let mut next = vec![];
            for chosen in &stack {
                for hole in holes {
                    if !overlaps(chosen, hole) {
                        let mut c: Vec<(usize, [Card; 2])> = chosen.clone();
                        c.push((*seat, *hole));
                        next.push(c);
                    }
                }
            }
            stack = next;
        }
        out = stack;
    } else {
        let mut rng = Rng(seed);
        let mut attempts = 0;
        while out.len() < JOINT_SAMPLES && attempts < JOINT_SAMPLES * 20 {
            attempts += 1;
            let mut chosen = vec![];
            for (seat, holes) in ranges {
                let hole = holes[rng.below(holes.len() as u64) as usize];
                if overlaps(&chosen, &hole) {
                    break;
                }
                chosen.push((*seat, hole));
            }
            if chosen.len() == ranges.len() {
                out.push(chosen);
            }
        }
    }
    out
}

/// Every action tried at this spot: the passive options, a range of sizes, all-in, and the
/// action actually taken.
pub(crate) fn candidates(view: &PlayerView, chosen: Action) -> Result<Vec<Action>, String> {
    let legal = view.legal.as_ref().ok_or("not acting")?;
    let call = legal.call.unwrap_or(0);
    let committed = view.players[view.seat].street_committed;
    let mut out = vec![];
    if legal.check {
        out.push(Action::Check);
    } else {
        out.push(Action::Fold);
        out.push(Action::Call);
    }
    if let (Some(min), Some(max)) = (legal.min_raise_to, legal.max_raise_to) {
        let base = (view.pot + call) as f64;
        for fraction in SIZES {
            let target = committed + call + (base * fraction).round() as Chips;
            out.push(Action::RaiseTo(target.clamp(min, max)));
        }
        out.push(Action::RaiseTo(max));
    } else if let Some(all_in) = legal.short_all_in_to {
        out.push(Action::RaiseTo(all_in));
    }
    out.push(chosen);
    let mut unique = vec![];
    for action in out {
        if legal.accepts(action) && !unique.contains(&action) {
            unique.push(action);
        }
    }
    Ok(unique)
}

fn grade(
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
    let mut results = vec![];
    for action in candidates(view, chosen)? {
        results.push((
            action,
            value(record, bots, number, seat, action, &holdings)?,
        ));
    }
    finish(hand_index, number, view, chosen, &ranges, &results)
}

/// The grade of a decision from the value of every alternative tried.
pub(crate) fn finish(
    hand_index: usize,
    number: usize,
    view: &PlayerView,
    chosen: Action,
    ranges: &[(usize, Vec<[Card; 2]>)],
    results: &[(Action, f64)],
) -> Result<RiverGrade, String> {
    let chosen_value = results
        .iter()
        .find(|(a, _)| *a == chosen)
        .map(|(_, v)| *v)
        .ok_or("chosen action was not legal")?;
    // The best alternative; it must beat the chosen action by more than rounding to count.
    let (best, best_value) = results
        .iter()
        .fold((chosen, chosen_value), |(ba, bv), &(a, v)| {
            if v > bv + 1e-9 { (a, v) } else { (ba, bv) }
        });
    let facing_bet = view.legal.as_ref().is_some_and(|l| !l.check);
    let loss = best_value - chosen_value;
    let raise = |a: Action| matches!(a, Action::RaiseTo(_));
    let kind = if best == chosen {
        Kind::Correct
    } else if !facing_bet {
        match (raise(chosen), raise(best)) {
            (false, _) => Kind::MissedBet,
            (true, false) => Kind::BadBet,
            (true, true) => Kind::Sizing,
        }
    } else {
        match (chosen, best) {
            (Action::Fold, _) => Kind::BadFold,
            (Action::Call, Action::Fold) => Kind::BadCall,
            (Action::Call, _) => Kind::MissedRaise,
            (_, Action::RaiseTo(_)) => Kind::Sizing,
            _ => Kind::BadRaise,
        }
    };
    Ok(RiverGrade {
        hand: hand_index,
        decision: number,
        seat: view.seat,
        street: view.street,
        facing_bet,
        pot: view.pot,
        range_sizes: ranges.iter().map(|(_, r)| r.len()).collect(),
        chosen,
        chosen_value,
        best,
        best_value,
        loss,
        kind,
    })
}

/// How a play-out from the graded seat's action ended: the hand finished, or the seat must act
/// again after the opponents' actions `seen` (what it observed since its action).
enum Outcome {
    Done(Chips),
    Again {
        seen: Vec<(usize, Action)>,
        options: BTreeMap<String, Chips>,
    },
}

/// Expected final stack of `seat` when it takes `action` at decision `from`, averaged over
/// `holdings`. Later choices of the seat are made once per sequence of opponent actions it saw.
fn value(
    record: &Record,
    bots: &[Option<Bot>],
    from: usize,
    seat: usize,
    action: Action,
    holdings: &[Vec<(usize, [Card; 2])>],
) -> Result<f64, String> {
    let mut done = 0.0;
    let mut again: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for holding in holdings {
        match play_out(record, bots, &deal(record, holding), from, seat, &[action])? {
            Outcome::Done(stack) => done += stack as f64,
            Outcome::Again { seen, options } => {
                let entry = again.entry(format!("{seen:?}")).or_default();
                for (option, stack) in options {
                    *entry.entry(option).or_default() += stack as f64;
                }
            }
        }
    }
    let replies: f64 = again
        .values()
        .map(|options| options.values().copied().fold(f64::MIN, f64::max))
        .sum();
    Ok((done + replies) / holdings.len() as f64)
}

/// Plays the hand dealt from `cards` from decision `from`: the graded seat takes `path` (its
/// first action, then any forced replies), the bots answer with their policies. When the seat
/// must choose again, every passive option is played out and reported, keyed by name.
fn play_out(
    record: &Record,
    bots: &[Option<Bot>],
    cards: &[Card],
    from: usize,
    seat: usize,
    path: &[Action],
) -> Result<Outcome, String> {
    let mut hand = rebuild(record, cards, from)?;
    let mut policies: Vec<Option<Box<dyn Agent>>> =
        bots.iter().map(|b| b.as_ref().map(Bot::policy)).collect();
    let mut taken = 0;
    let mut seen = vec![];
    while let Some(actor) = hand.actor() {
        let legal = hand.legal();
        let action = if actor == seat {
            if taken == path.len() {
                if path.len() > 1 {
                    // A third choice in one river: rare; take the passive option.
                    if legal.check {
                        Action::Check
                    } else {
                        Action::Call
                    }
                } else {
                    let options: Vec<Action> = if legal.check {
                        vec![Action::Check]
                    } else {
                        [Action::Fold, Action::Call]
                            .into_iter()
                            .filter(|a| legal.accepts(*a))
                            .collect()
                    };
                    let mut results = BTreeMap::new();
                    for option in options {
                        let mut next = path.to_vec();
                        next.push(option);
                        let stack = match play_out(record, bots, cards, from, seat, &next)? {
                            Outcome::Done(stack) => stack,
                            Outcome::Again { .. } => unreachable!("path is longer than one"),
                        };
                        results.insert(format!("{option:?}"), stack);
                    }
                    return Ok(Outcome::Again {
                        seen,
                        options: results,
                    });
                }
            } else {
                taken += 1;
                path[taken - 1]
            }
        } else {
            let policy = policies[actor]
                .as_mut()
                .ok_or("an ungraded seat without a policy acted")?;
            let a = match policy.decide(&hand.view(actor)?) {
                Ok(a) if legal.accepts(a) => a,
                _ => legal.fallback(),
            };
            if taken > 0 {
                seen.push((actor, a));
            }
            a
        };
        hand.act(actor, action)?;
    }
    Ok(Outcome::Done(hand.stacks()[seat]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arena::{Scripted, Session},
        evaluation::holes,
    };

    const FIELD: [Style; 6] = [
        Style::Solid,
        Style::Station,
        Style::Overfolder,
        Style::Adaptive,
        Style::Tight,
        Style::Solid,
    ];

    fn session(field: &[Style], hands: usize, seed: u64) -> Vec<Record> {
        let mut session = Session::new(field.len(), seed).unwrap();
        session.reset_stacks = true;
        let mut agents: Vec<Box<dyn Agent>> = field.iter().map(|s| s.agent()).collect();
        (0..hands)
            .map(|_| session.next_hand(&mut agents).unwrap().record)
            .collect()
    }

    #[test]
    fn the_real_holding_is_in_range_and_replays_the_recorded_result() {
        // An all-bot table, grading seat 0 as if it were the model. The bots' real cards must
        // be consistent with their policies (including the adaptive bot after it has learned),
        // and replaying the real deal with the action taken must reproduce the record.
        let records = session(&FIELD, 400, 11);
        let styles: Vec<Option<Style>> = FIELD.iter().map(|s| Some(*s)).collect();
        let mut bots: Vec<Option<Bot>> = vec![None];
        bots.extend(FIELD[1..].iter().map(|s| {
            Some(match s {
                Style::Adaptive => Bot::Adaptive(Adaptive::default()),
                s => Bot::Plain(*s),
            })
        }));
        let mut checked = 0;
        for (index, record) in records.iter().enumerate() {
            let real = holes(record);
            let mut hand = rebuild(record, &deck(record.seed), 0).unwrap();
            for (number, d) in record.decisions.iter().enumerate() {
                let view = hand.view(d.seat).unwrap();
                if d.seat == 0 && view.street == Street::River {
                    let seen: Vec<Card> = view.hole.iter().chain(&view.board).copied().collect();
                    let unseen: Vec<Card> = (0..52u8)
                        .map(|c| Card::try_from(c).unwrap())
                        .filter(|c| !seen.contains(c))
                        .collect();
                    for p in view.players.iter().filter(|p| !p.folded && p.seat != 0) {
                        let r = range(
                            record,
                            bots[p.seat].as_ref().unwrap(),
                            p.seat,
                            number,
                            &unseen,
                        )
                        .unwrap();
                        let h = real[p.seat];
                        assert!(
                            r.contains(&h) || r.contains(&[h[1], h[0]]),
                            "hand {index}: seat {} real cards outside its range",
                            p.seat
                        );
                    }
                    let later = record.decisions[number + 1..].iter().any(|x| x.seat == 0);
                    if !later {
                        let dealt = deal(record, &(1..6).map(|s| (s, real[s])).collect::<Vec<_>>());
                        assert_eq!(
                            dealt,
                            deck(record.seed),
                            "dealing the real cards changes nothing"
                        );
                        match play_out(record, &bots, &dealt, number, 0, &[d.applied]).unwrap() {
                            Outcome::Done(stack) => assert_eq!(stack, record.final_stacks[0]),
                            Outcome::Again { .. } => panic!("no later decision expected"),
                        }
                        checked += 1;
                    }
                }
                hand.act(d.seat, d.applied).unwrap();
            }
            for (seat, bot) in bots.iter_mut().enumerate() {
                if let Some(Bot::Adaptive(a)) = bot {
                    a.finish_hand(&hand.view(seat).unwrap()).unwrap();
                }
            }
        }
        assert!(checked > 30, "only {checked} river decisions checked");
        // And the whole session grades consistently.
        let grades = grade_session(&records[..120], &styles, &[0]).unwrap();
        assert!(!grades.is_empty());
        for g in &grades {
            assert!(g.best_value >= g.chosen_value);
            assert!((g.loss - (g.best_value - g.chosen_value)).abs() < 1e-9);
            assert_eq!(g.kind == Kind::Correct, g.best == g.chosen);
            assert!(g.range_sizes.iter().all(|n| *n > 0));
        }
    }

    #[test]
    fn checking_the_nuts_to_the_station_is_a_missed_bet_and_bluffing_it_is_a_bad_bet() {
        // Seat 0 always checks or calls; the station never folds a pair or ace-high and never
        // bets. Checking a near-certain winner to it misses value; betting nothing into it
        // (here: replaying a raise where seat 0 held the worst hand) only gets called.
        let mut missed = false;
        for seed in 0..300u64 {
            let mut session = Session::new(2, seed).unwrap();
            session.reset_stacks = true;
            let mut agents: Vec<Box<dyn Agent>> =
                vec![Box::new(Scripted::Caller), Style::Station.agent()];
            let played: Vec<Record> = (0..10)
                .map(|_| session.next_hand(&mut agents).unwrap().record)
                .collect();
            let grades = grade_session(&played, &[None, Some(Style::Station)], &[0]).unwrap();
            for g in &grades {
                if !g.facing_bet && g.chosen == Action::Check && g.kind == Kind::MissedBet {
                    assert!(matches!(g.best, Action::RaiseTo(_)));
                    assert!(g.loss > 0.0);
                    missed = true;
                }
            }
            if missed {
                return;
            }
        }
        panic!("no missed river bet against the station found");
    }

    #[test]
    fn a_seat_that_is_neither_graded_nor_scripted_is_refused() {
        let records = session(&[Style::Solid; 6], 2, 1);
        let mut styles: Vec<Option<Style>> = vec![Some(Style::Solid); 6];
        styles[3] = None;
        assert!(grade_session(&records, &styles, &[0]).is_err());
    }
}
