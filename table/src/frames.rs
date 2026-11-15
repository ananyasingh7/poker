// ABOUTME: Turns a hand (its deal and the decisions so far) into the sequence of table pictures a
// ABOUTME: spectator steps through: each action, the bets being collected, board cards, showdown, payout.
use poker_lab::{
    arena::{Decision, Record},
    cards::Card,
    engine::{Action, Chips, Event, Hand, Rules, Street},
};

/// Everything that fixes a hand's deal. Referee-side only: the seed never reaches an agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandStart {
    pub rules: Rules,
    pub stacks: Vec<Chips>,
    pub button: usize,
    pub seed: u64,
}
impl HandStart {
    pub fn of(record: &Record) -> Self {
        Self {
            rules: record.rules.clone(),
            stacks: record.initial_stacks.clone(),
            button: record.button,
            seed: record.seed,
        }
    }
    pub fn hand(&self) -> Result<Hand, String> {
        Hand::new(
            self.rules.clone(),
            self.stacks.clone(),
            self.button,
            self.seed,
        )
    }
    /// Every seat's hole cards.
    pub fn holes(&self) -> Result<Vec<[Card; 2]>, String> {
        let hand = self.hand()?;
        (0..self.stacks.len())
            .map(|seat| Ok(hand.view(seat)?.hole))
            .collect()
    }
}

