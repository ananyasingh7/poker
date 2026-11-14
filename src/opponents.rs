//! Version-1 deterministic heuristics, not solver or GTO baselines.
//! Version 2 adds bounded-leak archetypes (station, overfolder), an equity-based `Solid`
//! baseline, and an `Adaptive` policy with a published adjustment rule.
use crate::{
    arena::Agent,
    cards::{Card, Rng, evaluate},
    engine::{Action, Event, LegalActions, PlayerView, Street},
    equity::equity,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Tight,
    Loose,
    Aggressive,
    /// Leak: never folds a pair or ace-high and never bets or raises. Pays off value bets.
    Station,
    /// Leak: folds any hand weaker than two pair to every river bet. Loses to river bluffs.
    Overfolder,
    /// Equity-versus-pot-odds baseline with default `Params`.
    Solid,
    /// `Solid` that changes its bluffing with how often its bets get folded to.
    Adaptive,
}
impl Style {
    /// The participant-protocol policy for this style.
    pub fn policy(self) -> crate::participant::Policy {
        use crate::participant::Policy;
        match self {
            Self::Tight => Policy::Tight,
            Self::Loose => Policy::Loose,
            Self::Aggressive => Policy::Aggressive,
            Self::Station => Policy::Station,
            Self::Overfolder => Policy::Overfolder,
            Self::Solid => Policy::Solid,
            Self::Adaptive => Policy::Adaptive,
        }
    }
}
impl Style {
    /// What an informed observer would write about this policy. Used only for the labelled
    /// oracle-notes condition; never shown to a model otherwise.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Tight => {
                "Tight and straightforward. Enters few pots (strong high cards and pairs). After the flop continues only with a pair or better, raises two pair or better, and folds one pair to bets larger than a quarter of its stack. Its bets and raises mean a real hand. Never bluffs."
            }
            Self::Loose => {
                "Loose and passive. Enters many pots. After the flop continues with any pair or a jack-or-higher hole card, raises only trips or better, and calls bets up to half its stack. Never bluffs. Value-bet it; its raises are very strong."
            }
            Self::Aggressive => {
                "Aggressive. Raises often before the flop and bets or raises any pair after it, for about half the pot, and will commit its whole stack with one pair. Does not bluff with nothing but overvalues weak pairs. Call or raise it with good pairs and better; do not bluff it."
            }
            Self::Station => {
                "Calling station. Never bets or raises. Never folds a pair or an ace-high hand to any bet of any size. Bet your good hands big for value on every street; never bluff it."
            }
            Self::Overfolder => {
                "River over-folder. Plays loosely and calls flop and turn bets with any pair, but on the river folds everything weaker than two pair to any bet. Bluff the river when it has only called; give up when it bets or raises the river."
            }
            Self::Solid => {
                "Solid. Compares its winning chances with the pot odds: value-bets about two thirds of the pot with strong hands, calls when the price is right, folds otherwise, and bluffs occasionally. No large leak; avoid marginal confrontations."
            }
            Self::Adaptive => {
                "Adaptive. Plays solidly but tracks how often its bets get folded to. If opponents fold to most of its bets it starts bluffing very often; if they rarely fold it stops bluffing and value-bets thinner. Its bluffing frequency depends on how the table has responded to it recently."
            }
        }
    }
    /// A fresh agent for this style. Shared by both execution backends.
    pub fn agent(self) -> Box<dyn Agent> {
        match self {
            Self::Solid => Box::new(Solid::new(Params::default())),
            Self::Adaptive => Box::new(Adaptive::default()),
            other => Box::new(Opponent(other)),
        }
    }
}
pub struct Opponent(pub Style);
impl Agent for Opponent {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        let l = v.legal.as_ref().ok_or("not acting")?;
        let a = v.hole[0].rank();
        let b = v.hole[1].rank();
        let hi = a.max(b);
        let lo = a.min(b);
        let score = if a == b {
            30 + a
        } else {
            hi + lo
                + if v.hole[0].suit() == v.hole[1].suit() {
                    3
                } else {
                    0
                }
        };
        let (play, strong) = if v.street == Street::Preflop {
            match self.0 {
                Style::Tight => (score >= 25, score >= 38 || hi == 14 && lo >= 12),
                Style::Loose | Style::Overfolder => (score >= 17, score >= 40),
                Style::Aggressive => (score >= 21, score >= 26),
                Style::Station => (score >= 17, false),
                Style::Solid | Style::Adaptive => {
                    return Err("equity styles use their own agents".into());
                }
            }
        } else {
            let mut cards = v.board.clone();
            cards.extend(v.hole);
            let category = evaluate(&cards)?.0[0];
            match self.0 {
                Style::Tight => (category >= 1, category >= 2),
                Style::Loose => (category >= 1 || hi >= 11, category >= 3),
                Style::Aggressive => (category >= 1 || hi >= 12, category >= 1),
                Style::Station => (category >= 1 || hi == 14, false),
                // The bounded leak: one pair or worse never continues against a river bet.
                Style::Overfolder if v.street == Street::River && l.call.is_some() => {
                    (category >= 2, category >= 3)
                }
                Style::Overfolder => (category >= 1 || hi >= 11, category >= 3),
                Style::Solid | Style::Adaptive => {
                    return Err("equity styles use their own agents".into());
                }
            }
        };
        let own = &v.players[v.seat];
        if strong && let Some(min) = l.min_raise_to {
            let target = if v.street == Street::Preflop {
                6.max(min)
            } else {
                own.street_committed + v.pot / 2
            };
            return Ok(Action::RaiseTo(
                target.max(min).min(l.max_raise_to.unwrap()),
            ));
        }
        if l.check {
            return Ok(Action::Check);
        }
        let affordable = l.call.is_some_and(|n| {
            n <= own.stack
                / match self.0 {
                    Style::Tight => 4,
                    Style::Loose | Style::Overfolder => 2,
                    Style::Aggressive => 3,
                    // A station calls any amount with a hand it likes.
                    Style::Station | Style::Solid | Style::Adaptive => 1,
                }
        });
        if play && (strong || affordable) {
            Ok(Action::Call)
        } else {
            Ok(Action::Fold)
        }
    }
}
/// The original mixed field.
pub const DEFAULT_FIELD: [Style; 5] = [
    Style::Tight,
    Style::Loose,
    Style::Aggressive,
    Style::Tight,
    Style::Loose,
];
/// Fixed opponent identities follow list order with the model inserted at its seat.
pub fn styles(seats: usize, model: usize, offset: usize, field: &[Style]) -> Vec<Option<Style>> {
    let mut j = 0;
    (0..seats)
        .map(|i| {
            if i == model {
                None
            } else {
                let s = field[(j + offset) % field.len()];
                j += 1;
                Some(s)
            }
        })
        .collect()
}

