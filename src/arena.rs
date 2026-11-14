use crate::{cards::Rng, engine::*};
use serde::{Deserialize, Serialize};

/// The error an agent returns when infrastructure alone prevented a decision (retries exhausted).
/// `play` abandons the hand instead of recording a fallback action for it.
pub const OUTAGE: &str = "openrouter:Outage";

pub trait Agent {
    fn decide(&mut self, view: &PlayerView) -> Result<Action, String>;
    /// Lifecycle callbacks report failures explicitly (e.g. a remote notebook update or a
    /// dead participant). They never change engine state or the replay record.
    fn start_hand(&mut self, _number: usize) -> Result<(), String> {
        Ok(())
    }
    fn finish_hand(&mut self, _view: &PlayerView) -> Result<(), String> {
        Ok(())
    }
    fn stop_reason(&self) -> Option<&str> {
        None
    }
}
pub enum Scripted {
    Caller,
    Folder,
    Raiser,
    Random(Rng),
}
impl Agent for Scripted {
    fn decide(&mut self, view: &PlayerView) -> Result<Action, String> {
        let legal = view.legal.as_ref().ok_or("no decision available")?;
        let passive = if legal.check {
            Action::Check
        } else {
            Action::Call
        };
        let raise = legal
            .min_raise_to
            .or(legal.short_all_in_to)
            .map(Action::RaiseTo);
        Ok(match self {
            Self::Caller => passive,
            Self::Folder => legal.fallback(),
            Self::Raiser => raise.unwrap_or(passive),
            Self::Random(rng) => match rng.below(5) {
                0 => legal.fallback(),
                1 => raise.unwrap_or(passive),
                2 => legal
                    .max_raise_to
                    .or(legal.short_all_in_to)
                    .map(Action::RaiseTo)
                    .unwrap_or(passive),
                _ => passive,
            },
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub seat: usize,
    pub proposed: Result<Action, String>,
    pub applied: Action,
    pub error: Option<String>,
}
/// Evaluator-only replay record. Never supplied to agents or public logs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub version: u32,
    pub rules: Rules,
    pub seed: u64,
    pub button: usize,
    pub initial_stacks: Vec<Chips>,
    pub decisions: Vec<Decision>,
    pub events: Vec<Event>,
    pub final_stacks: Vec<Chips>,
}
/// A failed lifecycle callback. Kept out of `Record` so engine replay stays version 1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleError {
    pub seat: usize,
    pub phase: String,
    pub error: String,
}
pub struct PlayedHand {
    pub record: Record,
    pub private_views: Vec<Vec<PlayerView>>,
    pub lifecycle_errors: Vec<LifecycleError>,
}
pub fn play(
    rules: Rules,
    stacks: Vec<Chips>,
    button: usize,
    seed: u64,
    agents: &mut [Box<dyn Agent>],
) -> Result<PlayedHand, String> {
    if agents.len() != stacks.len() {
        return Err("one agent per seat required".into());
    }
    let mut hand = Hand::new(rules.clone(), stacks.clone(), button, seed)?;
    let mut views = vec![vec![]; agents.len()];
    let mut decisions = vec![];
    while let Some(seat) = hand.actor() {
        let view = hand.view(seat)?;
        let proposed = agents[seat].decide(&view);
        if proposed.as_ref().is_err_and(|e| e == OUTAGE) {
            return Err(OUTAGE.into());
        }
        let legal = hand.legal();
        let (applied, error) = match &proposed {
            Ok(action) if legal.accepts(*action) => (*action, None),
            Ok(_) => (legal.fallback(), Some("illegal agent action".into())),
            Err(e) => (legal.fallback(), Some(e.clone())),
        };
        views[seat].push(view);
        hand.act(seat, applied)?;
        decisions.push(Decision {
            seat,
            proposed,
            applied,
            error,
        });
    }
    // Hand-end callbacks run sequentially by seat so shared budget accounting is deterministic.
    let mut lifecycle_errors = vec![];
    for (seat, history) in views.iter_mut().enumerate() {
        history.push(hand.view(seat)?);
        if let Err(error) = agents[seat].finish_hand(history.last().unwrap()) {
            lifecycle_errors.push(LifecycleError {
                seat,
                phase: "hand_end".into(),
                error,
            });
        }
    }
    Ok(PlayedHand {
        record: Record {
            version: 1,
            rules,
            seed,
            button,
            initial_stacks: stacks,
            decisions,
            events: hand.events().to_vec(),
            final_stacks: hand.stacks(),
        },
        private_views: views,
        lifecycle_errors,
    })
}
pub fn replay(record: &Record) -> Result<(), String> {
    if record.version != 1 {
        return Err("unsupported replay version".into());
    }
    let mut hand = Hand::new(
        record.rules.clone(),
        record.initial_stacks.clone(),
        record.button,
        record.seed,
    )?;
    for d in &record.decisions {
        let legal = hand.legal();
        let expected = match &d.proposed {
            Ok(a) if legal.accepts(*a) => *a,
            _ => legal.fallback(),
        };
        if expected != d.applied {
            return Err("fallback/action mismatch".into());
        }
        hand.act(d.seat, d.applied)?;
    }
    if !hand.is_complete() || hand.events() != record.events || hand.stacks() != record.final_stacks
    {
        return Err("replay mismatch".into());
    }
    Ok(())
}
/// Fixed seats; busted players rebuy to 100 BB between hands. No mid-hand top-ups.
pub struct Session {
    pub rules: Rules,
    pub stacks: Vec<Chips>,
    pub button: usize,
    pub rebuys: Vec<Chips>,
    /// When set, every hand starts with 100 BB in every seat and nothing counts as a rebuy.
    pub reset_stacks: bool,
    rng: Rng,
    hands_played: usize,
}
impl Session {
    pub fn new(seats: usize, seed: u64) -> Result<Self, String> {
        if !(2..=6).contains(&seats) {
            return Err("seats must be 2..6".into());
        }
        Ok(Self {
            rules: Rules::default(),
            stacks: vec![200; seats],
            button: 0,
            rebuys: vec![0; seats],
            reset_stacks: false,
            rng: Rng(seed),
            hands_played: 0,
        })
    }
    pub fn next_hand(&mut self, agents: &mut [Box<dyn Agent>]) -> Result<PlayedHand, String> {
        let mut start_errors = vec![];
        for (seat, agent) in agents.iter_mut().enumerate() {
            if let Some(reason) = agent.stop_reason() {
                return Err(reason.to_owned());
            }
            if let Err(error) = agent.start_hand(self.hands_played + 1) {
                start_errors.push(LifecycleError {
                    seat,
                    phase: "start_hand".into(),
                    error,
                });
            }
        }
        for (i, stack) in self.stacks.iter_mut().enumerate() {
            if self.reset_stacks {
                *stack = 100 * self.rules.big_blind;
            } else if *stack == 0 {
                *stack = 100 * self.rules.big_blind;
                self.rebuys[i] += *stack;
            }
        }
        let mut played = play(
            self.rules.clone(),
            self.stacks.clone(),
            self.button,
            self.rng.next_u64(),
            agents,
        )?;
        start_errors.append(&mut played.lifecycle_errors);
        played.lifecycle_errors = start_errors;
        self.stacks = played.record.final_stacks.clone();
        self.button = (self.button + 1) % self.stacks.len();
        self.hands_played += 1;
        Ok(played)
    }
}
