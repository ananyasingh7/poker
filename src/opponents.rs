//! Version-1 deterministic heuristics, not solver or GTO baselines.
//! Version 2 adds bounded-leak archetypes (station, overfolder), an equity-based `Solid`
//! baseline, and an `Adaptive` policy with a published adjustment rule.
//! Opt-in `Variation` (2026-11-18) makes the field less predictable: per-session personalities,
//! per-decision mixing and a randomly drawn field, all derived from the session seed so every
//! hand still replays and the range-based graders stay exact.
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

/// Opt-in unpredictability for the scripted field. All of it is drawn from the session seed, so
/// two arms on the same seed meet the same bots (pairing holds) and every hand replays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Variation {
    /// Draw each bot's thresholds per session from a range around its defaults (its defining
    /// leak is kept: the station never folds a pair, the overfolder folds rivers).
    pub personalities: bool,
    /// Chance per decision (0 to 0.5) that a bot deviates from its policy: bets instead of
    /// checking, calls instead of folding, raises instead of calling, checks instead of betting.
    pub mixing: f64,
    /// Draw the field per seed (with replacement) from `opponent_field` (all seven styles when
    /// that is empty) instead of seating it in order.
    pub random_field: bool,
}
impl Variation {
    pub fn is_active(&self) -> bool {
        self.personalities || self.mixing > 0.0 || self.random_field
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=0.5).contains(&self.mixing) {
            return Err("opponent_variation.mixing must be between 0 and 0.5".into());
        }
        Ok(())
    }
}

/// This session's personality of one heuristic or equity bot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tweak {
    /// Added to every preflop score threshold of the heuristic styles (negative = looser).
    pub preflop: i8,
    /// Multiplies the share of its stack a heuristic bot calls with a non-strong hand.
    pub call_share: f64,
    /// Equity parameters for `solid` and the base of `adaptive`.
    pub params: Params,
}
impl Default for Tweak {
    fn default() -> Self {
        Self {
            preflop: 0,
            call_share: 1.0,
            params: Params::default(),
        }
    }
}
impl Tweak {
    /// A personality drawn from `rng`: preflop thresholds within 4 points, call share 0.5 to
    /// 1.5x, and for equity bots value 0.55-0.70, margin 0-0.10, bluff 0-0.35, bet 0.45-1.0 pot,
    /// open 22-28.
    pub fn draw(rng: &mut Rng) -> Self {
        let mut unit = || rng.below(10_001) as f64 / 10_000.0;
        let preflop = (unit() * 8.0).round() as i8 - 4;
        let call_share = 0.5 + unit();
        let params = Params {
            open: 22 + (unit() * 6.0).round() as u8,
            value: 0.55 + unit() * 0.15,
            margin: unit() * 0.10,
            bluff: unit() * 0.35,
            bet: 0.45 + unit() * 0.55,
            ..Params::default()
        };
        Self {
            preflop,
            call_share,
            params,
        }
    }
}

/// One scripted seat as built for a session: its style, this session's personality, and its
/// mixing rate. `SeatBot::plain` is the original fixed bot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeatBot {
    pub style: Style,
    pub tweak: Tweak,
    pub mixing: f64,
    /// Separates this seat's deviation rolls from the other seats'.
    pub salt: u64,
}
impl SeatBot {
    pub fn plain(style: Style) -> Self {
        Self {
            style,
            tweak: Tweak::default(),
            mixing: 0.0,
            salt: 0,
        }
    }
    /// A fresh agent for this seat. Plain bots are exactly `Style::agent`.
    pub fn agent(&self) -> Box<dyn Agent> {
        let inner: Box<dyn Agent> = match self.style {
            Style::Solid => Box::new(Solid::new(self.tweak.params)),
            Style::Adaptive => Box::new(Adaptive::with_base(self.tweak.params)),
            style if self.tweak == Tweak::default() => Box::new(Opponent(style)),
            style => Box::new(TunedOpponent {
                style,
                tweak: self.tweak,
            }),
        };
        self.mix(inner)
    }
    /// `inner` with this seat's mixing, if any.
    pub fn mix(&self, inner: Box<dyn Agent>) -> Box<dyn Agent> {
        if self.mixing > 0.0 {
            Box::new(Mixed {
                inner,
                rate: self.mixing,
                salt: self.salt,
            })
        } else {
            inner
        }
    }
}