/// Tunable equity-based policy. `Solid` uses the defaults; the exploitation-ceiling search
/// explores this family to estimate what a simple exploiter can win from a field.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Minimum preflop score to open-raise an unraised pot.
    pub open: u8,
    /// Minimum preflop score to call a raise.
    pub continue_pre: u8,
    /// Bet when unadjusted equity is at least this and nobody has bet.
    pub value: f64,
    /// Raise when discounted equity is at least this.
    pub raise: f64,
    /// Call when discounted equity exceeds the pot odds by this margin.
    pub margin: f64,
    /// Subtracted from equity when facing a bet: a bettor's range beats a random hand.
    pub discount: f64,
    /// Chance of betting a weak hand when nobody has bet (decided deterministically per spot).
    pub bluff: f64,
    /// Bet and raise size as a fraction of the pot.
    pub bet: f64,
}
impl Default for Params {
    fn default() -> Self {
        Self {
            open: 25,
            continue_pre: 30,
            value: 0.62,
            raise: 0.85,
            margin: 0.05,
            discount: 0.12,
            bluff: 0.15,
            bet: 0.66,
        }
    }
}
const EQUITY_SAMPLES: u32 = 80;

fn score(hole: [Card; 2]) -> u8 {
    let (a, b) = (hole[0].rank(), hole[1].rank());
    if a == b {
        30 + a
    } else {
        a + b
            + if hole[0].suit() == hole[1].suit() {
                3
            } else {
                0
            }
    }
}
/// A legal full raise nearest to `target`, if any full raise is available.
fn raise_near(legal: &LegalActions, target: u64) -> Option<Action> {
    let (min, max) = (legal.min_raise_to?, legal.max_raise_to?);
    Some(Action::RaiseTo(target.clamp(min, max)))
}
fn passive(legal: &LegalActions) -> Action {
    if legal.check {
        Action::Check
    } else {
        Action::Call
    }
}

