use crate::{memory::Mode, openrouter::Config, scaffold::Scaffold};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub pilot: Config,
    pub seeds: Vec<u64>,
    pub model_seats: Vec<usize>,
    pub conditions: Vec<Mode>,
    /// Scaffold treatments crossed with `conditions`; empty means the pilot's own scaffold.
    #[serde(default)]
    pub scaffolds: Vec<Scaffold>,
    pub max_requests: u64,
    pub spend_limit_usd: f64,
    /// Independent sessions to run concurrently (0 or 1: one at a time).
    #[serde(default)]
    pub parallel: usize,
    /// Permit hand counts that are not a whole number of orbits. Positions are then unevenly
    /// sampled within a session (in ten six-handed hands, two seats post one fewer big blind).
    #[serde(default)]
    pub allow_partial_orbits: bool,
    /// Requests with unreported cost (for example an edge 503 with no usage) the batch absorbs
    /// before halting. 0, the default, halts on the first one.
    #[serde(default)]
    pub unknown_cost_allowance: u64,
    /// Conservative charge per absorbed unknown-cost request, counted against `spend_limit_usd`.
    #[serde(default = "default_unknown_charge")]
    pub unknown_cost_charge_usd: f64,
    /// When a session ends because the provider stayed down through every retry, the batch
    /// pauses new sessions for this long (doubling on repeats), then re-runs that session.
    #[serde(default = "default_outage_pause")]
    pub outage_pause_ms: u64,
    /// Total pause time after which a batch stops with `provider_outage` (resumable).
    #[serde(default = "default_outage_wait")]
    pub outage_max_wait_s: u64,
}
fn default_outage_pause() -> u64 {
    30_000
}
fn default_outage_wait() -> u64 {
    900
}
fn default_unknown_charge() -> f64 {
    0.01
}
impl Batch {
    pub fn schedule(&self) -> Result<Vec<Config>, String> {
        self.pilot.validate()?;
        if !self.pilot.hands.is_multiple_of(self.pilot.seats) && !self.allow_partial_orbits {
            return Err(format!(
                "hands ({}) must be a multiple of seats ({}) so every seat posts the same blinds; set allow_partial_orbits to override",
                self.pilot.hands, self.pilot.seats
            ));
        }
        if self.parallel > 64 {
            return Err("parallel is limited to 64 concurrent sessions".into());
        }
        if self.seeds.is_empty()
            || self.model_seats.is_empty()
            || self.conditions.is_empty()
            || self.max_requests == 0
            || !self.spend_limit_usd.is_finite()
            || self.spend_limit_usd <= 0.0
            || !self.unknown_cost_charge_usd.is_finite()
            || self.unknown_cost_charge_usd < 0.0
        {
            return Err("empty schedule or invalid global limit".into());
        }
        let scaffolds = if self.scaffolds.is_empty() {
            vec![self.pilot.scaffold]
        } else {
            self.scaffolds.clone()
        };
        let mut result = vec![];
        for (condition, scaffold) in self
            .conditions
            .iter()
            .flat_map(|c| scaffolds.iter().map(move |s| (c, s)))
        {
            for &seed in &self.seeds {
                for &seat in &self.model_seats {
                    let mut cfg = self.pilot.clone();
                    cfg.memory = *condition;
                    cfg.scaffold = *scaffold;
                    cfg.seed = seed;
                    if cfg.all_models {
                        // All-model table: `model_seats` are rotations of the model list, so
                        // every model plays every position on every seed.
                        cfg.model_offset = seat;
                    } else {
                        cfg.model_seat = seat;
                        // Reorder opponents as well as moving the model; avoid fixed relative neighbors.
                        cfg.opponent_offset = seat;
                    }
                    cfg.validate()?;
                    result.push(cfg);
                }
            }
        }
        if result.len() > 10000 {
            return Err("schedule too large".into());
        }
        Ok(result)
    }
}