/// The field for one session: every non-model seat's bot, drawn from `seed` as `variation`
/// says. Without variation this is `styles(..)` with plain bots.
pub fn seat_bots(
    seats: usize,
    model: usize,
    offset: usize,
    field: &[Style],
    variation: &Variation,
    seed: u64,
) -> Vec<Option<SeatBot>> {
    const ALL: [Style; 7] = [
        Style::Tight,
        Style::Loose,
        Style::Aggressive,
        Style::Station,
        Style::Overfolder,
        Style::Solid,
        Style::Adaptive,
    ];
    let mut rng = Rng(seed ^ 0x5eed_b075);
    let drawn: Vec<Style>;
    let field = if variation.random_field {
        let pool = if field.is_empty() { &ALL[..] } else { field };
        drawn = (0..seats.saturating_sub(1))
            .map(|_| pool[rng.below(pool.len() as u64) as usize])
            .collect();
        &drawn[..]
    } else if field.is_empty() {
        &DEFAULT_FIELD
    } else {
        field
    };
    let mut j = 0;
    (0..seats)
        .map(|seat| {
            if seat == model {
                return None;
            }
            // Keyed by field slot, so a bot keeps its personality when the model changes seats.
            let slot = (j + offset) % field.len();
            j += 1;
            let style = field[slot];
            let mut own = Rng(seed ^ (slot as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            Some(SeatBot {
                style,
                tweak: if variation.personalities {
                    Tweak::draw(&mut own)
                } else {
                    Tweak::default()
                },
                mixing: variation.mixing,
                // Only mixing uses the salt; plain bots keep 0 so they equal `SeatBot::plain`.
                salt: if variation.mixing > 0.0 {
                    own.next_u64()
                } else {
                    0
                },
            })
        })
        .collect()
}

/// A bot that sometimes deviates from its policy. The roll is a hash of the seat's salt and the
/// spot it sees (its cards, the board, how far the hand has gone), so it replays exactly and a
/// range grader that replays the policy with other hole cards sees the same rule.
pub struct Mixed {
    pub inner: Box<dyn Agent>,
    pub rate: f64,
    pub salt: u64,
}
impl Agent for Mixed {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        let action = self.inner.decide(v)?;
        let l = v.legal.as_ref().ok_or("not acting")?;
        // Hole cards in a fixed order: the same two cards must roll the same however they were
        // dealt, or a range grader that lists hands in card order would disagree.
        let mut hole = v.hole;
        hole.sort_by_key(|c| u8::from(*c));
        let spot = hole
            .iter()
            .chain(&v.board)
            .fold(self.salt ^ v.history.len() as u64, |h, c| {
                h.wrapping_mul(0x100_0000_01b3)
                    .wrapping_add(u64::from(u8::from(*c)))
            });
        if (Rng(spot).below(10_000) as f64) >= self.rate * 10_000.0 {
            return Ok(action);
        }
        let own = &v.players[v.seat];
        let call = l.call.unwrap_or(0);
        let deviation = match action {
            Action::Check => raise_near(l, own.street_committed + (v.pot / 2).max(1)),
            Action::Fold => Some(Action::Call),
            Action::Call => raise_near(l, own.street_committed + call + v.pot.max(1)),
            Action::RaiseTo(_) if l.check => Some(Action::Check),
            Action::RaiseTo(_) => Some(Action::Call),
        };
        Ok(deviation.filter(|a| l.accepts(*a)).unwrap_or(action))
    }
    fn start_hand(&mut self, number: usize) -> Result<(), String> {
        self.inner.start_hand(number)
    }
    fn finish_hand(&mut self, v: &PlayerView) -> Result<(), String> {
        self.inner.finish_hand(v)
    }
}
pub struct Opponent(pub Style);
impl Agent for Opponent {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        decide_tuned(self.0, &Tweak::default(), v)
    }
}
/// A heuristic style with this session's personality (`Tweak`).
pub struct TunedOpponent {
    pub style: Style,
    pub tweak: Tweak,
}
impl Agent for TunedOpponent {
    fn decide(&mut self, v: &PlayerView) -> Result<Action, String> {
        decide_tuned(self.style, &self.tweak, v)
    }
}
fn decide_tuned(style: Style, tweak: &Tweak, v: &PlayerView) -> Result<Action, String> {
    {
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
        // A personality moves every preflop threshold together (negative is looser).
        let at = |threshold: u8| score >= (threshold as i16 + tweak.preflop as i16).max(0) as u8;
        let (play, strong) = if v.street == Street::Preflop {
            match style {
                Style::Tight => (at(25), at(38) || hi == 14 && lo >= 12),
                Style::Loose | Style::Overfolder => (at(17), at(40)),
                Style::Aggressive => (at(21), at(26)),
                Style::Station => (at(17), false),
                Style::Solid | Style::Adaptive => {
                    return Err("equity styles use their own agents".into());
                }
            }
        } else {
            let mut cards = v.board.clone();
            cards.extend(v.hole);
            let category = evaluate(&cards)?.0[0];
            match style {
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
        let share = match style {
            Style::Tight => 0.25,
            Style::Loose | Style::Overfolder => 0.5,
            Style::Aggressive => 1.0 / 3.0,
            // A station calls any amount with a hand it likes: its defining leak, never varied.
            Style::Station | Style::Solid | Style::Adaptive => 1.0,
        } * if style == Style::Station {
            1.0
        } else {
            tweak.call_share
        };
        // Integer division as before when the share is unchanged, so default play is identical.
        let limit = if tweak.call_share == 1.0 || style == Style::Station {
            own.stack
                / match style {
                    Style::Tight => 4,
                    Style::Loose | Style::Overfolder => 2,
                    Style::Aggressive => 3,
                    Style::Station | Style::Solid | Style::Adaptive => 1,
                }
        } else {
            (own.stack as f64 * share) as u64
        };
        let affordable = l.call.is_some_and(|n| n <= limit);
        if play && (strong || affordable) {
            Ok(Action::Call)
        } else {
            Ok(Action::Fold)
        }
    }
}
/// The field every experiment since 2026-11-13 uses (`poker-lab ceiling current`).
pub const CURRENT_FIELD: [Style; 5] = [
    Style::Station,
    Style::Overfolder,
    Style::Solid,
    Style::Adaptive,
    Style::Tight,
];
/// The original mixed field (the fallback when a config names no field).
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
    /// Its parameters before any adjustment (the defaults, or this session's personality).
    base: Params,
    pub responses: u64,
    pub folds: u64,
}
pub const ADAPT_AFTER: u64 = 8;
impl Default for Adaptive {
    fn default() -> Self {
        Self::with_base(Params::default())
    }
}
impl Adaptive {
    pub fn with_base(base: Params) -> Self {
        Self {
            inner: Solid::new(base),
            base,
            responses: 0,
            folds: 0,
        }
    }
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
        let mut params = self.base;
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

#[cfg(test)]
mod variation_tests {
    use super::*;
    use crate::arena::{Record, Session};

    fn play(bots: &[Option<SeatBot>], hands: usize, seed: u64) -> Vec<Record> {
        let mut session = Session::new(bots.len(), seed).unwrap();
        session.reset_stacks = true;
        let mut agents: Vec<Box<dyn Agent>> = bots
            .iter()
            .map(|b| b.map_or_else(|| Style::Solid.agent(), |b| b.agent()))
            .collect();
        (0..hands)
            .map(|_| session.next_hand(&mut agents).unwrap().record)
            .collect()
    }

    const FIELD: [Style; 5] = [
        Style::Station,
        Style::Overfolder,
        Style::Solid,
        Style::Adaptive,
        Style::Tight,
    ];

    #[test]
    fn no_variation_is_the_original_field() {
        let bots = seat_bots(6, 0, 2, &FIELD, &Variation::default(), 9);
        let plain: Vec<Option<SeatBot>> = styles(6, 0, 2, &FIELD)
            .into_iter()
            .map(|s| s.map(SeatBot::plain))
            .collect();
        assert_eq!(bots, plain);
        // And plays exactly like `Style::agent`.
        let mut session = Session::new(6, 3).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = vec![Style::Solid.agent()];
        agents.extend(FIELD.iter().map(|s| s.agent()));
        let original: Vec<Record> = (0..80)
            .map(|_| session.next_hand(&mut agents).unwrap().record)
            .collect();
        let mut session = Session::new(6, 3).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = vec![Style::Solid.agent()];
        agents.extend(FIELD.iter().map(|s| SeatBot::plain(*s).agent()));
        let rebuilt: Vec<Record> = (0..80)
            .map(|_| session.next_hand(&mut agents).unwrap().record)
            .collect();
        assert_eq!(original, rebuilt);
    }

    #[test]
    fn variation_is_drawn_from_the_seed_and_replays() {
        let v = Variation {
            personalities: true,
            mixing: 0.15,
            random_field: true,
        };
        let a = seat_bots(6, 0, 0, &[], &v, 11);
        assert_eq!(a, seat_bots(6, 0, 0, &[], &v, 11), "same seed, same bots");
        assert_ne!(
            a,
            seat_bots(6, 0, 0, &[], &v, 12),
            "another seed, other bots"
        );
        // Personalities stay inside their ranges.
        for seed in 0..200 {
            for bot in seat_bots(6, 0, 0, &[], &v, seed).into_iter().flatten() {
                let t = bot.tweak;
                assert!((-4..=4).contains(&t.preflop));
                assert!((0.5..=1.5).contains(&t.call_share));
                assert!((0.0..=0.35).contains(&t.params.bluff));
                assert!((0.45..=1.0).contains(&t.params.bet));
            }
        }
        // A bot keeps its personality when the model moves to another seat.
        let at0 = seat_bots(6, 0, 0, &FIELD, &v, 5);
        let at3 = seat_bots(6, 3, 0, &FIELD, &v, 5);
        let mut first: Vec<SeatBot> = at0.into_iter().flatten().collect();
        let mut second: Vec<SeatBot> = at3.into_iter().flatten().collect();
        first.sort_by_key(|b| b.salt);
        second.sort_by_key(|b| b.salt);
        assert_eq!(first, second);
        // Two plays of the same seed are identical hand for hand.
        assert_eq!(play(&a, 60, 11), play(&a, 60, 11));
    }

    #[test]
    fn mixing_deviates_at_about_its_rate_and_never_illegally() {
        let plain = seat_bots(6, 0, 0, &FIELD, &Variation::default(), 4);
        let mixed = seat_bots(
            6,
            0,
            0,
            &FIELD,
            &Variation {
                mixing: 0.2,
                ..Variation::default()
            },
            4,
        );
        let (a, b) = (play(&plain, 150, 4), play(&mixed, 150, 4));
        // Every applied action was the policy's own (no fallbacks), and play differs.
        assert!(
            b.iter()
                .flat_map(|r| &r.decisions)
                .all(|d| d.error.is_none())
        );
        assert_ne!(a, b);
        // A mixing rate of zero is the inner policy exactly.
        let mut zero = Mixed {
            inner: Style::Tight.agent(),
            rate: 0.0,
            salt: 1,
        };
        for record in &a {
            let mut hand = crate::engine::Hand::new(
                record.rules.clone(),
                record.initial_stacks.clone(),
                record.button,
                record.seed,
            )
            .unwrap();
            for d in &record.decisions {
                let view = hand.view(d.seat).unwrap();
                assert_eq!(
                    zero.decide(&view).unwrap(),
                    Style::Tight.agent().decide(&view).unwrap()
                );
                hand.act(d.seat, d.applied).unwrap();
            }
        }
    }
}