pub struct Solid {
    pub params: Params,
    /// Equity for the current street, keyed by board length; cleared every hand.
    cache: Option<(usize, f64)>,
}
impl Solid {
    pub fn new(params: Params) -> Self {
        Self {
            params,
            cache: None,
        }
    }
    fn choose(&mut self, v: &PlayerView) -> Result<Action, String> {
        let l = v.legal.as_ref().ok_or("not acting")?;
        let p = self.params;
        let own = &v.players[v.seat];
        let to_call = l.call.unwrap_or(0);
        if v.street == Street::Preflop {
            let score = score(v.hole);
            let raised = v.history.iter().any(|e| {
                matches!(
                    e,
                    Event::Act {
                        action: Action::RaiseTo(_),
                        ..
                    }
                )
            });
            let wager = own.street_committed + to_call;
            return Ok(if !raised && score >= p.open {
                raise_near(l, 6).unwrap_or(passive(l))
            } else if raised && score >= 41 {
                raise_near(l, wager * 3).unwrap_or(Action::Call)
            } else if l.check {
                Action::Check
            } else if raised && score >= p.continue_pre && to_call <= own.stack / 4 {
                Action::Call
            } else {
                Action::Fold
            });
        }
        let equity = match self.cache {
            Some((street, value)) if street == v.board.len() => value,
            _ => {
                let value = equity(v.hole, &v.board, EQUITY_SAMPLES);
                self.cache = Some((v.board.len(), value));
                value
            }
        };
        let size = |base: u64| (base as f64 * p.bet).round().max(1.0) as u64;
        if to_call == 0 {
            // Deterministic per spot, so a seeded table replays identically.
            let spot = v
                .hole
                .iter()
                .chain(&v.board)
                .fold(v.history.len() as u64, |h, c| {
                    h.wrapping_mul(53).wrapping_add(u64::from(u8::from(*c)))
                });
            let roll = Rng(spot).below(1000) as f64 / 1000.0;
            if equity >= p.value || roll < p.bluff {
                return Ok(
                    raise_near(l, own.street_committed + size(v.pot)).unwrap_or(Action::Check)
                );
            }
            return Ok(Action::Check);
        }
        let adjusted = equity - p.discount;
        let odds = to_call as f64 / (v.pot + to_call) as f64;
        Ok(if adjusted >= p.raise && l.min_raise_to.is_some() {
            let target = own.street_committed + to_call + size(v.pot + to_call);
            raise_near(l, target).unwrap_or(Action::Call)
        } else if adjusted >= odds + p.margin {
            Action::Call
        } else {
            Action::Fold
        })
    }
}
impl Agent for Solid {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        self.choose(v)
    }
    fn start_hand(&mut self, _number: usize) -> Result<(), String> {
        self.cache = None;
        Ok(())
    }
}

/// `Solid` with one published adjustment rule. After each hand it counts opponent responses
/// to its own postflop bets and raises. Once it has at least `ADAPT_AFTER` responses:
/// fold rate above 60% -> bluff 80% of weak hands; below 30% -> never bluff and value-bet
/// thinner (equity 0.55); otherwise the `Solid` defaults.
pub struct Adaptive {
    inner: Solid,
    pub responses: u64,
    pub folds: u64,
}
pub const ADAPT_AFTER: u64 = 8;
impl Default for Adaptive {
    fn default() -> Self {
        Self {
            inner: Solid::new(Params::default()),
            responses: 0,
            folds: 0,
        }
    }
}
impl Adaptive {
    pub fn params(&self) -> Params {
        self.inner.params
    }
}
impl Agent for Adaptive {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        self.inner.choose(v)
    }
    fn start_hand(&mut self, number: usize) -> Result<(), String> {
        self.inner.start_hand(number)
    }
    fn finish_hand(&mut self, v: &PlayerView) -> Result<(), String> {
        let (mut postflop, mut live) = (false, false);
        for event in &v.history {
            match event {
                Event::Board { .. } => (postflop, live) = (true, false),
                Event::Act { seat, action } if postflop && *seat == v.seat => {
                    live = matches!(action, Action::RaiseTo(_));
                }
                Event::Act { action, .. } if live => {
                    self.responses += 1;
                    match action {
                        Action::Fold => self.folds += 1,
                        Action::RaiseTo(_) => live = false,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        let mut params = Params::default();
        if self.responses >= ADAPT_AFTER {
            let rate = self.folds as f64 / self.responses as f64;
            if rate > 0.6 {
                params.bluff = 0.8;
            } else if rate < 0.3 {
                params.bluff = 0.0;
                params.value = 0.55;
            }
        }
        self.inner.params = params;
        Ok(())
    }
}
