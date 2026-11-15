// ABOUTME: Plays a session one hand at a time with the lab's engine, reporting each step so a
// ABOUTME: spectator can follow and pause. Deals, fallbacks and records match `arena::Session` exactly.
use crate::frames::HandStart;
use poker_lab::{
    arena::{Agent, Decision, LifecycleError, OUTAGE, Record},
    cards::Rng,
    engine::{Chips, Rules},
};

/// A moment in a hand, reported before the dealer continues.
pub enum Step<'a> {
    /// Cards are dealt and blinds posted.
    Dealt(&'a HandStart),
    /// `seat` is about to be asked for a decision.
    Turn { seat: usize },
    /// The decisions so far, the last one just applied.
    Acted(&'a [Decision]),
    /// Betting is over; seats are being told the result (a model may update its notes).
    Closing,
}

pub enum Outcome {
    Played {
        record: Record,
        lifecycle_errors: Vec<LifecycleError>,
    },
    /// The provider outlasted every retry mid-hand: the hand is dropped, not played with a fallback.
    Abandoned,
    /// An agent refused to start another hand (budget or repeated faults).
    Stopped(String),
}

/// Fixed seats; busted players rebuy to 100 big blinds between hands, as in `arena::Session`.
pub struct Dealer {
    pub rules: Rules,
    pub stacks: Vec<Chips>,
    pub button: usize,
    pub rebuys: Vec<Chips>,
    /// Every hand starts with 100 big blinds in every seat (the lab's experiment setting).
    pub reset_stacks: bool,
    rng: Rng,
    hands_played: usize,
}
impl Dealer {
    pub fn new(seats: usize, seed: u64) -> Result<Self, String> {
        if !(2..=6).contains(&seats) {
            return Err("seats must be 2..6".into());
        }
        let rules = Rules::default();
        Ok(Self {
            stacks: vec![100 * rules.big_blind; seats],
            rules,
            button: 0,
            rebuys: vec![0; seats],
            reset_stacks: false,
            rng: Rng(seed),
            hands_played: 0,
        })
    }
    pub fn hands_played(&self) -> usize {
        self.hands_played
    }
    pub fn next_hand(
        &mut self,
        agents: &mut [Box<dyn Agent>],
        on: &mut dyn FnMut(Step),
    ) -> Result<Outcome, String> {
        if agents.len() != self.stacks.len() {
            return Err("one agent per seat required".into());
        }
        let mut lifecycle_errors = vec![];
        for (seat, agent) in agents.iter_mut().enumerate() {
            if let Some(reason) = agent.stop_reason() {
                return Ok(Outcome::Stopped(reason.to_owned()));
            }
            if let Err(error) = agent.start_hand(self.hands_played + 1) {
                lifecycle_errors.push(LifecycleError {
                    seat,
                    phase: "start_hand".into(),
                    error,
                });
            }
        }
        for (seat, stack) in self.stacks.iter_mut().enumerate() {
            if self.reset_stacks {
                *stack = 100 * self.rules.big_blind;
            } else if *stack == 0 {
                *stack = 100 * self.rules.big_blind;
                self.rebuys[seat] += *stack;
            }
        }
        let start = HandStart {
            rules: self.rules.clone(),
            stacks: self.stacks.clone(),
            button: self.button,
            seed: self.rng.next_u64(),
        };
        let mut hand = start.hand()?;
        on(Step::Dealt(&start));
        let mut decisions: Vec<Decision> = vec![];
        while let Some(seat) = hand.actor() {
            on(Step::Turn { seat });
            let view = hand.view(seat)?;
            let proposed = agents[seat].decide(&view);
            if proposed.as_ref().is_err_and(|e| e == OUTAGE) {
                return Ok(Outcome::Abandoned);
            }
            let legal = hand.legal();
            let (applied, error) = match &proposed {
                Ok(action) if legal.accepts(*action) => (*action, None),
                Ok(_) => (legal.fallback(), Some("illegal agent action".into())),
                Err(e) => (legal.fallback(), Some(e.clone())),
            };
            hand.act(seat, applied)?;
            decisions.push(Decision {
                seat,
                proposed,
                applied,
                error,
            });
            on(Step::Acted(&decisions));
        }
        on(Step::Closing);
        // Hand-end callbacks run sequentially by seat so shared budget accounting is deterministic.
        for (seat, agent) in agents.iter_mut().enumerate() {
            if let Err(error) = agent.finish_hand(&hand.view(seat)?) {
                lifecycle_errors.push(LifecycleError {
                    seat,
                    phase: "hand_end".into(),
                    error,
                });
            }
        }
        let record = Record {
            version: 1,
            rules: start.rules,
            seed: start.seed,
            button: start.button,
            initial_stacks: start.stacks,
            decisions,
            events: hand.events().to_vec(),
            final_stacks: hand.stacks(),
        };
        self.stacks = record.final_stacks.clone();
        self.button = (self.button + 1) % self.stacks.len();
        self.hands_played += 1;
        Ok(Outcome::Played {
            record,
            lifecycle_errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poker_lab::{
        arena::{Scripted, Session, replay},
        opponents::Style,
    };

    fn field(seed: u64) -> Vec<Box<dyn Agent>> {
        vec![
            Box::new(Scripted::Random(Rng(seed))),
            Style::Station.agent(),
            Style::Overfolder.agent(),
            Style::Solid.agent(),
            Style::Adaptive.agent(),
            Style::Tight.agent(),
        ]
    }

    /// The spectator table must deal and score exactly what the lab's own session does.
    #[test]
    fn dealer_records_equal_the_lab_session() {
        for reset in [false, true] {
            let mut session = Session::new(6, 42).unwrap();
            session.reset_stacks = reset;
            let mut dealer = Dealer::new(6, 42).unwrap();
            dealer.reset_stacks = reset;
            let (mut lab_agents, mut table_agents) = (field(5), field(5));
            let mut steps = 0;
            for _ in 0..150 {
                let expected = session.next_hand(&mut lab_agents).unwrap().record;
                let Outcome::Played { record, .. } = dealer
                    .next_hand(&mut table_agents, &mut |_| steps += 1)
                    .unwrap()
                else {
                    panic!("scripted hands always complete");
                };
                assert_eq!(record, expected);
                replay(&record).unwrap();
            }
            assert_eq!(dealer.stacks, session.stacks);
            assert_eq!(dealer.rebuys, session.rebuys);
            assert!(steps > 150 * 3);
        }
    }

    struct Down;
    impl Agent for Down {
        fn decide(
            &mut self,
            _: &poker_lab::engine::PlayerView,
        ) -> Result<poker_lab::engine::Action, String> {
            Err(OUTAGE.into())
        }
    }

    #[test]
    fn an_outage_abandons_the_hand_without_moving_the_button() {
        let mut dealer = Dealer::new(2, 1).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = vec![Box::new(Down), Box::new(Down)];
        assert!(matches!(
            dealer.next_hand(&mut agents, &mut |_| {}).unwrap(),
            Outcome::Abandoned
        ));
        assert_eq!((dealer.button, dealer.hands_played()), (0, 0));
    }
}
