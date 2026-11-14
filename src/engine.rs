use crate::cards::{Card, deck, evaluate};
use serde::{Deserialize, Serialize};
pub type Chips = u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub small_blind: Chips,
    pub big_blind: Chips,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            small_blind: 1,
            big_blind: 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "amount",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Action {
    Fold,
    Check,
    Call,
    RaiseTo(Chips),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalActions {
    pub fold: bool,
    pub check: bool,
    pub call: Option<Chips>,
    pub min_raise_to: Option<Chips>,
    pub max_raise_to: Option<Chips>,
    pub short_all_in_to: Option<Chips>,
}
impl LegalActions {
    pub fn accepts(&self, a: Action) -> bool {
        match a {
            Action::Fold => self.fold,
            Action::Check => self.check,
            Action::Call => self.call.is_some(),
            Action::RaiseTo(n) => {
                self.short_all_in_to == Some(n)
                    || self
                        .min_raise_to
                        .zip(self.max_raise_to)
                        .is_some_and(|(lo, hi)| (lo..=hi).contains(&n))
            }
        }
    }
    pub fn fallback(&self) -> Action {
        if self.check {
            Action::Check
        } else {
            Action::Fold
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicPlayer {
    pub seat: usize,
    pub stack: Chips,
    pub committed: Chips,
    pub street_committed: Chips,
    pub folded: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Blind {
        seat: usize,
        amount: Chips,
    },
    Act {
        seat: usize,
        action: Action,
    },
    Board {
        street: Street,
        cards: Vec<Card>,
    },
    Reveal {
        seat: usize,
        cards: [Card; 2],
    },
    Refund {
        seat: usize,
        amount: Chips,
    },
    Award {
        amount: Chips,
        eligible: Vec<usize>,
        winners: Vec<usize>,
        shares: Vec<Chips>,
    },
}
/// The only engine object supplied to an agent. No seed or opponent hole cards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerView {
    pub protocol: u32,
    pub seat: usize,
    pub button: usize,
    pub street: Street,
    pub hole: [Card; 2],
    pub board: Vec<Card>,
    pub pot: Chips,
    pub players: Vec<PublicPlayer>,
    pub history: Vec<Event>,
    pub legal: Option<LegalActions>,
}
#[derive(Clone)]
struct Player {
    public: PublicPlayer,
    hole: [Card; 2],
    acted_at: Option<Chips>,
}
/// Deliberately not serializable or Debug: privileged state stays behind the referee boundary.
pub struct Hand {
    rules: Rules,
    players: Vec<Player>,
    button: usize,
    deck: Vec<Card>,
    board: Vec<Card>,
    street: Street,
    actor: Option<usize>,
    current: Chips,
    last_raise: Chips,
    events: Vec<Event>,
    initial_total: Chips,
}
impl Hand {
    pub fn new(rules: Rules, stacks: Vec<Chips>, button: usize, seed: u64) -> Result<Self, String> {
        if !(2..=6).contains(&stacks.len()) || button >= stacks.len() || stacks.contains(&0) {
            return Err("require 2..6 funded seats and a valid button".into());
        }
        if rules.small_blind == 0
            || rules.small_blind > rules.big_blind
            || rules.big_blind > u64::MAX / 100
        {
            return Err("invalid blinds".into());
        }
        let total = stacks
            .iter()
            .try_fold(0u64, |s, x| s.checked_add(*x))
            .ok_or("stack overflow")?;
        // Reserve room for minimum raise arithmetic even at extreme configurations.
        if total > u64::MAX / 2 {
            return Err("table chips exceed supported maximum".into());
        }
        let mut deck = deck(seed);
        let n = stacks.len();
        let dummy = Card::try_from(0).unwrap();
        let mut holes = vec![[dummy; 2]; n];
        for round in [0, 1] {
            for offset in 1..=n {
                holes[(button + offset) % n][round] = deck.pop().unwrap();
            }
        }
        let players = stacks
            .into_iter()
            .enumerate()
            .map(|(seat, stack)| Player {
                public: PublicPlayer {
                    seat,
                    stack,
                    committed: 0,
                    street_committed: 0,
                    folded: false,
                },
                hole: holes[seat],
                acted_at: None,
            })
            .collect();
        let mut h = Self {
            last_raise: rules.big_blind,
            current: rules.big_blind,
            rules,
            players,
            button,
            deck,
            board: vec![],
            street: Street::Preflop,
            actor: None,
            events: vec![],
            initial_total: total,
        };
        let sb = if n == 2 { button } else { (button + 1) % n };
        let bb = (sb + 1) % n;
        for (seat, amount) in [(sb, h.rules.small_blind), (bb, h.rules.big_blind)] {
            let amount = amount.min(h.players[seat].public.stack);
            h.pay(seat, amount);
            h.events.push(Event::Blind { seat, amount });
        }
        h.advance(bb);
        Ok(h)
    }
    pub fn actor(&self) -> Option<usize> {
        self.actor
    }
    pub fn is_complete(&self) -> bool {
        self.street == Street::Complete
    }
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn stacks(&self) -> Vec<Chips> {
        self.players.iter().map(|p| p.public.stack).collect()
    }
    pub fn pot(&self) -> Chips {
        self.players.iter().map(|p| p.public.committed).sum()
    }
    pub fn conserved(&self) -> bool {
        self.stacks().iter().sum::<Chips>() + self.pot() == self.initial_total
    }
    pub fn view(&self, seat: usize) -> Result<PlayerView, String> {
        let player = self.players.get(seat).ok_or("invalid seat")?;
        Ok(PlayerView {
            protocol: 1,
            seat,
            button: self.button,
            street: self.street,
            hole: player.hole,
            board: self.board.clone(),
            pot: self.pot(),
            players: self.players.iter().map(|p| p.public.clone()).collect(),
            history: self.events.clone(),
            legal: if self.actor == Some(seat) {
                Some(self.legal())
            } else {
                None
            },
        })
    }
    pub fn legal(&self) -> LegalActions {
        let mut legal = LegalActions {
            fold: false,
            check: false,
            call: None,
            min_raise_to: None,
            max_raise_to: None,
            short_all_in_to: None,
        };
        let Some(i) = self.actor else { return legal };
        let p = &self.players[i];
        let owed = self.current.saturating_sub(p.public.street_committed);
        legal.fold = true;
        legal.check = owed == 0;
        if owed > 0 {
            legal.call = Some(owed.min(p.public.stack));
        }
        let reopened = p
            .acted_at
            .is_none_or(|at| self.current.saturating_sub(at) >= self.last_raise);
        let opponent_can_bet = self
            .players
            .iter()
            .enumerate()
            .any(|(j, p)| j != i && !p.public.folded && p.public.stack > 0);
        let max = p.public.street_committed + p.public.stack;
        let min = self.current + self.last_raise;
        if reopened && opponent_can_bet && max > self.current {
            if max >= min {
                legal.min_raise_to = Some(min);
                legal.max_raise_to = Some(max);
            } else {
                legal.short_all_in_to = Some(max);
            }
        }
        legal
    }
    pub fn act(&mut self, seat: usize, action: Action) -> Result<(), String> {
        if self.actor != Some(seat) {
            return Err("not this seat's turn".into());
        }
        if !self.legal().accepts(action) {
            return Err("illegal action or amount".into());
        }
        match action {
            Action::Fold => self.players[seat].public.folded = true,
            Action::Check => {}
            Action::Call => {
                let amount = (self.current - self.players[seat].public.street_committed)
                    .min(self.players[seat].public.stack);
                self.pay(seat, amount);
            }
            Action::RaiseTo(to) => {
                let increase = to - self.current;
                if increase >= self.last_raise {
                    self.last_raise = increase;
                }
                self.pay(seat, to - self.players[seat].public.street_committed);
                self.current = to;
            }
        }
        self.players[seat].acted_at = Some(self.current);
        self.events.push(Event::Act { seat, action });
        self.advance(seat);
        debug_assert!(self.conserved());
        Ok(())
    }
    fn pay(&mut self, seat: usize, amount: Chips) {
        let p = &mut self.players[seat].public;
        p.stack -= amount;
        p.committed += amount;
        p.street_committed += amount;
    }
    fn advance(&mut self, after: usize) {
        if self.players.iter().filter(|p| !p.public.folded).count() == 1 {
            self.settle();
            return;
        }
        let capable = self
            .players
            .iter()
            .filter(|p| !p.public.folded && p.public.stack > 0)
            .count();
        if capable <= 1 {
            // A short blind cannot create a phantom call when everyone else is all-in.
            self.current = self
                .players
                .iter()
                .map(|p| p.public.street_committed)
                .max()
                .unwrap();
        }
        self.actor = (1..=self.players.len())
            .map(|o| (after + o) % self.players.len())
            .find(|&i| {
                let p = &self.players[i];
                !p.public.folded
                    && p.public.stack > 0
                    && (p.public.street_committed < self.current
                        || (capable > 1 && p.acted_at.is_none()))
            });
        if self.actor.is_some() {
            return;
        }
        if self.street == Street::River {
            self.settle();
            return;
        }
        self.street = match self.street {
            Street::Preflop => Street::Flop,
            Street::Flop => Street::Turn,
            _ => Street::River,
        };
        self.deck.pop(); // burn
        for _ in 0..if self.street == Street::Flop { 3 } else { 1 } {
            self.board.push(self.deck.pop().unwrap());
        }
        self.events.push(Event::Board {
            street: self.street,
            cards: self.board.clone(),
        });
        self.current = 0;
        self.last_raise = self.rules.big_blind;
        for p in &mut self.players {
            p.public.street_committed = 0;
            p.acted_at = None;
        }
        self.advance(self.button);
    }
    fn settle(&mut self) {
        self.actor = None;
        let live: Vec<_> = (0..self.players.len())
            .filter(|&i| !self.players[i].public.folded)
            .collect();
        if live.len() > 1 {
            for &seat in &live {
                self.events.push(Event::Reveal {
                    seat,
                    cards: self.players[seat].hole,
                });
            }
        }
        let mut levels: Vec<_> = self
            .players
            .iter()
            .map(|p| p.public.committed)
            .filter(|&v| v > 0)
            .collect();
        levels.sort_unstable();
        levels.dedup();
        let mut previous = 0;
        for level in levels {
            let contributors: Vec<_> = (0..self.players.len())
                .filter(|&i| self.players[i].public.committed >= level)
                .collect();
            let amount = (level - previous) * contributors.len() as u64;
            previous = level;
            if contributors.len() == 1 {
                let seat = contributors[0];
                self.players[seat].public.stack += amount;
                self.events.push(Event::Refund { seat, amount });
                continue;
            }
            let eligible: Vec<_> = contributors
                .iter()
                .copied()
                .filter(|i| live.contains(i))
                .collect();
            // A sole survivor wins all contested tiers, including dead contributions.
            let eligible = if live.len() == 1 {
                live.clone()
            } else {
                eligible
            };
            assert!(!eligible.is_empty(), "pot without eligible player");
            let winners = if eligible.len() == 1 {
                eligible.clone()
            } else {
                let ranked: Vec<_> = eligible
                    .iter()
                    .map(|&i| {
                        let mut c = self.board.clone();
                        c.extend(self.players[i].hole);
                        (i, evaluate(&c).unwrap())
                    })
                    .collect();
                let best = ranked.iter().map(|(_, r)| *r).max().unwrap();
                ranked
                    .into_iter()
                    .filter(|(_, r)| *r == best)
                    .map(|(i, _)| i)
                    .collect()
            };
            let mut winners: Vec<usize> = winners;
            winners
                .sort_by_key(|&i| (i + self.players.len() - self.button - 1) % self.players.len());
            let shares: Vec<_> = (0..winners.len())
                .map(|j| {
                    amount / winners.len() as u64
                        + u64::from((j as u64) < amount % winners.len() as u64)
                })
                .collect();
            for (&i, &share) in winners.iter().zip(&shares) {
                self.players[i].public.stack += share;
            }
            self.events.push(Event::Award {
                amount,
                eligible,
                winners,
                shares,
            });
        }
        for p in &mut self.players {
            p.public.committed = 0;
            p.public.street_committed = 0;
        }
        self.street = Street::Complete;
        debug_assert!(self.conserved());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tied_pot_odd_chip_and_folded_money() {
        let mut h = Hand::new(Rules::default(), vec![10; 3], 0, 1).unwrap();
        // Construct a river fixture: the board's royal flush ties both live players.
        h.board = [8, 9, 10, 11, 12]
            .into_iter()
            .map(|v| Card::try_from(v).unwrap())
            .collect();
        for (i, p) in h.players.iter_mut().enumerate() {
            p.hole = [
                Card::try_from(13 + i as u8 * 2).unwrap(),
                Card::try_from(14 + i as u8 * 2).unwrap(),
            ];
            p.public.stack = 7;
            p.public.committed = 3;
            p.public.street_committed = 0;
        }
        h.players[2].public.folded = true;
        h.events.clear();
        h.settle();
        assert_eq!(h.stacks(), vec![11, 12, 7]);
        assert!(h.conserved());
        assert!(h.events.contains(&Event::Award {
            amount: 9,
            eligible: vec![0, 1],
            winners: vec![1, 0],
            shares: vec![5, 4]
        }));
        assert!(
            !h.events
                .iter()
                .any(|e| matches!(e, Event::Reveal { seat: 2, .. }))
        );
    }
    #[test]
    fn unequal_all_ins_award_only_eligible_players() {
        let mut h = Hand::new(Rules::default(), vec![10, 20, 30], 0, 1).unwrap();
        h.board = [0, 16, 32, 48, 7]
            .into_iter()
            .map(|v| Card::try_from(v).unwrap())
            .collect();
        // Aces beat kings beat queens, but aces cannot win the side pot.
        for (i, p) in h.players.iter_mut().enumerate() {
            p.hole = [
                Card::try_from(12 - i as u8).unwrap(),
                Card::try_from(25 - i as u8).unwrap(),
            ];
            p.public.stack = 0;
            p.public.committed = (i as u64 + 1) * 10;
            p.public.street_committed = 0;
        }
        h.events.clear();
        h.settle();
        assert_eq!(h.stacks(), vec![30, 20, 10]);
        assert!(h.conserved());
    }
}
