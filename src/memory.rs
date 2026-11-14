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
        for e in &v.history {
            match e {
                Event::Board { .. } => preflop = false,
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
