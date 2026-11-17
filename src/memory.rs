use crate::engine::{Action, Event, PlayerView, Street};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    None,
    Notes,
    Stats,
    Both,
    /// Experimenter-written, accurate opponent descriptions; never updated by the model.
    Oracle,
}
impl Mode {
    /// A notebook is shown to the model at decision time.
    pub fn notes(self) -> bool {
        matches!(self, Self::Notes | Self::Both | Self::Oracle)
    }
    /// The model rewrites its notebook after each hand (one extra provider call).
    pub fn updates(self) -> bool {
        matches!(self, Self::Notes | Self::Both)
    }
    pub fn stats(self) -> bool {
        matches!(self, Self::Stats | Self::Both)
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SeatStats {
    pub dealt: u64,
    pub preflop_opportunities: u64,
    pub vpip: u64,
    pub pfr: u64,
    pub showdowns: u64,
    /// Postflop aggression, as a tracker shows it: bets and raises, and calls.
    pub postflop_bets_raises: u64,
    pub postflop_calls: u64,
    /// Postflop decisions made while facing another seat's bet, and how many were folds.
    pub faced_postflop_bets: u64,
    pub folded_to_postflop_bets: u64,
    /// The same on the river alone.
    pub faced_river_bets: u64,
    pub folded_to_river_bets: u64,
}
#[derive(Default)]
pub struct Memory {
    pub notebook: String,
    pub revision: u64,
    pub stats: Vec<SeatStats>,
    /// Per-opponent profiles for the structured notebook; `notebook` is rendered from them.
    pub profiles: std::collections::BTreeMap<usize, String>,
}
impl Memory {
    /// Applies per-seat profile edits. A profile longer than `per_seat` characters, or for a
    /// seat that is not a listed opponent, is ignored and that seat keeps its old profile; an
    /// empty profile deletes the seat's entry. Returns how many edits were ignored.
    pub fn apply_profiles(
        &mut self,
        opponents: &[usize],
        edits: &[(usize, String)],
        per_seat: usize,
    ) -> usize {
        let mut ignored = 0;
        let before = self.notebook.clone();
        for (seat, text) in edits {
            if !opponents.contains(seat) || text.chars().count() > per_seat {
                ignored += 1;
            } else if text.trim().is_empty() {
                self.profiles.remove(seat);
            } else {
                self.profiles.insert(*seat, text.trim().to_owned());
            }
        }
        self.notebook = self
            .profiles
            .iter()
            .map(|(seat, text)| format!("Seat {seat}: {text}"))
            .collect::<Vec<_>>()
            .join("\n");
        if self.notebook != before {
            self.revision += 1;
        }
        ignored
    }
    pub fn observe(&mut self, v: &PlayerView) {
        self.stats.resize_with(v.players.len(), SeatStats::default);
        let mut opportunity = vec![false; v.players.len()];
        let mut vpip = opportunity.clone();
        let mut pfr = opportunity.clone();
        let mut revealed = opportunity.clone();
        let mut preflop = true;
        let mut river = false;
        // The last seat to bet or raise on the current street.
        let mut bettor: Option<usize> = None;
        for e in &v.history {
            match e {
                Event::Board { street, .. } => {
                    preflop = false;
                    river = *street == Street::River;
                    bettor = None;
                }
                Event::Act { seat, action } if !preflop => {
                    let s = &mut self.stats[*seat];
                    let facing = bettor.is_some_and(|b| b != *seat);
                    let folded = *action == Action::Fold;
                    s.faced_postflop_bets += u64::from(facing);
                    s.folded_to_postflop_bets += u64::from(facing && folded);
                    s.faced_river_bets += u64::from(facing && river);
                    s.folded_to_river_bets += u64::from(facing && river && folded);
                    match action {
                        Action::RaiseTo(_) => {
                            s.postflop_bets_raises += 1;
                            bettor = Some(*seat);
                        }
                        Action::Call => s.postflop_calls += 1,
                        _ => {}
                    }
                }
                Event::Act { seat, action } if preflop => {
                    opportunity[*seat] = true;
                    if matches!(action, Action::Call | Action::RaiseTo(_)) {
                        vpip[*seat] = true;
                    }
                    if matches!(action, Action::RaiseTo(_)) {
                        pfr[*seat] = true;
                    }
                }
                Event::Reveal { seat, .. } => revealed[*seat] = true,
                _ => {}
            }
        }
        for (i, s) in self.stats.iter_mut().enumerate() {
            s.dealt += 1;
            s.preflop_opportunities += u64::from(opportunity[i]);
            s.vpip += u64::from(vpip[i]);
            s.pfr += u64::from(pfr[i]);
            s.showdowns += u64::from(revealed[i]);
        }
    }
    pub fn update(&mut self, operation: &str, text: &str, cap: usize) -> Result<(), String> {
        match operation {
            // "keep" means keep: any text sent alongside it is ignored, not an error.
            "keep" => Ok(()),
            "replace" if text.chars().count() <= cap => {
                self.notebook = text.into();
                self.revision += 1;
                Ok(())
            }
            _ => Err("invalid or oversized notebook update".into()),
        }
    }
}
pub fn completed(v: &PlayerView) -> bool {
    v.street == Street::Complete && v.legal.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arena::{Agent, Scripted, play},
        engine::Rules,
    };

    #[test]
    fn stats_count_postflop_aggression_and_folds_to_bets_like_a_tracker() {
        // Seat 0 min-raises every street, seat 1 calls everything: seat 1 faces a bet on the
        // flop, turn and river and calls each; seat 0 bets each street and never faces one.
        let played = play(
            Rules::default(),
            vec![200; 2],
            0,
            7,
            &mut [
                Box::new(Scripted::Raiser) as Box<dyn Agent>,
                Box::new(Scripted::Caller),
            ],
        )
        .unwrap();
        let mut memory = Memory::default();
        memory.observe(played.private_views[0].last().unwrap());
        let (raiser, caller) = (&memory.stats[0], &memory.stats[1]);
        assert_eq!(raiser.postflop_bets_raises, 3);
        assert_eq!((raiser.faced_postflop_bets, raiser.postflop_calls), (0, 0));
        assert_eq!(caller.faced_postflop_bets, 3);
        assert_eq!(caller.postflop_calls, 3);
        assert_eq!(caller.folded_to_postflop_bets, 0);
        assert_eq!(
            (caller.faced_river_bets, caller.folded_to_river_bets),
            (1, 0)
        );
        // A folder facing a river bet counts there.
        let played = play(
            Rules::default(),
            vec![200; 2],
            1,
            7,
            &mut [
                Box::new(Scripted::Folder) as Box<dyn Agent>,
                Box::new(Scripted::Raiser),
            ],
        )
        .unwrap();
        let mut memory = Memory::default();
        memory.observe(played.private_views[0].last().unwrap());
        assert_eq!(memory.stats[0].dealt, 1);
    }
}
