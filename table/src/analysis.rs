// ABOUTME: Per-hand analysis of the watched seat using the lab's own evaluators: all-in luck
// ABOUTME: adjustment, pot-odds grades of call/fold decisions, and agreement with the reference rule.
use poker_lab::{
    arena::Record,
    cards::Card,
    engine::{Action, Chips, Event, Hand, Street},
    equity::equity_vs,
    evaluation::{self, GRADE_SAMPLES},
    scaffold,
};

/// A call-or-fold decision scored against the pot odds (see `evaluation::grade_seat`).
#[derive(Clone, Debug, PartialEq)]
pub struct Graded {
    pub called: bool,
    /// Equity needed for a call to break even.
    pub required: f64,
    /// Immediate expected chips of calling; folding is 0.
    pub ev_call: f64,
    /// Chips given up against the better of call and fold (0 when correct).
    pub loss: f64,
}
impl Graded {
    pub fn verdict(&self) -> &'static str {
        match (self.called, self.loss > 0.0) {
            (true, false) => "good call",
            (true, true) => "bad call",
            (false, false) => "good fold",
            (false, true) => "bad fold",
        }
    }
    /// The better of the two graded options and its expected chips.
    pub fn best(&self) -> (&'static str, f64) {
        if self.ev_call > 0.0 {
            ("call", self.ev_call)
        } else {
            ("fold", 0.0)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecisionNote {
    /// Index into the record's decisions.
    pub index: usize,
    pub street: Street,
    pub pot: Chips,
    pub to_call: Chips,
    pub action: Action,
    /// Why the referee played a fallback instead of the seat's own answer, if it did.
    pub fallback: Option<String>,
    /// Showdown share against the cards the remaining opponents actually held.
    pub equity: f64,
    pub opponents: usize,
    /// Present when the seat faced a bet and called or folded; raises and checks are not graded.
    pub graded: Option<Graded>,
    /// What the lab's reference rule (`solid`) would have done.
    pub rule: Option<Action>,
}
impl DecisionNote {
    pub fn agrees_with_rule(&self) -> Option<bool> {
        self.rule
            .map(|rule| scaffold::kind(rule) == scaffold::kind(self.action))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandAnalysis {
    pub seat: usize,
    pub position: &'static str,
    pub holes: Vec<[Card; 2]>,
    pub big_blind: Chips,
    /// Chips won or lost by the seat.
    pub net: i64,
    /// The same with all-in luck removed; `None` when no luck followed the last decision.
    pub adjusted_net: Option<f64>,
    pub reached_showdown: bool,
    pub decisions: Vec<DecisionNote>,
    /// Chips the seat's graded call/fold mistakes gave up in this hand.
    pub mistake_cost: f64,
}
impl HandAnalysis {
    pub fn net_bb(&self) -> f64 {
        self.net as f64 / self.big_blind as f64
    }
    /// Luck-adjusted result in big blinds (the actual result when nothing needed adjusting).
    pub fn adjusted_bb(&self) -> f64 {
        self.adjusted_net.unwrap_or(self.net as f64) / self.big_blind as f64
    }
}

pub fn analyse(record: &Record, seat: usize) -> Result<HandAnalysis, String> {
    let holes = evaluation::holes(record);
    let seats = record.initial_stacks.len();
    if seat >= seats {
        return Err("analysed seat is not at the table".into());
    }
    let mut grades = evaluation::grade_seat(record, seat)?.into_iter();
    let mut hand = Hand::new(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        record.seed,
    )?;
    let mut decisions = vec![];
    for (index, decision) in record.decisions.iter().enumerate() {
        if decision.seat == seat {
            let view = hand.view(seat)?;
            let to_call = view.legal.as_ref().and_then(|l| l.call).unwrap_or(0);
            let opponents: Vec<[Card; 2]> = view
                .players
                .iter()
                .filter(|p| !p.folded && p.seat != seat)
                .map(|p| holes[p.seat])
                .collect();
            let is_graded = to_call > 0 && matches!(decision.applied, Action::Fold | Action::Call);
            // Graded decisions take the lab's numbers so both tools always agree.
            let (equity, graded) = match is_graded.then(|| grades.next()).flatten() {
                Some(grade) => (
                    grade.equity,
                    Some(Graded {
                        called: grade.called,
                        required: grade.required,
                        ev_call: grade.equity * (view.pot + to_call) as f64 - to_call as f64,
                        loss: grade.loss,
                    }),
                ),
                None => (
                    equity_vs(holes[seat], &view.board, &opponents, GRADE_SAMPLES),
                    None,
                ),
            };
            decisions.push(DecisionNote {
                index,
                street: view.street,
                pot: view.pot,
                to_call,
                action: decision.applied,
                fallback: decision.error.clone(),
                equity,
                opponents: opponents.len(),
                graded,
                rule: scaffold::suggestion(&view),
            });
        }
        hand.act(decision.seat, decision.applied)?;
    }
    let adjusted = evaluation::allin_adjusted(record)?;
    let mistake_cost = decisions
        .iter()
        .filter_map(|d| d.graded.as_ref())
        .map(|g| g.loss)
        .sum();
    Ok(HandAnalysis {
        seat,
        position: scaffold::position(seats, seat, record.button),
        holes,
        big_blind: record.rules.big_blind,
        net: record.final_stacks[seat] as i64 - record.initial_stacks[seat] as i64,
        adjusted_net: adjusted.map(|stacks| stacks[seat] - record.initial_stacks[seat] as f64),
        reached_showdown: record
            .events
            .iter()
            .any(|e| matches!(e, Event::Reveal { seat: s, .. } if *s == seat)),
        decisions,
        mistake_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use poker_lab::{
        arena::{Agent, Scripted, Session},
        cards::Rng,
    };

    #[test]
    fn analysis_agrees_with_the_lab_grades_and_adjustment() {
        let mut session = Session::new(6, 11).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = (0..6)
            .map(|s| Box::new(Scripted::Random(Rng(40 + s))) as Box<dyn Agent>)
            .collect();
        let (mut graded, mut adjusted) = (0, 0);
        for _ in 0..120 {
            let record = session.next_hand(&mut agents).unwrap().record;
            for seat in [0, 3] {
                let analysis = analyse(&record, seat).unwrap();
                let lab = evaluation::grade_seat(&record, seat).unwrap();
                let mine: Vec<&Graded> = analysis
                    .decisions
                    .iter()
                    .filter_map(|d| d.graded.as_ref())
                    .collect();
                assert_eq!(mine.len(), lab.len());
                for (m, l) in mine.iter().zip(&lab) {
                    assert_eq!(
                        (m.called, m.loss, m.required),
                        (l.called, l.loss, l.required)
                    );
                    assert!(
                        (m.ev_call.max(0.0) - if m.called { m.ev_call } else { 0.0 } - m.loss)
                            .abs()
                            < 1e-9
                    );
                }
                let lab_loss: f64 = lab.iter().map(|g| g.loss).sum();
                assert!((analysis.mistake_cost - lab_loss).abs() < 1e-9);
                assert_eq!(
                    analysis.decisions.len(),
                    record.decisions.iter().filter(|d| d.seat == seat).count()
                );
                assert_eq!(
                    analysis.net,
                    record.final_stacks[seat] as i64 - record.initial_stacks[seat] as i64
                );
                graded += lab.len();
                adjusted += usize::from(analysis.adjusted_net.is_some());
            }
        }
        assert!(
            graded > 30 && adjusted > 5,
            "sample must exercise grades and adjustment"
        );
    }
}