/// What a seat last did on the current street, shown beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    SmallBlind,
    BigBlind,
    Fold,
    Check,
    Call,
    Bet,
    Raise,
    AllIn,
}
impl Label {
    pub fn text(self) -> &'static str {
        match self {
            Self::SmallBlind => "SB",
            Self::BigBlind => "BB",
            Self::Fold => "FOLD",
            Self::Check => "CHECK",
            Self::Call => "CALL",
            Self::Bet => "BET",
            Self::Raise => "RAISE",
            Self::AllIn => "ALL IN",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeatState {
    pub stack: Chips,
    /// Chips in front of the seat on this street, not yet in the pot.
    pub bet: Chips,
    pub folded: bool,
    pub label: Option<Label>,
    /// Hole cards turned face up at showdown.
    pub shown: bool,
    /// Chips collected from the pot at the end of the hand.
    pub won: Chips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Cards dealt and blinds posted.
    Deal,
    /// A seat acted.
    Action,
    /// New board cards.
    Board,
    Showdown,
    /// The pot has been paid out; stacks are final.
    Payout,
}

/// One picture of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub kind: Kind,
    pub street: Street,
    pub board: Vec<Card>,
    /// Chips already collected in the middle (bets in front of seats are not included).
    pub pot: Chips,
    pub seats: Vec<SeatState>,
    /// The seat to act next, if the hand is waiting on one.
    pub actor: Option<usize>,
    /// Index into the hand's decisions of the action this picture shows.
    pub decision: Option<usize>,
    /// How many decisions had been made when this picture was taken.
    pub decisions_made: usize,
    pub caption: String,
}
impl Frame {
    /// All chips on the table: collected pot plus bets in front of seats.
    pub fn total_pot(&self) -> Chips {
        self.pot + self.seats.iter().map(|s| s.bet).sum::<Chips>()
    }
}

pub fn action_text(action: Action) -> String {
    match action {
        Action::Fold => "folds".into(),
        Action::Check => "checks".into(),
        Action::Call => "calls".into(),
        Action::RaiseTo(n) => format!("raises to {n}"),
    }
}

/// The pictures for a hand after `decisions`. `names` label seats in captions. A finished hand
/// ends with a `Payout` picture; an unfinished one ends waiting on the next seat.
pub fn build(
    start: &HandStart,
    decisions: &[Decision],
    names: &[String],
) -> Result<Vec<Frame>, String> {
    let mut hand = start.hand()?;
    let n = start.stacks.len();
    let first = hand.view(0)?;
    let mut seats: Vec<SeatState> = first
        .players
        .iter()
        .map(|p| SeatState {
            stack: p.stack,
            bet: p.street_committed,
            folded: false,
            label: None,
            shown: false,
            won: 0,
        })
        .collect();
    let mut blinds = hand.events().iter().filter_map(|e| match e {
        Event::Blind { seat, .. } => Some(*seat),
        _ => None,
    });
    if let Some(seat) = blinds.next() {
        seats[seat].label = Some(Label::SmallBlind);
    }
    if let Some(seat) = blinds.next() {
        seats[seat].label = Some(Label::BigBlind);
    }
    let mut street = Street::Preflop;
    let mut board: Vec<Card> = vec![];
    let mut pot: Chips = 0;
    let mut frames = vec![Frame {
        kind: Kind::Deal,
        street,
        board: board.clone(),
        pot,
        seats: seats.clone(),
        actor: hand.actor(),
        decision: None,
        decisions_made: 0,
        caption: format!(
            "Blinds {}/{}",
            start.rules.small_blind, start.rules.big_blind
        ),
    }];
    let name = |seat: usize| {
        names
            .get(seat)
            .cloned()
            .unwrap_or_else(|| format!("Seat {seat}"))
    };
    for (index, decision) in decisions.iter().enumerate() {
        let legal = hand.legal();
        let seen = hand.events().len();
        hand.act(decision.seat, decision.applied)?;
        let highest = seats.iter().map(|s| s.bet).max().unwrap_or(0);
        let seat = &mut seats[decision.seat];
        let mut caption = format!("{} {}", name(decision.seat), action_text(decision.applied));
        match decision.applied {
            Action::Fold => {
                seat.folded = true;
                seat.label = Some(Label::Fold);
            }
            Action::Check => seat.label = Some(Label::Check),
            Action::Call => {
                let amount = legal.call.ok_or("call without a bet to call")?;
                seat.stack -= amount;
                seat.bet += amount;
                seat.label = Some(Label::Call);
                caption = format!("{} calls {amount}", name(decision.seat));
            }
            Action::RaiseTo(to) => {
                seat.stack -= to - seat.bet;
                seat.bet = to;
                seat.label = Some(if highest == 0 {
                    Label::Bet
                } else {
                    Label::Raise
                });
                if highest == 0 {
                    caption = format!("{} bets {to}", name(decision.seat));
                }
            }
        }
        if seat.stack == 0 && !seat.folded && decision.applied != Action::Check {
            seat.label = Some(Label::AllIn);
            caption.push_str(" (all in)");
        }
        if decision.error.is_some() {
            caption.push_str(" [fallback]");
        }
        let after: Vec<Event> = hand.events()[seen + 1..].to_vec();
        let made = index + 1;
        frames.push(Frame {
            kind: Kind::Action,
            street,
            board: board.clone(),
            pot,
            seats: seats.clone(),
            actor: if after.is_empty() { hand.actor() } else { None },
            decision: Some(index),
            decisions_made: made,
            caption,
        });
        if after.is_empty() {
            continue;
        }
        // The betting round is over: bets go to the middle before anything else happens.
        for seat in &mut seats {
            pot += seat.bet;
            seat.bet = 0;
            if !seat.folded {
                seat.label = None;
            }
        }
        let mut revealed = false;
        let mut paid = false;
        for event in &after {
            match event {
                Event::Board {
                    street: next,
                    cards,
                } => {
                    street = *next;
                    board = cards.clone();
                    frames.push(Frame {
                        kind: Kind::Board,
                        street,
                        board: board.clone(),
                        pot,
                        seats: seats.clone(),
                        actor: None,
                        decision: None,
                        decisions_made: made,
                        caption: format!("{street:?}"),
                    });
                }
                Event::Reveal { seat, .. } => {
                    seats[*seat].shown = true;
                    revealed = true;
                }
                Event::Refund { seat, amount } => {
                    if revealed && !paid {
                        frames.push(showdown(street, &board, pot, &seats, made));
                        revealed = false;
                    }
                    seats[*seat].stack += amount;
                    pot -= amount;
                }
                Event::Award {
                    winners, shares, ..
                } => {
                    if revealed && !paid {
                        frames.push(showdown(street, &board, pot, &seats, made));
                        revealed = false;
                    }
                    paid = true;
                    for (winner, share) in winners.iter().zip(shares) {
                        seats[*winner].stack += share;
                        seats[*winner].won += share;
                        pot -= share;
                    }
                }
                Event::Blind { .. } | Event::Act { .. } => {}
            }
        }
        if hand.is_complete() {
            let winners: Vec<String> = (0..n)
                .filter(|s| seats[*s].won > 0)
                .map(|s| format!("{} wins {}", name(s), seats[s].won))
                .collect();
            frames.push(Frame {
                kind: Kind::Payout,
                street: Street::Complete,
                board: board.clone(),
                pot,
                seats: seats.clone(),
                actor: None,
                decision: None,
                decisions_made: made,
                caption: winners.join(", "),
            });
        } else if let Some(last) = frames.last_mut() {
            last.actor = hand.actor();
        }
    }
    Ok(frames)
}

fn showdown(street: Street, board: &[Card], pot: Chips, seats: &[SeatState], made: usize) -> Frame {
    Frame {
        kind: Kind::Showdown,
        street,
        board: board.to_vec(),
        pot,
        seats: seats.to_vec(),
        actor: None,
        decision: None,
        decisions_made: made,
        caption: "Showdown".into(),
    }
}

/// The pictures of a completed hand.
pub fn of_record(record: &Record, names: &[String]) -> Result<Vec<Frame>, String> {
    build(&HandStart::of(record), &record.decisions, names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use poker_lab::{
        arena::{Agent, Scripted, Session},
        cards::Rng,
    };

    fn random_table(seats: usize, seed: u64) -> Vec<Box<dyn Agent>> {
        (0..seats)
            .map(|seat| Box::new(Scripted::Random(Rng(seed + seat as u64))) as Box<dyn Agent>)
            .collect()
    }

    /// Every picture must agree with the engine: chips are conserved, the money in front of
    /// each seat matches the referee's view at that point, and the last picture has the final stacks.
    #[test]
    fn frames_match_the_engine_on_random_hands() {
        for seats in [2, 3, 6] {
            let mut session = Session::new(seats, 7 + seats as u64).unwrap();
            let mut agents = random_table(seats, 99);
            let (mut showdowns, mut side_pots) = (0, 0);
            for _ in 0..300 {
                let record = session.next_hand(&mut agents).unwrap().record;
                let start = HandStart::of(&record);
                let frames = of_record(&record, &[]).unwrap();
                let total: Chips = record.initial_stacks.iter().sum();
                for frame in &frames {
                    let on_table: Chips =
                        frame.seats.iter().map(|s| s.stack).sum::<Chips>() + frame.total_pot();
                    assert_eq!(on_table, total, "chips conserved in every picture");
                }
                let last = frames.last().unwrap();
                assert_eq!(last.kind, Kind::Payout);
                assert_eq!(last.pot, 0);
                assert_eq!(
                    last.seats.iter().map(|s| s.stack).collect::<Vec<_>>(),
                    record.final_stacks
                );
                showdowns += usize::from(frames.iter().any(|f| f.kind == Kind::Showdown));
                side_pots += usize::from(
                    record
                        .events
                        .iter()
                        .filter(|e| matches!(e, Event::Award { .. }))
                        .count()
                        > 1,
                );
                // Each prefix of the hand ends in a picture that matches the referee's state.
                let mut hand = start.hand().unwrap();
                for (made, decision) in record.decisions.iter().enumerate() {
                    let partial = build(&start, &record.decisions[..made], &[]).unwrap();
                    let picture = partial.last().unwrap();
                    let view = hand.view(0).unwrap();
                    assert_eq!(picture.actor, hand.actor());
                    assert_eq!(picture.board, view.board);
                    assert_eq!(picture.total_pot(), view.pot);
                    for (seat, player) in picture.seats.iter().zip(&view.players) {
                        assert_eq!(
                            (seat.stack, seat.bet, seat.folded),
                            (player.stack, player.street_committed, player.folded)
                        );
                    }
                    hand.act(decision.seat, decision.applied).unwrap();
                }
            }
            assert!(showdowns > 20, "the sample must include showdowns");
            if seats > 2 {
                assert!(side_pots > 0, "the sample must include side pots");
            }
        }
    }

    #[test]
    fn a_walk_shows_the_blinds_then_pays_the_big_blind() {
        let mut session = Session::new(3, 1).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = vec![
            Box::new(Scripted::Folder),
            Box::new(Scripted::Folder),
            Box::new(Scripted::Folder),
        ];
        let record = session.next_hand(&mut agents).unwrap().record;
        let names: Vec<String> = ["A", "B", "C"].iter().map(|s| s.to_string()).collect();
        let frames = of_record(&record, &names).unwrap();
        assert_eq!(frames[0].kind, Kind::Deal);
        assert_eq!(frames[0].seats[1].label, Some(Label::SmallBlind));
        assert_eq!(frames[0].seats[2].label, Some(Label::BigBlind));
        assert_eq!(frames[0].seats[2].bet, 2);
        let last = frames.last().unwrap();
        assert_eq!(last.caption, "C wins 2");
        assert_eq!(last.seats[2].won, 2);
        assert_eq!(last.seats[2].stack, 201);
    }
}
